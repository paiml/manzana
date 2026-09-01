//! Hardware discovery for Apple accelerators on macOS.
//!
//! manzana reports which Apple accelerators are present on a machine, and for
//! the Afterburner FPGA it reports what that card is currently doing. Compute
//! is not implemented: Metal shader compilation, buffer allocation and
//! dispatch, and CoreML model loading and inference all return
//! [`Error::Unimplemented`].
//!
//! The crate is built on one rule: **an operation that cannot reach the
//! hardware it names returns [`Error::Unimplemented`] rather than a plausible
//! value.** Versions 0.1.0 and 0.2.0 did the opposite and were yanked; see
//! [Security](#security).
//!
//! # What is implemented
//!
//! | Hardware | Module | Presence detection | Operations *on the hardware* |
//! |----------|--------|--------------------|------------|
//! | Afterburner FPGA | [`afterburner`] | IOKit registry query | [`AfterburnerMonitor::stats`] implemented |
//! | Metal GPU | [`metal`] | `system_profiler SPDisplaysDataType` | none — every one returns [`Error::Unimplemented`] |
//! | Neural Engine | [`neural_engine`] | build target (`macos` + `aarch64`) | none — [`load`](NeuralEngineSession::load) and [`infer`](NeuralEngineSession::infer) return [`Error::Unimplemented`]; [`capabilities`](NeuralEngineSession::capabilities) returns `None` |
//! | Unified memory | [`unified_memory`] | none — [`unified_memory::is_available`] is always `false` | page-aligned **host** allocation ([`UmaBuffer`]) |
//!
//! Read the last column narrowly: it covers operations that would have to
//! reach the hardware. Plain data types that never touch a device do work, and
//! are not exceptions to it — [`Tensor`] really allocates and really validates
//! that `shape` and `data` agree (returning [`Error::InvalidInput`] when they
//! do not), and [`AfterburnerStats`]'s accessors read
//! fields of a snapshot you already hold. None of them queries anything.
//!
//! What each row does and does not establish:
//!
//! - **Afterburner.** Presence and [`AfterburnerStats`] come from real IOKit
//!   calls (`IOServiceMatching`, `IORegistryEntryCreateCFProperties`) in the
//!   private `ffi` module.
//! - **Metal.** [`MetalCompute::devices`] shells out to `system_profiler` and
//!   parses two things out of it: the device name and the VRAM figure. The
//!   remaining [`MetalDevice`] fields are not measurements —
//!   [`registry_id`](MetalDevice::registry_id) is the enumeration index plus
//!   one, [`max_threads_per_threadgroup`](MetalDevice::max_threads_per_threadgroup)
//!   is a hardcoded `1024`,
//!   [`is_headless`](MetalDevice::is_headless) is always `false`, and
//!   [`is_low_power`](MetalDevice::is_low_power) and
//!   [`has_unified_memory`](MetalDevice::has_unified_memory) are inferred from
//!   the device name and the build target. When no VRAM figure is reported,
//!   [`max_buffer_length`](MetalDevice::max_buffer_length) falls back to
//!   16 GiB on Apple Silicon and 4 GiB elsewhere, so
//!   [`vram_gb`](MetalDevice::vram_gb) can return a default rather than a
//!   reading. If `system_profiler` fails, the device list is empty; no device
//!   is invented.
//! - **Neural Engine.** [`neural_engine::is_available`] is a compile-time
//!   `cfg` check on `target_os = "macos"` and `target_arch = "aarch64"`, not a
//!   probe of the running machine. It is sound as a presence claim because
//!   every Apple Silicon part ships an ANE. Nothing else about the ANE is
//!   queried: [`NeuralEngineSession::capabilities`] returns `None`, and there
//!   is deliberately no `Default` on [`AneCapabilities`] for
//!   `capabilities().unwrap_or_default()` to reach — that line used to yield
//!   the M1's published 15.8 TOPS on any machine at all.
//! - **Unified memory.** [`UmaBuffer`] is a real page-aligned allocation made
//!   with [`std::alloc::alloc_zeroed`], freed on drop. It is not an
//!   `MTLBuffer`, it is not wrapped with `newBufferWithBytesNoCopy:`, and no
//!   GPU can read it.
//!
//! [`CompiledShader`], [`MetalBuffer`] and [`NeuralEngineSession`] are
//! re-exported but cannot be constructed through the public API in this
//! release, because the only functions that return them
//! ([`MetalCompute::compile_shader`], [`MetalCompute::allocate_buffer`],
//! [`NeuralEngineSession::load`]) always fail.
//!
//! # Presence is not usability
//!
//! [`is_acceleration_available`] answers "is an accelerator here?" and
//! [`is_acceleration_usable`] answers "can manzana operate one?". On Apple
//! Silicon they disagree, and both answers are correct: the Metal GPU and the
//! Neural Engine are detected, and every operation on them is unimplemented.
//!
//! The same split explains a pair of outputs that look contradictory. A device
//! from [`MetalCompute::devices`] can report
//! [`has_unified_memory`](MetalDevice::has_unified_memory) as `true` while
//! [`unified_memory::is_available`] returns `false`. The first is a claim about
//! the chip — Apple Silicon has a unified memory architecture. The second is a
//! claim about this crate — it cannot hand you a GPU-visible buffer — and it is
//! `false` on every platform.
//!
//! # Examples
//!
//! Discovery. These calls never panic and are safe to run on any target.
//!
//! ```
//! use manzana::{afterburner, metal, neural_engine, unified_memory};
//!
//! println!("Afterburner FPGA: {}", afterburner::is_available());
//! println!("Neural Engine:    {}", neural_engine::is_available());
//! println!("Metal GPU:        {}", metal::is_available());
//!
//! for device in metal::MetalCompute::devices() {
//!     // vram_gb divides by 2^30, so the unit is GiB despite the name.
//!     println!("{} — {:.1} GiB", device.name, device.vram_gb());
//! }
//!
//! // Always false: manzana cannot provide GPU-visible memory.
//! assert!(!unified_memory::is_available());
//! ```
//!
//! An unimplemented operation refuses rather than returning a value. This is
//! what a caller actually gets today:
//!
//! ```
//! use manzana::neural_engine::NeuralEngineSession;
//! use std::path::Path;
//!
//! let err = NeuralEngineSession::load(Path::new("model.mlmodelc"))
//!     .expect_err("CoreML model loading is not implemented");
//!
//! assert!(err.is_unimplemented());
//! assert_eq!(
//!     err.to_string(),
//!     "operation not implemented: CoreML model loading \
//!      (requires MLModel compileModelAtURL) (Neural Engine)",
//! );
//! ```
//!
//! Allocating a page-aligned host buffer, which works on every platform:
//!
//! ```
//! use manzana::unified_memory::UmaBuffer;
//!
//! let mut buffer = UmaBuffer::new(1024 * 1024)?;
//! buffer.as_mut_slice()[0] = 42;
//!
//! assert!(buffer.is_aligned());
//! assert_eq!(buffer.len(), 1024 * 1024);
//! # Ok::<(), manzana::Error>(())
//! ```
//!
//! # Error model
//!
//! Fallible operations return [`Result<T, Error>`](crate::error::Result).
//! Three variants carry most of the meaning, and they are deliberately
//! distinct:
//!
//! - [`Error::NotAvailable`] — the hardware is not on this machine.
//! - [`Error::Unimplemented`] — the hardware may well be present; manzana has
//!   no backend for the operation. Test it with
//!   [`Error::is_unimplemented`].
//! - [`Error::InvalidInput`] — the caller's arguments are wrong, independent
//!   of any hardware.
//!
//! Not every fallible entry point returns `Result`:
//! [`AfterburnerMonitor::new`] returns `Option<AfterburnerMonitor>` and yields
//! `None` when no Afterburner is found. [`MetalCompute::new`] and
//! [`MetalCompute::default_device`] do return `Result`.
//!
//! # Platform support
//!
//! The crate compiles and runs on any target. Off macOS there is no IOKit and
//! no `system_profiler`, so every detector reports absence:
//! [`afterburner::is_available`] and [`metal::is_available`] are `false`,
//! [`MetalCompute::devices`] is empty, and [`neural_engine::is_available`] is
//! `false`. [`UmaBuffer`] works everywhere, since it is a plain host
//! allocation. Nothing fabricates a device to fill the gap.
//!
//! # Feature flags
//!
//! `afterburner`, `neural-engine`, `metal` and `full` are declared in
//! `Cargo.toml` and gate nothing: there is no `#[cfg(feature = "...")]`
//! anywhere in `src/`, so every module is compiled whichever features you
//! enable. The `secure-enclave` flag was removed in 0.3.0 along with the
//! module it named.
//!
//! # Safety
//!
//! The crate root sets `#![deny(unsafe_code)]`. Two modules override it with
//! `#![allow(unsafe_code)]`: `src/ffi/`, which holds the IOKit bindings and is
//! private, and `src/unified_memory/mod.rs`, for [`UmaBuffer`]'s allocation and
//! `Drop`. The attribute is `deny` rather than `forbid` precisely because those
//! two overrides exist.
//!
//! # Thread safety
//!
//! - [`MetalCompute`] and [`NeuralEngineSession`] are `!Send` and `!Sync` on
//!   every target.
//! - [`AfterburnerMonitor`] is `!Send` and `!Sync` on macOS, where it holds an
//!   IOKit service handle. On other targets that handle is an empty type and
//!   the monitor is `Send + Sync` — but it can never be constructed there,
//!   because [`AfterburnerMonitor::new`] always returns `None`.
//! - [`UmaBuffer`] is `Send` and not `Sync`: it can be moved between threads,
//!   and concurrent access needs external synchronization.
//!
//! # Security
//!
//! **manzana ships no cryptography.** Do not use it for signing, verification,
//! key management, or attestation.
//!
//! Versions 0.1.0 and 0.2.0 are yanked ([RUSTSEC-2026-0273]). Their
//! `secure_enclave` module documented hardware-backed P-256 ECDSA while
//! `sign()` returned repeated byte-sums of the message and the key tag, and
//! `verify()` recomputed the same value and compared it — so it accepted
//! forgeries from anyone who knew the tag. The same release fabricated results
//! elsewhere: `neural_engine::infer()` returned a correctly shaped all-zero
//! tensor, and `metal::dispatch()` returned `Ok(())` having dispatched nothing.
//!
//! In 0.3.0 the `secure_enclave` module, `src/ffi/security.rs` and the
//! `secure-enclave` feature are deleted rather than repaired, and the
//! remaining fabricating operations return [`Error::Unimplemented`]. For real
//! Secure Enclave and Keychain access, use [`security-framework`].
//!
//! [RUSTSEC-2026-0273]: https://rustsec.org/advisories/RUSTSEC-2026-0273.html
//! [`security-framework`]: https://crates.io/crates/security-framework

