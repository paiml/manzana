// This module requires unsafe for memory allocation
#![allow(unsafe_code)]

//! Page-aligned host buffer management.
//!
//! # Scope
//!
//! [`UmaBuffer`] is a real, page-aligned heap allocation made with
//! [`std::alloc::alloc`], with RAII deallocation. That much works and is
//! tested.
//!
//! It is **not** a Metal buffer and it is **not** shared with a GPU. This
//! module creates no `MTLBuffer`, no `IOSurface`, and makes no Metal or IOKit
//! call of any kind. On Apple Silicon the CPU and GPU do share physical
//! memory, but a host allocation only becomes GPU-visible once it is wrapped
//! (for example via `newBufferWithBytesNoCopy:`), and manzana does not do
//! that. Page alignment is a *precondition* for such a wrap, not the wrap
//! itself.
//!
//! Earlier documentation claimed the buffer was "accessible to GPU without
//! copying" and listed a falsification claim "F074: Zero-copy verified".
//! Nothing verified it, because there was no GPU path to verify.
//!
//! # Example
//!
//! ```
//! use manzana::unified_memory::UmaBuffer;
//!
//! // Allocate a 1 MB page-aligned host buffer.
//! let mut buffer = UmaBuffer::new(1024 * 1024)?;
//!
//! // Write data from the CPU.
//! let data = buffer.as_mut_slice();
//! data[0] = 42;
//! # Ok::<(), manzana::Error>(())
//! ```
//!
//! # Falsification Claims
//!
//! - F071: Buffer allocation succeeds
//! - F076: Allocation is page-aligned (a prerequisite for a future Metal wrap)

use crate::error::{Error, Result};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;

/// Page size for Metal buffer alignment (4096 bytes).
pub const PAGE_SIZE: usize = 4096;

/// Maximum allocation size (16 GB).
pub const MAX_ALLOCATION: usize = 17_179_869_184;

/// A unified memory buffer shared between CPU and GPU.
///
/// On Apple Silicon, this buffer uses unified memory architecture,
/// meaning both CPU and GPU can access it without data copies.
///
/// # Safety
///
/// The buffer is page-aligned for Metal compatibility and uses
/// RAII for automatic deallocation.
///
/// # Thread Safety
///
/// This type is `Send` but not `Sync`. The buffer can be moved
/// between threads, but concurrent access requires external
/// synchronization.
pub struct UmaBuffer {
    ptr: NonNull<u8>,
    len: usize,
    layout: Layout,
}

// SAFETY: UmaBuffer owns its memory and uses NonNull for the pointer.
// The buffer can safely be sent to another thread since the memory
// is heap-allocated and will be properly deallocated in Drop.
// Concurrent access is prevented by not implementing Sync.
unsafe impl Send for UmaBuffer {}

