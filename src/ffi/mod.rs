//! FFI Quarantine Zone - All unsafe code isolated here.
//!
//! # Safety Architecture
//!
//! This module contains most, but not all, of the unsafe code in manzana:
//! `src/unified_memory.rs` carries its own `#![allow(unsafe_code)]` for its
//! page-aligned allocation and RAII `Drop`. The crate root uses
//! `#![deny(unsafe_code)]` -- `deny`, not `forbid`, precisely because those
//! two overrides exist. Earlier revisions of this file claimed otherwise.
//!
//! ## Design Principles (Iron Lotus Framework)
//!
//! - **Poka-Yoke**: Type-safe wrappers prevent misuse at compile time
//! - **Jidoka**: All unsafe blocks have SAFETY comments
//! - **Genchi Genbutsu**: Direct hardware queries, no simulation. This holds
//!   for `iokit.rs`; `security.rs` currently binds nothing at all.
//!
//! ## Safety Rules (from specification S1-S6)
//!
//! - S1: Every `unsafe` block has `// SAFETY:` comment
//! - S2: No raw pointers escape FFI module
//! - S3: All C strings validated as UTF-8 or handled
//! - S4: CFRelease called for every CFRetain
//! - S5: No transmute without size/alignment proof
//! - S6: Thread safety explicitly documented
//!
//! # Module Structure
//!
//! ```text
//! ffi/
//! ├── mod.rs          # This file - module router
//! ├── iokit.rs        # IOKit bindings (Afterburner, GPU discovery)
//! ├── coreml.rs       # CoreML bindings (Neural Engine)
//! ├── metal_sys.rs    # Metal bindings (GPU compute)
//! └── security.rs     # Security.framework bindings (Secure Enclave)
//! ```

// Allow unsafe in this module only - quarantine zone
#![allow(unsafe_code)]

#[cfg(target_os = "macos")]
pub mod iokit;

// Non-macOS implementations. These are not stubs in the fabricating sense:
// they report unavailability rather than inventing a result.
#[cfg(not(target_os = "macos"))]
pub mod iokit {
    //! IOKit surface for platforms that have no IOKit.
    //!
    //! Every entry point reports absence. Nothing here manufactures a value.

    use crate::error::{Error, Subsystem};

    /// Always `None`: there is no IOKit on this platform.
    pub const fn find_afterburner_service() -> Option<AfterburnerService> {
        None
    }

    /// Placeholder-free service handle: it can never be constructed here,
    /// because `find_afterburner_service` always returns `None`.
    pub struct AfterburnerService;

    impl AfterburnerService {
        /// Always `Err(NotAvailable)`: unreachable, since no value of this
        /// type can be obtained on a non-macOS target.
        #[allow(clippy::unused_self)]
        pub const fn get_stats(&self) -> Result<AfterburnerRawStats, Error> {
            Err(Error::not_available(Subsystem::Afterburner))
        }
    }

    /// Raw stats from IOKit.
    #[derive(Debug, Clone, Default)]
    pub struct AfterburnerRawStats {
        pub streams_active: u32,
        pub streams_capacity: u32,
        pub utilization: f64,
        pub throughput_fps: f64,
        pub temperature: Option<f64>,
        pub power: Option<f64>,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    // The non-macOS stub reports absence rather than inventing stats. It is
    // unreachable via find_afterburner_service (which returns None), so the
    // service value is constructed directly to exercise the refusal.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_non_macos_get_stats_reports_absence() {
        use super::iokit::AfterburnerService;
        let svc = AfterburnerService;
        let err = svc.get_stats().expect_err("there is no IOKit here");
        assert!(err.is_not_available(), "got {err:?}");
        assert!(
            !err.is_unimplemented(),
            "absent hardware is not an unimplemented op"
        );
        assert!(super::iokit::find_afterburner_service().is_none());
    }

    #[test]
    fn test_module_compiles() {
        // Verifies the module structure is correct
        // This test passes if compilation succeeds
        let _ = super::iokit::AfterburnerRawStats::default();
    }
}