// SAFETY: This crate denies unsafe code at the library level, and two modules
// override it: `ffi` (IOKit calls, not exported) and `unified_memory` (the
// alloc_zeroed/dealloc pair behind UmaBuffer). This comment used to say all
// unsafe was quarantined in src/ffi/, which the crate docs 40 lines above
// already contradicted.
// deny rather than forbid, so those two can override it.
#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::doc_markdown)] // Allow ProRes, IOKit, etc. without backticks

pub mod afterburner;
pub mod error;
pub mod metal;
pub mod neural_engine;
pub mod unified_memory;

// FFI module is internal only - not exported
mod ffi;

// Re-export main types for convenience
pub use afterburner::{AfterburnerMonitor, AfterburnerStats, ProResCodec};
pub use error::{Error, Result, Subsystem};
pub use metal::{CompiledShader, MetalBuffer, MetalCompute, MetalDevice};
pub use neural_engine::{AneCapabilities, AneOp, NeuralEngineSession, Tensor};
pub use unified_memory::UmaBuffer;

/// The version of this crate, from `CARGO_PKG_VERSION` at compile time.
///
/// ```
/// assert!(manzana::VERSION.starts_with("0."));
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Whether this build targets macOS.
///
/// This is `cfg!(target_os = "macos")`, decided when the crate is compiled. It
/// does not inspect the running machine.
///
/// ```
/// assert_eq!(manzana::is_macos(), cfg!(target_os = "macos"));
/// ```
#[must_use]
pub const fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Whether any Apple acceleration hardware is **present**.
///
/// Returns `true` if any of [`afterburner::is_available`],
/// [`neural_engine::is_available`], [`metal::is_available`] or
/// [`unified_memory::is_available`] is `true`. The last is always `false`, so
/// it never contributes.
///
/// Presence is not usability. On Apple Silicon this returns `true` because the
/// Neural Engine and the Metal GPU are detected, while every operation on
/// either returns [`Error::Unimplemented`]. To branch on what manzana can
/// actually do, use [`is_acceleration_usable`].
///
/// ```
/// // Detected hardware is not necessarily hardware you can drive.
/// if manzana::is_acceleration_available() && !manzana::is_acceleration_usable() {
///     println!("accelerators are present, but manzana cannot operate them");
/// }
/// ```
#[must_use]
pub fn is_acceleration_available() -> bool {
    afterburner::is_available()
        || neural_engine::is_available()
        || metal::is_available()
        || unified_memory::is_available()
}