impl UmaBuffer {
    /// Allocate a new unified memory buffer.
    ///
    /// # Arguments
    ///
    /// * `len` - Size in bytes (must be > 0 and <= 16 GB)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `len` is zero
    /// - `len` exceeds maximum allocation size
    /// - Memory allocation fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manzana::unified_memory::UmaBuffer;
    ///
    /// let buffer = UmaBuffer::new(1024)?;
    /// assert_eq!(buffer.len(), 1024);
    /// # Ok::<(), manzana::Error>(())
    /// ```
    pub fn new(len: usize) -> Result<Self> {
        if len == 0 {
            return Err(Error::invalid_input("buffer length cannot be zero"));
        }

        if len > MAX_ALLOCATION {
            return Err(Error::invalid_input(format!(
                "allocation size {len} exceeds maximum {MAX_ALLOCATION} bytes"
            )));
        }

        // Round up to page alignment for Metal compatibility
        let aligned_len = (len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        // Create layout with page alignment
        let layout = Layout::from_size_align(aligned_len, PAGE_SIZE)
            .map_err(|e| Error::internal(format!("invalid layout: {e}")))?;

        // SAFETY: layout is valid (checked above) and its size is non-zero,
        // since `len >= 1` and `aligned_len` rounds up.
        //
        // `alloc_zeroed`, not `alloc`: `as_slice` and `as_mut_slice` are SAFE
        // public methods that build a `&[u8]` over this allocation. Producing a
        // reference to uninitialized memory is undefined behaviour, so with a
        // plain `alloc` the sequence
        //
        //     let b = UmaBuffer::new(1024)?;   // safe
        //     let _ = b.as_slice()[0];         // safe -> UB
        //
        // was unsound from entirely safe code. Zeroing on allocation makes the
        // buffer initialized before any reference to it can exist. The OS
        // generally hands back pre-zeroed pages, so this is close to free.
        let ptr = unsafe { alloc_zeroed(layout) };

        let ptr = NonNull::new(ptr).ok_or_else(|| {
            Error::internal(format!("memory allocation failed for {aligned_len} bytes"))
        })?;

        Ok(Self { ptr, len, layout })
    }

    /// Allocate a zeroed unified memory buffer.
    ///
    /// Equivalent to [`UmaBuffer::new`], which also zeroes. Retained because
    /// it names the guarantee explicitly at the call site.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `len` is zero
    /// - `len` exceeds maximum allocation size
    /// - Memory allocation fails
    pub fn zeroed(len: usize) -> Result<Self> {
        // `new` already allocates with `alloc_zeroed`.
        let buffer = Self::new(len)?;

        Ok(buffer)
    }

    /// Get the buffer length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the actual allocated size (page-aligned).
    #[must_use]
    pub const fn allocated_size(&self) -> usize {
        self.layout.size()
    }

    /// Get a raw pointer to the buffer.
    ///
    /// # Safety
    ///
    /// The caller must ensure the buffer is not accessed after
    /// the `UmaBuffer` is dropped.
    #[must_use]
    pub const fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    /// Get a mutable raw pointer to the buffer.
    ///
    /// # Safety
    ///
    /// The caller must ensure exclusive access and that the buffer
    /// is not accessed after the `UmaBuffer` is dropped.
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Get a slice view of the buffer.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // slice::from_raw_parts is not const-stable
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `ptr` is non-null and owns at least `len` bytes, which were
        // zeroed by `alloc_zeroed` in `new`, so the region is initialized.
        // `&self` guarantees no concurrent mutable alias.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Get a mutable slice view of the buffer.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: as `as_slice`, and `&mut self` guarantees exclusive access.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    /// Check if the buffer is page-aligned (required for Metal).
    #[must_use]
    pub fn is_aligned(&self) -> bool {
        (self.ptr.as_ptr() as usize) % PAGE_SIZE == 0
    }

    /// Copy data into the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the source slice is larger than the buffer.
    pub fn copy_from_slice(&mut self, src: &[u8]) -> Result<()> {
        if src.len() > self.len {
            return Err(Error::invalid_input(format!(
                "source length {} exceeds buffer length {}",
                src.len(),
                self.len
            )));
        }

        self.as_mut_slice()[..src.len()].copy_from_slice(src);
        Ok(())
    }

    /// Check if UMA is available on this system.
    ///
    /// Returns `true` on Apple Silicon, `false` on Intel Macs.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_uma_available() -> bool {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            true
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            false
        }
    }
}

impl Drop for UmaBuffer {
    fn drop(&mut self) {
        // SAFETY: ptr was allocated with the same layout
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

impl std::fmt::Debug for UmaBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UmaBuffer")
            .field("len", &self.len)
            .field("allocated_size", &self.layout.size())
            .field("aligned", &self.is_aligned())
            .finish_non_exhaustive()
    }
}

