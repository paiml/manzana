//! Manzana: Safe Rust Interfaces for Apple Hardware
//!
//! Manzana provides safe, pure Rust interfaces to Apple hardware subsystems
//! for the Sovereign AI Stack. It enables on-premise, privacy-preserving
//! machine learning workloads on macOS.
//!
//! # Design Philosophy
//!
//! - **Iron Lotus Framework**: Toyota Production System principles applied to systems programming
//! - **Safe public API**: `#![deny(unsafe_code)]`, overridden only in
//!   `src/ffi/` and `src/unified_memory.rs`
//!
//! # Hardware support
//!
//! manzana ships no cryptography. Secure Enclave access was removed in
//! 0.3.0 -- see the Security section below. Use `security-framework`.
//!
//! Presence detection is implemented for the rows below. What you can *do*
//! with each is a separate question -- see the capability table in the README.
//! Operations that are not implemented return [`Error::Unimplemented`] rather
//! than a fabricated result.
//!
//! | Hardware | Module | Detection | Operations |
//! |----------|--------|-----------|------------|
//! | Afterburner FPGA | [`afterburner`] | Yes (IOKit) | Stats implemented |
//! | Metal GPU | `metal` | Yes (`system_profiler`) | Compute not implemented |
//! | Neural Engine | `neural_engine` | Build-target only | Inference not implemented |
//!
//! # Quick Start
//!
//! ```no_run
//! use manzana::afterburner::AfterburnerMonitor;
//!
//! // Check if Afterburner is available
//! if AfterburnerMonitor::is_available() {
//!     let monitor = AfterburnerMonitor::new().expect("just checked availability");
//!     let stats = monitor.stats().expect("failed to get stats");
//!     println!("Active streams: {}", stats.streams_active);
//!     println!("Utilization: {:.1}%", stats.utilization_percent);
//! } else {
//!     println!("Afterburner not available on this system");
//! }
//! ```
//!
//! # Feature Flags
//!
//! - `afterburner` - Enable Afterburner FPGA support (Mac Pro 2019+)
//! - `neural-engine` - Enable Neural Engine support (Apple Silicon)
//! - `metal` - Enable Metal GPU compute
//!
//! These flags are currently declared but gate nothing: no `#[cfg(feature)]`
//! exists in `src/`. The `secure-enclave` flag was removed in 0.3.0 with the
//! module it named.
//! - `full` - Enable all features
//!
//! # Safety Guarantees
//!
//! This crate uses `#![deny(unsafe_code)]` at the library level (not
//! `forbid`, which could not be overridden). All FFI
//! code is quarantined in the internal `ffi` module, which is not exported.
//!
//! # Error Handling
//!
//! All operations that can fail return [`Result<T, Error>`]. The [`Error`]
//! type provides specific variants for different failure modes, enabling
//! programmatic error handling.
//!
//! # Thread Safety
//!
//! Hardware monitors are `!Send` and `!Sync` because the underlying Apple
//! frameworks (IOKit, CoreML) are not thread-safe. Create monitors on each
//! thread that needs them, or use synchronization primitives if sharing.
//!
//! # Graceful Degradation
//!
//! On unsupported hardware, monitor constructors return `None` rather than
//! panicking. This allows applications to gracefully fall back to alternative
//! implementations.
//!
//! ```no_run
//! use manzana::afterburner::AfterburnerMonitor;
//!
//! let stats = match AfterburnerMonitor::new() {
//!     Some(monitor) => monitor.stats().ok(),
//!     None => None, // Graceful fallback
//! };
//! ```

// SAFETY: This crate denies unsafe code at the library level.
// All unsafe FFI code is quarantined in src/ffi/, which is not exported.
// We use deny (not forbid) so it can be overridden in the ffi module.
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

/// Library version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Check if we're running on macOS.
#[must_use]
pub const fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Check whether any Apple acceleration **hardware is present**.
///
/// Presence is not usability. On Apple Silicon this returns `true` because the
/// Neural Engine and Metal GPU are detected, yet every operation on them
/// returns [`Error::Unimplemented`] — so a caller branching on this is told it
/// has acceleration it cannot actually reach. See [`is_acceleration_usable`].
#[must_use]
pub fn is_acceleration_available() -> bool {
    afterburner::is_available()
        || neural_engine::is_available()
        || metal::is_available()
        || unified_memory::is_available()
}

/// Whether any accelerator can actually be *used* through manzana.
///
/// [`is_acceleration_available`] reports hardware **presence**. That is a
/// different question, and on Apple Silicon the two disagree: the Neural
/// Engine and Metal GPU are detected, but every operation on them returns
/// [`Error::Unimplemented`], so a caller branching on presence is told it has
/// acceleration it cannot reach.
///
/// This reports the question a caller usually means. Only Afterburner
/// statistics and host buffer allocation are implemented today, so it is
/// `true` only where those are.
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

    #[test]
    fn test_is_macos_consistent() {
        // This test just verifies the function works
        let _ = is_macos();
    }

    #[test]
    fn test_is_acceleration_available_no_panic() {
        // Should not panic on any platform
        let _ = is_acceleration_available();
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
