//! The page-aligned host buffer type.
//!
//! Split out of `mod.rs` because documenting precisely what this buffer is
//! NOT -- an MTLBuffer, GPU-visible memory -- takes more room than the type.

use super::{page_align, MAX_ALLOCATION, PAGE_SIZE};
use crate::error::{Error, Result};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;

/// An owned, zero-initialized, [`PAGE_SIZE`]-aligned buffer in **host memory**.
///
/// **The name overstates this type: it is not GPU-visible.** A `UmaBuffer` is
/// a [`std::alloc::alloc_zeroed`] allocation on the process heap. It is not an
/// `MTLBuffer`, it is never wrapped with `newBufferWithBytesNoCopy:`, and
/// nothing here makes it addressable by a GPU or by the Neural Engine — on
/// Apple Silicon or anywhere else. Only the CPU can read or write it. The name
/// is kept for API continuity in 0.3.0; renaming it is in scope for the pass
/// described in `docs/specifications/security-architecture-plan.md`.
///
/// What the type does provide:
///
/// - the allocation is aligned to [`PAGE_SIZE`], which
///   [`is_aligned`](Self::is_aligned) re-checks;
/// - its bytes are zero when it is handed to you (see
///   [Initialization](#initialization));
/// - the memory is freed when the value is dropped.
///
/// # Initialization
///
/// The allocation is always made with `alloc_zeroed`, never plain `alloc`,
/// including in [`new`](Self::new). This is required for soundness rather than
/// a convenience: [`as_slice`](Self::as_slice) and
/// [`as_mut_slice`](Self::as_mut_slice) are *safe* methods that construct a
/// `&[u8]` over the allocation, and a reference to uninitialized memory is
/// undefined behaviour. Zeroing at allocation means the bytes are initialized
/// before any reference to them can exist, so
///
/// ```
/// # use manzana::unified_memory::UmaBuffer;
/// let b = UmaBuffer::new(1024)?;   // safe
/// let _ = b.as_slice()[0];         // safe, and defined
/// # Ok::<(), manzana::Error>(())
/// ```
///
/// is sound from entirely safe code. Through 0.2.0 `new` used plain `alloc`
/// and that sequence was UB.
///
/// # Sizes
///
/// [`len`](Self::len) is the size you requested and the length of the slices;
/// [`allocated_size`](Self::allocated_size) is that size rounded up to
/// [`PAGE_SIZE`], which is what is actually allocated and freed. The bytes
/// past `len` are allocated and zeroed but are not reachable through the safe
/// API.
///
/// # Thread safety
///
/// `UmaBuffer` is [`Send`] but not [`Sync`]: it owns its allocation and can be
/// moved between threads, but shared concurrent access needs external
/// synchronization.
///
/// # Examples
///
/// ```
/// use manzana::unified_memory::{UmaBuffer, PAGE_SIZE};
///
/// let mut buffer = UmaBuffer::new(100)?;
///
/// assert_eq!(buffer.len(), 100);              // what you asked for
/// assert_eq!(buffer.allocated_size(), PAGE_SIZE); // what was allocated
/// assert!(buffer.is_aligned());
///
/// buffer.copy_from_slice(&[1, 2, 3])?;
/// assert_eq!(&buffer.as_slice()[..4], &[1, 2, 3, 0]);
/// # Ok::<(), manzana::Error>(())
/// ```
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
    /// Allocates `len` bytes of zeroed, page-aligned host memory.
    ///
    /// The allocation is rounded up to a multiple of [`PAGE_SIZE`]; `len`
    /// itself is what [`len`](Self::len) and the slice accessors report. The
    /// buffer is not GPU-visible — see the [type
    /// documentation](Self).
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidInput`] if `len` is `0`. A zero-length buffer is
    ///   rejected rather than allocated, which is why
    ///   [`is_empty`](Self::is_empty) never returns `true`.
    /// - [`Error::InvalidInput`] if `len` is greater than [`MAX_ALLOCATION`].
    /// - [`Error::InvalidInput`] if rounding `len` up to [`PAGE_SIZE`]
    ///   overflows `usize`. Reachable only on targets where `MAX_ALLOCATION`
    ///   is `usize::MAX`, i.e. those that cannot express 16 GB; on 64-bit the
    ///   size check above rejects such a `len` first.
    ///   Unchecked, that wrap produced a zero-sized allocation over which
    ///   `as_slice` would build a `usize::MAX`-length slice — out-of-bounds
    ///   undefined behaviour reachable from safe code.
    /// - [`Error::Internal`] if [`Layout::from_size_align`] rejects the
    ///   rounded size, which happens when it exceeds `isize::MAX` after
    ///   alignment. Same target caveat as above.
    /// - [`Error::Internal`] if the global allocator returns null. `new`
    ///   reports this as an error; it does not call `handle_alloc_error` and
    ///   does not abort.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::unified_memory::{UmaBuffer, PAGE_SIZE};
    ///
    /// let buffer = UmaBuffer::new(1024)?;
    /// assert_eq!(buffer.len(), 1024);
    /// assert_eq!(buffer.allocated_size(), PAGE_SIZE);
    /// assert!(buffer.as_slice().iter().all(|&b| b == 0));
    /// # Ok::<(), manzana::Error>(())
    /// ```
    ///
    /// Rejected sizes:
    ///
    /// ```
    /// use manzana::unified_memory::{UmaBuffer, MAX_ALLOCATION};
    ///
    /// let err = UmaBuffer::new(0).unwrap_err();
    /// assert_eq!(err.to_string(), "invalid input: buffer length cannot be zero");
    ///
    /// let err = UmaBuffer::new(MAX_ALLOCATION + 1).unwrap_err();
    /// assert!(err.to_string().contains("exceeds maximum"));
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

    /// Allocates `len` bytes of zeroed, page-aligned host memory.
    ///
    /// Identical to [`new`](Self::new), which also zeroes; this is a thin
    /// wrapper around it. Retained because it names the guarantee at the call
    /// site.
    ///
    /// # Errors
    ///
    /// Exactly those of [`new`](Self::new).
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::unified_memory::UmaBuffer;
    ///
    /// let a = UmaBuffer::new(8192)?;
    /// let b = UmaBuffer::zeroed(8192)?;
    /// assert_eq!(a.as_slice(), b.as_slice());
    /// # Ok::<(), manzana::Error>(())
    /// ```
    pub fn zeroed(len: usize) -> Result<Self> {
        // `new` already allocates with `alloc_zeroed`.
        let buffer = Self::new(len)?;

        Ok(buffer)
    }

    /// Returns the requested length in bytes.
    ///
    /// This is the `len` passed to [`new`](Self::new), and the length of the
    /// slices returned by [`as_slice`](Self::as_slice) and
    /// [`as_mut_slice`](Self::as_mut_slice). For the size actually allocated,
    /// see [`allocated_size`](Self::allocated_size).
    ///
    /// ```
    /// # use manzana::unified_memory::UmaBuffer;
    /// let buffer = UmaBuffer::new(100)?;
    /// assert_eq!(buffer.len(), 100);
    /// assert_eq!(buffer.as_slice().len(), 100);
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the buffer has length zero.
    ///
    /// Always `false` for a buffer you can hold: [`new`](Self::new) rejects a
    /// `len` of `0`, so no zero-length `UmaBuffer` can be constructed. The
    /// method exists because clippy requires it alongside
    /// [`len`](Self::len).
    ///
    /// ```
    /// # use manzana::unified_memory::UmaBuffer;
    /// assert!(!UmaBuffer::new(1)?.is_empty());
    /// assert!(UmaBuffer::new(0).is_err());
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of bytes actually allocated: [`len`](Self::len)
    /// rounded up to a multiple of [`PAGE_SIZE`].
    ///
    /// This is the size that will be freed on drop. Bytes between
    /// [`len`](Self::len) and this value are allocated and zeroed but are not
    /// exposed by the slice accessors.
    ///
    /// ```
    /// # use manzana::unified_memory::{UmaBuffer, PAGE_SIZE};
    /// let buffer = UmaBuffer::new(PAGE_SIZE + 1)?;
    /// assert_eq!(buffer.len(), PAGE_SIZE + 1);
    /// assert_eq!(buffer.allocated_size(), PAGE_SIZE * 2);
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub const fn allocated_size(&self) -> usize {
        self.layout.size()
    }

    /// Returns a raw pointer to the start of the allocation.
    ///
    /// The pointer is non-null, aligned to [`PAGE_SIZE`], and valid for reads
    /// of [`allocated_size`](Self::allocated_size) bytes for as long as this
    /// `UmaBuffer` is alive. It is host memory: passing it to a GPU or to
    /// another process establishes nothing, because nothing has mapped it
    /// there.
    ///
    /// Dereferencing it is `unsafe` and is the caller's responsibility; in
    /// particular it dangles once the `UmaBuffer` is dropped.
    ///
    /// ```
    /// # use manzana::unified_memory::{UmaBuffer, PAGE_SIZE};
    /// let buffer = UmaBuffer::new(1024)?;
    /// assert!(!buffer.as_ptr().is_null());
    /// assert_eq!(buffer.as_ptr() as usize % PAGE_SIZE, 0);
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub const fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    /// Returns a mutable raw pointer to the start of the allocation.
    ///
    /// As [`as_ptr`](Self::as_ptr), and additionally valid for writes. Taking
    /// `&mut self` means no other reference into the buffer can exist while
    /// this pointer is obtained, but keeping it past that borrow and writing
    /// through it while a slice is alive is the caller's responsibility, as is
    /// not using it after the `UmaBuffer` is dropped.
    ///
    /// ```
    /// # use manzana::unified_memory::UmaBuffer;
    /// let mut buffer = UmaBuffer::new(1024)?;
    /// assert_eq!(buffer.as_ptr(), buffer.as_mut_ptr());
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Borrows the buffer's first [`len`](Self::len) bytes as a slice.
    ///
    /// Safe to call: the allocation is zero-initialized before any `UmaBuffer`
    /// exists, so this never builds a reference to uninitialized memory. The
    /// slice does not cover the page padding up to
    /// [`allocated_size`](Self::allocated_size).
    ///
    /// ```
    /// # use manzana::unified_memory::UmaBuffer;
    /// let buffer = UmaBuffer::new(64)?;
    /// assert_eq!(buffer.as_slice(), &[0u8; 64][..]);
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // slice::from_raw_parts is not const-stable
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `ptr` is non-null and owns at least `len` bytes, which were
        // zeroed by `alloc_zeroed` in `new`, so the region is initialized.
        // `&self` guarantees no concurrent mutable alias.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Mutably borrows the buffer's first [`len`](Self::len) bytes as a slice.
    ///
    /// As [`as_slice`](Self::as_slice), with exclusive access. Writes go to
    /// host memory only; no GPU observes them.
    ///
    /// ```
    /// # use manzana::unified_memory::UmaBuffer;
    /// let mut buffer = UmaBuffer::new(16)?;
    /// buffer.as_mut_slice()[3] = 7;
    /// assert_eq!(buffer.as_slice()[3], 7);
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: as `as_slice`, and `&mut self` guarantees exclusive access.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    /// Returns `true` if the allocation's address is a multiple of
    /// [`PAGE_SIZE`].
    ///
    /// This re-checks an invariant rather than reporting something variable:
    /// the [`Layout`] passed to the allocator specifies [`PAGE_SIZE`]
    /// alignment, so a successfully constructed `UmaBuffer` always satisfies
    /// it. Note what it does *not* establish — page alignment is one
    /// precondition for wrapping host memory in a Metal buffer, and manzana
    /// performs no such wrap.
    ///
    /// ```
    /// # use manzana::unified_memory::UmaBuffer;
    /// assert!(UmaBuffer::new(1)?.is_aligned());
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[must_use]
    pub fn is_aligned(&self) -> bool {
        (self.ptr.as_ptr() as usize) % PAGE_SIZE == 0
    }

    /// Copies `src` into the start of the buffer, leaving the remaining bytes
    /// unchanged.
    ///
    /// `src` may be shorter than the buffer; it is copied to offset `0` and
    /// nothing after `src.len()` is touched.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidInput`] if `src.len()` is greater than
    /// [`len`](Self::len). Nothing is copied in that case.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::unified_memory::UmaBuffer;
    ///
    /// let mut buffer = UmaBuffer::new(8)?;
    /// buffer.copy_from_slice(&[1, 2, 3])?;
    /// assert_eq!(buffer.as_slice(), &[1, 2, 3, 0, 0, 0, 0, 0]);
    ///
    /// let err = buffer.copy_from_slice(&[0u8; 9]).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "invalid input: source length 9 exceeds buffer length 8"
    /// );
    /// // The failed copy left the buffer alone.
    /// assert_eq!(buffer.as_slice(), &[1, 2, 3, 0, 0, 0, 0, 0]);
    /// # Ok::<(), manzana::Error>(())
    /// ```
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

    /// Whether manzana can provide unified — that is, GPU-visible — memory.
    ///
    /// **Always `false`, on every target.** Apple Silicon does have a unified
    /// memory architecture, but that is a fact about the chip, not about this
    /// crate: [`UmaBuffer`] is a host allocation no GPU can read, and manzana
    /// has no other memory to offer. Returning `true` on Apple Silicon would
    /// tell a caller it has zero-copy CPU/GPU sharing available when nothing
    /// here can deliver it.
    ///
    /// To ask about the chip instead, see
    /// [`crate::MetalDevice::has_unified_memory`] — itself inferred from the
    /// device name and build target, not queried.
    ///
    /// ```
    /// use manzana::unified_memory::UmaBuffer;
    ///
    /// // Including on an Apple Silicon Mac.
    /// assert!(!UmaBuffer::is_uma_available());
    /// ```
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
