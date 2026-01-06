//! FFI Quarantine Zone - All unsafe code isolated here.
//!
//! # Safety Architecture
//!
//! This module contains ALL unsafe code in the manzana crate. The public API
//! in `src/lib.rs` uses `#![forbid(unsafe_code)]`, ensuring no unsafe code
//! can leak into the user-facing interface.
//!
//! ## Design Principles (Iron Lotus Framework)
//!
//! - **Poka-Yoke**: Type-safe wrappers prevent misuse at compile time
//! - **Jidoka**: All unsafe blocks have SAFETY comments
//! - **Genchi Genbutsu**: Direct hardware queries, no simulation
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

#[cfg(target_os = "macos")]
pub mod security;

// Stub modules for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub mod iokit {
    //! Stub IOKit module for non-macOS platforms.

    use crate::error::{Error, Subsystem};

    /// Stub: Always returns None on non-macOS.
    pub fn find_afterburner_service() -> Option<AfterburnerService> {
        None
    }

    /// Stub service type.
    pub struct AfterburnerService;

    impl AfterburnerService {
        /// Stub: Returns error on non-macOS.
        pub fn get_stats(&self) -> Result<AfterburnerRawStats, Error> {
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
mod tests {
    #[test]
    fn test_module_compiles() {
        // Verifies the module structure is correct
        // This test passes if compilation succeeds
        let _ = super::iokit::AfterburnerRawStats::default();
    }
}
