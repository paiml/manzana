//! Tests for the `unified_memory` module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;

#[test]
fn test_page_align_reports_overflow_instead_of_wrapping() {
    // Tested on `page_align` DIRECTLY, not through UmaBuffer::new. Going
    // through new() is vacuous on 64-bit: MAX_ALLOCATION is 16 GB there, so
    // every wrapping input is rejected by the bounds check first, and such
    // a test passes against the wrapping implementation too. It was
    // written that way first, and it did.
    //
    // The wrap it guards: `(len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)` gives
    // 0 for len near usize::MAX. alloc_zeroed(0) succeeds, and as_slice()
    // would then build a usize::MAX-length slice over a zero-sized
    // allocation -- out-of-bounds UB from safe code.
    for len in [usize::MAX, usize::MAX - 1, usize::MAX - PAGE_SIZE + 2] {
        assert_eq!(page_align(len), None, "len {len} must report overflow");
    }
    assert_eq!(page_align(0), Some(0));
    assert_eq!(page_align(1), Some(PAGE_SIZE));
    assert_eq!(page_align(PAGE_SIZE), Some(PAGE_SIZE));
    assert_eq!(page_align(PAGE_SIZE + 1), Some(PAGE_SIZE * 2));
}

#[test]
fn test_zeroed_constructor() {
    let b = UmaBuffer::zeroed(8192).expect("allocation");
    assert_eq!(b.len(), 8192);
    assert!(b.as_slice().iter().all(|&x| x == 0));
    assert!(b.is_aligned());
}

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
fn test_is_uma_available_reports_capability_not_chip_property() {
    // Ungated: the answer is the same on every target. The previous
    // version asserted `true` on aarch64 macOS, inferring a capability
    // from the build target, which cannot observe the running machine.
    assert!(
        !UmaBuffer::is_uma_available(),
        "manzana provides no GPU-visible memory, so this must be false \
         even on Apple Silicon, whose chip does have unified memory"
    );
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