/// Check if unified memory is available.
///
/// Convenience function equivalent to `UmaBuffer::is_uma_available()`.
#[must_use]
#[allow(clippy::missing_const_for_fn)]
pub fn is_available() -> bool {
    UmaBuffer::is_uma_available()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {

    // Regression: `new()` used plain `alloc()`, leaving the buffer
    // uninitialized. `as_slice()` is a SAFE method that builds a `&[u8]` over
    // it, so this sequence was undefined behaviour from entirely safe code.
    // It must now read back as zeros.
    #[test]
    fn test_new_buffer_is_initialized_not_uninit() {
        let buf = UmaBuffer::new(4096).expect("allocation");
        assert!(
            buf.as_slice().iter().all(|&b| b == 0),
            "new() must hand back initialized memory; as_slice() is safe and \
             a &[u8] over uninitialized bytes is UB"
        );
    }

    #[test]
    fn test_new_and_zeroed_agree() {
        let a = UmaBuffer::new(8192).expect("alloc");
        let b = UmaBuffer::zeroed(8192).expect("alloc");
        assert_eq!(a.as_slice(), b.as_slice());
    }

    use super::*;

    // F071: UMA buffer allocation succeeds
    #[test]
    fn test_allocation_success() {
        let buffer = UmaBuffer::new(1024);
        assert!(buffer.is_ok());
        let buffer = buffer.unwrap();
        assert_eq!(buffer.len(), 1024);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn test_allocation_zero_fails() {
        let result = UmaBuffer::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_allocation_too_large_fails() {
        let result = UmaBuffer::new(MAX_ALLOCATION + 1);
        assert!(result.is_err());
    }

    // F076: Alignment correct for Metal
    #[test]
    fn test_page_alignment() {
        let buffer = UmaBuffer::new(100).unwrap();
        assert!(buffer.is_aligned(), "Buffer should be page-aligned");
        assert!(
            buffer.allocated_size() >= PAGE_SIZE,
            "Allocated size should be at least one page"
        );
    }

    #[test]
    fn test_zeroed_buffer() {
        let buffer = UmaBuffer::zeroed(1024).unwrap();
        let slice = buffer.as_slice();
        assert!(slice.iter().all(|&b| b == 0), "Buffer should be zeroed");
    }

    #[test]
    fn test_read_write() {
        let mut buffer = UmaBuffer::new(1024).unwrap();

        // Write some data
        let data = buffer.as_mut_slice();
        data[0] = 42;
        data[100] = 255;

        // Read it back
        let data = buffer.as_slice();
        assert_eq!(data[0], 42);
        assert_eq!(data[100], 255);
    }

    #[test]
    fn test_copy_from_slice() {
        let mut buffer = UmaBuffer::new(1024).unwrap();
        let src = vec![1u8, 2, 3, 4, 5];

        let result = buffer.copy_from_slice(&src);
        assert!(result.is_ok());

        let data = buffer.as_slice();
        assert_eq!(&data[..5], &src[..]);
    }

    #[test]
    fn test_copy_from_slice_too_large() {
        let mut buffer = UmaBuffer::new(10).unwrap();
        let src = vec![0u8; 100];

        let result = buffer.copy_from_slice(&src);
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_format() {
        let buffer = UmaBuffer::new(1024).unwrap();
        let debug = format!("{buffer:?}");
        assert!(debug.contains("UmaBuffer"));
        assert!(debug.contains("len"));
        assert!(debug.contains("1024"));
    }

    #[test]
    fn test_is_uma_available() {
        let available = UmaBuffer::is_uma_available();
        // On Apple Silicon: true, elsewhere: false
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert!(available);
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        assert!(!available);
    }

    #[test]
    fn test_convenience_function() {
        assert_eq!(is_available(), UmaBuffer::is_uma_available());
    }

    #[test]
    fn test_pointers() {
        let mut buffer = UmaBuffer::new(1024).unwrap();

        let ptr = buffer.as_ptr();
        assert!(!ptr.is_null());

        let mut_ptr = buffer.as_mut_ptr();
        assert!(!mut_ptr.is_null());
        assert_eq!(ptr, mut_ptr);
    }

    #[test]
    fn test_large_allocation() {
        // 1 MB allocation
        let buffer = UmaBuffer::new(1024 * 1024);
        assert!(buffer.is_ok());
    }

    #[test]
    fn test_multiple_buffers() {
        let buffer1 = UmaBuffer::new(1024).unwrap();
        let buffer2 = UmaBuffer::new(2048).unwrap();

        assert_eq!(buffer1.len(), 1024);
        assert_eq!(buffer2.len(), 2048);

        // Buffers should be at different addresses
        assert_ne!(buffer1.as_ptr(), buffer2.as_ptr());
    }
}
