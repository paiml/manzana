// This module requires unsafe for memory allocation
#![allow(unsafe_code)]

//! Page-aligned host buffer management.
//!
//! # Scope
//!
//! [`UmaBuffer`] is a real, page-aligned heap allocation made with
//! [`std::alloc::alloc_zeroed`], with RAII deallocation. That much works and is
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

/// Round `len` up to a page boundary, or `None` if that would overflow.
///
/// Extracted so it can be tested directly. Testing it through
/// [`UmaBuffer::new`] is VACUOUS on 64-bit targets: `MAX_ALLOCATION` is 16 GB
/// there, so every input that would wrap is rejected by the bounds check
/// before alignment runs, and a test written that way passes against the
/// wrapping implementation too. On 32-bit, `MAX_ALLOCATION` is `usize::MAX`
/// and the wrap is reachable.
const fn page_align(len: usize) -> Option<usize> {
    match len.checked_add(PAGE_SIZE - 1) {
        Some(v) => Some(v & !(PAGE_SIZE - 1)),
        None => None,
    }
}

/// Maximum allocation size: 16 GB, or `usize::MAX` on targets too small to
/// express it.
///
/// The literal `17_179_869_184` does not fit a 32-bit `usize` and made the
/// crate fail to compile on 32-bit targets outright.
// The cast below is guarded by `WANT <= usize::MAX as u64`, so it cannot
// truncate; clippy's lint is shape-based and cannot see the guard. Plain
// `allow` rather than `expect`/`reason`, which need Rust 1.81 while this
// crate's MSRV is 1.75.
#[allow(clippy::cast_possible_truncation)]
pub const MAX_ALLOCATION: usize = {
    // Expressed as a u64 literal, which fits on every target, then narrowed.
    // Writing `if usize::BITS >= 64 { 17_179_869_184 }` does NOT work: the
    // deny-by-default `overflowing_literals` lint fires on the LITERAL in a
    // 32-bit usize context, before the branch is chosen, so the crate still
    // failed to compile there while the comment claimed otherwise.
    //
    // Not verified by building: no 32-bit target is installed here, and
    // `cargo check --target i686-unknown-linux-gnu` fails with "can't find
    // crate for std". This form has no oversized literal on any target, but
    // that is reasoning, not a passing build.
    const WANT: u64 = 17_179_869_184; // 16 GB
    if WANT <= usize::MAX as u64 {
        WANT as usize
    } else {
        usize::MAX
    }
};

/// A page-aligned host buffer.
///
/// **Not GPU-visible.** This is a `std::alloc::alloc_zeroed` allocation on the
/// host heap. It is not an `MTLBuffer`, is not wrapped with
/// `newBufferWithBytesNoCopy:`, and no GPU can read it. Page alignment is a
/// prerequisite for such a wrap, not the wrap itself.
///
/// The name is retained for API continuity and is itself misleading; renaming
/// it is tracked in `docs/specifications/security-architecture-plan.md`.
/// Earlier documentation on this struct claimed CPU and GPU "can access it
/// without data copies", which was never true.
///
/// # Safety
///
/// The buffer is page-aligned and uses RAII for automatic deallocation.
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
        // Checked. `(len + PAGE_SIZE - 1)` wraps for len near usize::MAX, and
        // on a 32-bit target MAX_ALLOCATION is usize::MAX, so such a len passed
        // the bounds check above and then aligned to ZERO. alloc_zeroed(0)
        // succeeds, and as_slice() would then build a usize::MAX-length slice
        // over a zero-sized allocation -- out-of-bounds UB from safe code, the
        // same class as the alloc()/uninitialised defect this release fixed.
        let Some(aligned_len) = page_align(len) else {
            return Err(Error::invalid_input(format!(
                "allocation size {len} overflows when page-aligned"
            )));
        };

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

    /// Whether manzana can provide unified (GPU-visible) memory.
    ///
    /// **Always returns `false`.** Apple Silicon does have a unified memory
    /// architecture, but that is a fact about the chip, not about this crate:
    /// [`UmaBuffer`] is a host allocation that no GPU can read. Reporting
    /// `true` would tell a caller it has zero-copy CPU/GPU sharing available
    /// when nothing here can deliver it.
    ///
    /// The previous implementation returned `true` whenever the *build target*
    /// was `aarch64` macOS. A compile-time target check cannot observe the
    /// machine the code runs on, and it was reporting a capability rather than
    /// the chip property it actually inferred. Flagged by
    /// `scripts/check_hardware_reachability.sh` as `capability-without-probe`.
    #[must_use]
    pub const fn is_uma_available() -> bool {
        false
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
mod tests;
