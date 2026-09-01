// This module requires unsafe for memory allocation
#![allow(unsafe_code)]

//! Page-aligned host buffer allocation.
//!
//! [`UmaBuffer`] is a heap allocation made with [`std::alloc::alloc_zeroed`],
//! aligned to [`PAGE_SIZE`], freed on [`Drop`]. That is the whole of what this
//! module does.
//!
//! # It is not GPU memory
//!
//! Despite the name, a [`UmaBuffer`] is host memory and no GPU can read it.
//! This module creates no `MTLBuffer` and no `IOSurface`, and makes no Metal,
//! IOKit, or other framework call of any kind — it depends only on
//! [`std::alloc`] and [`crate::error`]. On Apple Silicon the CPU and GPU do
//! share physical memory, but a host allocation becomes GPU-visible only once
//! it is wrapped in a Metal buffer (`newBufferWithBytesNoCopy:`), and manzana
//! does not do that. Page alignment is a *precondition* for such a wrap, not
//! the wrap itself.
//!
//! Accordingly [`is_available`] returns `false` on every target, including
//! Apple Silicon.
//!
//! # Two different questions
//!
//! On an Apple Silicon Mac the `hardware_discovery` example prints
//! `UMA: Yes` in the Metal panel and `Unified Memory ... Not available` in the
//! panel below it. Both are the crate's honest answer to *different*
//! questions:
//!
//! - [`crate::MetalDevice::has_unified_memory`] is a guess at a **chip
//!   property**, inferred from the device name containing `"Apple"` or from
//!   the build target being `aarch64`. Nothing is queried.
//! - [`is_available`] reports a **manzana capability**: can this crate hand
//!   you memory both processors can address? It cannot, so it is `false`.
//!
//! # History
//!
//! Through 0.2.0 this module's documentation said the buffer was "accessible
//! to GPU without copying" and listed a falsification claim "F074: Zero-copy
//! verified". No such path existed to verify. `is_uma_available` also returned
//! `true` on `aarch64` macOS builds, reporting a capability from a
//! compile-time target check.
//!
//! # Examples
//!
//! ```
//! use manzana::unified_memory::{self, UmaBuffer, PAGE_SIZE};
//!
//! let mut buffer = UmaBuffer::new(1024 * 1024)?;
//!
//! // Freshly allocated memory reads back as zeros.
//! assert!(buffer.as_slice().iter().all(|&b| b == 0));
//!
//! // Write and read from the CPU. Only the CPU: this is host memory.
//! buffer.as_mut_slice()[0] = 42;
//! assert_eq!(buffer.as_slice()[0], 42);
//!
//! assert!(buffer.is_aligned());
//! assert_eq!(buffer.allocated_size() % PAGE_SIZE, 0);
//!
//! // manzana cannot provide GPU-visible memory anywhere.
//! assert!(!unified_memory::is_available());
//! # Ok::<(), manzana::Error>(())
//! ```
//!
//! # Falsification Claims
//!
//! - F071: Buffer allocation succeeds
//! - F076: Allocation is page-aligned (a prerequisite for a future Metal wrap)

/// Alignment used for every [`UmaBuffer`], in bytes.
///
/// This is a fixed constant, not a measurement: the crate never asks the
/// operating system for its page size (no `sysconf`, `getpagesize`, or
/// `vm_page_size` call exists in `src/`). On a host whose real page size is
/// larger than 4096, buffers are still aligned to 4096 and
/// [`UmaBuffer::is_aligned`] still returns `true`, but that is not a claim of
/// alignment to *that host's* page.
pub const PAGE_SIZE: usize = 4096;

/// Round `len` up to a [`PAGE_SIZE`] boundary, or `None` if that overflows.
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

/// Largest `len` [`UmaBuffer::new`] accepts: 16 GB, or `usize::MAX` on targets
/// too small to express 16 GB.
///
/// This is a policy limit chosen by this crate, not a query of available
/// memory. An allocation below it can still fail.
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

mod buffer;

pub use buffer::UmaBuffer;

/// Whether manzana can provide unified — that is, GPU-visible — memory.
///
/// Always `false`. Equivalent to [`UmaBuffer::is_uma_available`], which
/// documents why.
///
/// ```
/// assert!(!manzana::unified_memory::is_available());
/// ```
#[must_use]
#[allow(clippy::missing_const_for_fn)]
pub fn is_available() -> bool {
    UmaBuffer::is_uma_available()
}

#[cfg(test)]
mod tests;