/// Whether any accelerator can actually be *operated* through manzana.
///
/// Returns `afterburner::is_available() || unified_memory::is_available()`.
/// Because [`unified_memory::is_available`] is always `false`, this is exactly
/// Afterburner presence today: the FPGA is the only accelerator manzana can
/// both find and query.
///
/// [`UmaBuffer`] allocation works on every platform and is not counted here,
/// because [`unified_memory::is_available`] reports GPU-visible memory, which
/// this crate cannot provide.
///
/// ```
/// assert_eq!(
///     manzana::is_acceleration_usable(),
///     manzana::afterburner::is_available(),
/// );
/// ```
#[must_use]
pub fn is_acceleration_usable() -> bool {
    afterburner::is_available() || unified_memory::is_available()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_acceleration_presence_vs_usability() {
        // Presence and usability are different questions, and on Apple Silicon
        // they disagree: the ANE and GPU are detected, yet every operation on
        // them returns Unimplemented.
        let present = super::is_acceleration_available();
        let usable = super::is_acceleration_usable();
        if usable {
            assert!(present, "usable implies present");
        }
        // Usability follows only the genuinely implemented subsystems.
        assert_eq!(
            usable,
            super::afterburner::is_available() || super::unified_memory::is_available()
        );
    }

    use super::*;

    #[test]
    fn test_version_not_empty() {
        assert!(!VERSION.is_empty());
    }

    /// `is_macos()` must agree with the build target it is derived from.
    ///
    /// This was `let _ = is_macos();` with no assertion -- it passed against
    /// `true`, against `false`, and against any constant. A test that cannot
    /// fail is not evidence, which is the whole subject of this release.
    #[test]
    fn test_is_macos_matches_the_build_target() {
        assert_eq!(is_macos(), cfg!(target_os = "macos"));
    }

    /// Presence is the disjunction of the four subsystem predicates, and the
    /// unified-memory one never contributes because it is always `false`.
    ///
    /// Also `let _ = ...` before: it asserted nothing about the answer.
    #[test]
    fn test_acceleration_available_is_the_documented_disjunction() {
        let expected = afterburner::is_available()
            || neural_engine::is_available()
            || metal::is_available()
            || unified_memory::is_available();
        assert_eq!(is_acceleration_available(), expected);

        assert!(
            !unified_memory::is_available(),
            "the doc says this one never contributes; if it ever can, the \
             sentence above it stops being true"
        );

        // Presence is not usability -- the distinction this crate exists for.
        assert!(
            !is_acceleration_usable(),
            "manzana can drive none of it in 0.3.0"
        );
    }

    #[test]
    fn test_error_reexport() {
        let err = Error::not_available(Subsystem::Afterburner);
        assert!(err.is_not_available());
    }

    #[test]
    fn test_afterburner_reexport() {
        let stats = AfterburnerStats::default();
        assert!(!stats.is_active());
    }

    #[test]
    fn test_prores_codec_reexport() {
        let codec = ProResCodec::ProRes422;
        assert_eq!(codec.to_string(), "ProRes 422");
    }
}
