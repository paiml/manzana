//! The FFI quarantine zone: foreign-function bindings and the unsafe code
//! that calls them.
//!
//! This module is private. Nothing in it is re-exported, so no type or
//! function documented here is part of manzana's public API. The public
//! surface it backs is [`crate::afterburner`].
//!
//! # Where unsafe code lives
//!
//! The crate root sets `#![deny(unsafe_code)]`. Two modules override it:
//! this one, and `src/unified_memory/mod.rs`, which needs `unsafe` for a
//! page-aligned host allocation and its RAII `Drop`. `deny` rather than
//! `forbid` is precisely because those two overrides exist — `forbid` cannot
//! be lifted. So "all unsafe code is in `src/ffi/`" would be false, and this
//! module makes no such claim; what is true is that all *foreign* calls are
//! here.
//!
//! # Module structure
//!
//! ```text
//! ffi/
//! ├── mod.rs          # this file: platform routing
//! └── iokit.rs        # IOKit bindings (macOS only): Afterburner discovery
//!                      and registry property reads
//! ```
//!
//! That is the whole directory. `mod.rs` selects between `iokit.rs` on macOS
//! and the inline non-macOS module below; there is no CoreML, Metal or
//! Security framework binding anywhere in manzana. `src/ffi/security.rs` was
//! deleted in 0.3.0 with the rest of the cryptography.
//!
//! # Safety rules (specification S1-S6), and where each one stands
//!
//! - **S1 — every `unsafe` block carries a `// SAFETY:` comment.** Holds. All
//!   six unsafe blocks in `iokit.rs` are annotated, and
//!   `clippy::undocumented_unsafe_blocks = "deny"` in `Cargo.toml` fails the
//!   build if one is not.
//! - **S2 — no raw pointer escapes this module.** Holds. `AfterburnerService`
//!   stores an `io_service_t` (a `u32` handle) in a private field, and
//!   `AfterburnerRawStats` is plain data. No pointer appears in any signature
//!   reachable from outside `ffi`. (`unified_memory` keeps a `NonNull<u8>`,
//!   but that is a separate module with its own invariants, not this one's.)
//! - **S3 — C strings are validated, not assumed.** Holds.
//!   `CString::new(...).ok()?` rejects an interior NUL before any call, and
//!   `CStr::to_str().ok()` turns non-UTF-8 registry names into `None` rather
//!   than a lossy or invalid `String`.
//! - **S4 — Core Foundation reference counts balance.** Holds, though not by
//!   the route the rule names: nothing here calls `CFRetain`. There are two
//!   owned references in total. The matching dictionary from
//!   `IOServiceMatching` is consumed by `IOServiceGetMatchingService` and must
//!   not be released; the property dictionary from
//!   `IORegistryEntryCreateCFProperties` arrives under the Create Rule and is
//!   handed to `CFDictionary::wrap_under_create_rule`, which releases it on
//!   drop. The `io_service_t` itself is released by `AfterburnerService::drop`.
//! - **S5 — no `transmute` without a size and alignment proof.** Holds
//!   vacuously: there is no `transmute` in this module.
//! - **S6 — thread safety is explicitly documented.** Holds on macOS, where
//!   `AfterburnerService` carries a `PhantomData<*const ()>` that makes it
//!   `!Send + !Sync`, matching IOKit's own thread-safety rules. It does *not*
//!   hold structurally on other targets: the fallback `AfterburnerService`
//!   below is a unit struct and is therefore `Send + Sync`. That difference is
//!   not reachable — `find_afterburner_service` returns `None` there, so no
//!   value of the type can be obtained — but the guarantee is a platform
//!   accident off macOS, not an enforced one.
//!
//! # No simulation
//!
//! Presence and statistics both come from live IOKit calls. Where a value
//! cannot be obtained, the code returns `None` or an error. **Nothing is
//! substituted.** `parse_afterburner_properties` reports every absent registry
//! property as `None`, and `crate::afterburner` turns a missing required
//! property into `Err` rather than a snapshot.
//!
//! Until 0.3.0 that was not so, and this section said so: it named
//! `parse_afterburner_properties` as "the one place a value is substituted
//! rather than reported missing".

// Allow unsafe in this module only - quarantine zone
#![allow(unsafe_code)]

#[cfg(target_os = "macos")]
pub mod iokit;

/// The IOKit surface on targets that have no IOKit.
///
/// Every entry point reports absence. Nothing here manufactures a statistic,
/// and no `unsafe` code is compiled on these targets.
#[cfg(not(target_os = "macos"))]
pub mod iokit {
    use crate::error::{Error, Subsystem};

    /// Always `None`: there is no IOKit on this platform to search.
    pub const fn find_afterburner_service() -> Option<AfterburnerService> {
        None
    }

    /// A service handle that cannot be obtained here.
    ///
    /// It exists so that [`crate::afterburner::AfterburnerMonitor`] type-checks
    /// on every target. `find_afterburner_service` never returns one, so no
    /// method on it is reachable through manzana's public API.
    pub struct AfterburnerService;

    impl AfterburnerService {
        /// Always `Err(NotAvailable)`.
        ///
        /// # Errors
        ///
        /// [`Error::NotAvailable`] for [`Subsystem::Afterburner`], always: the
        /// hardware cannot be present on a non-macOS target. It is deliberately
        /// not `Unimplemented` — the operation is implemented, the machine
        /// simply has no card.
        ///
        /// Unreachable via `find_afterburner_service`, which yields no value of
        /// this type; the crate's own tests construct one directly to exercise
        /// the refusal.
        #[allow(clippy::unused_self)]
        pub const fn get_stats(&self) -> Result<AfterburnerRawStats, Error> {
            Err(Error::not_available(Subsystem::Afterburner))
        }
    }

    /// Raw statistics as read from IOKit, before range checking.
    ///
    /// Mirrors the macOS definition field for field so that
    /// `crate::afterburner::convert_raw_stats` compiles on every target. No
    /// value of it is ever produced from hardware here.
    #[derive(Debug, Clone, Default)]
    pub struct AfterburnerRawStats {
        /// Active decode streams.
        pub streams_active: Option<u32>,
        /// Maximum concurrent stream capacity.
        pub streams_capacity: Option<u32>,
        /// FPGA utilization, unclamped.
        pub utilization: Option<f64>,
        /// Decode throughput in frames per second, unclamped.
        pub throughput_fps: Option<f64>,
        /// FPGA temperature in Celsius, if reported.
        pub temperature: Option<f64>,
        /// Power draw in watts, if reported.
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
