//! Error types for manzana.
//!
//! [`enum@Error`] is the only error type this crate returns, and [`Result<T>`] is
//! the matching alias.
//!
//! # Choosing between variants
//!
//! Three variants cover nearly every failure a caller will see, and each calls
//! for a different response:
//!
//! | Variant | What happened | What to do |
//! |---|---|---|
//! | [`Error::NotAvailable`] | The hardware is not present on this machine. | Fall back. This is normal on most Macs, and on every non-Mac. |
//! | [`Error::Unimplemented`] | The hardware may well be present; manzana has no backend for the operation. | Use another crate for that operation. No machine will make it succeed. |
//! | [`Error::InvalidInput`] | The arguments were wrong. | Fix the call. |
//!
//! Conflating the first two costs a caller real behaviour: treating
//! `Unimplemented` as "wrong machine" sends a program looking for hardware
//! that is already installed, and treating `NotAvailable` as a bug reports a
//! fault on a machine that is working correctly. [`Error::is_not_available`]
//! and [`Error::is_unimplemented`] are the predicates to branch on.
//!
//! ```
//! use manzana::metal::MetalCompute;
//!
//! match MetalCompute::default_device() {
//!     Ok(gpu) => {
//!         // A GPU was enumerated. Compute on it is still not implemented.
//!         let err = gpu
//!             .compile_shader("kernel void k() {}", "k")
//!             .expect_err("shader compilation is not implemented in 0.3.0");
//!         assert!(err.is_unimplemented());
//!         assert!(!err.is_not_available());
//!     }
//!     Err(err) => {
//!         // No Metal device was enumerated — any non-macOS host, for one.
//!         assert!(err.is_not_available());
//!     }
//! }
//! ```
//!
//! # Which variants this crate produces
//!
//! Checked against the 0.3.0 sources:
//!
//! - [`Error::NotAvailable`] — [`MetalCompute::default_device`] when no device
//!   was enumerated, and Afterburner statistics on a non-macOS build.
//! - [`Error::IoKit`] — [`AfterburnerMonitor::stats`], from a failing
//!   `IORegistryEntryCreateCFProperties` or a registry entry that does not
//!   carry a property manzana needs.
//! - [`Error::InvalidInput`] — [`UmaBuffer::new`],
//!   [`UmaBuffer::copy_from_slice`], [`Tensor::new`], and
//!   [`NeuralEngineSession::load`] for a path that is not `.mlmodel` or
//!   `.mlmodelc`.
//! - [`Error::Internal`] — [`UmaBuffer::new`], when the host allocator fails.
//! - [`Error::NotFound`] — [`MetalCompute::new`] for a device index past the
//!   end of the enumerated list.
//! - [`Error::Unimplemented`] — Metal shader compilation, buffer allocation
//!   and dispatch; CoreML model loading and inference.
//!
//! [`Error::Metal`], [`Error::CoreMl`], [`Error::Timeout`] and
//! [`Error::PermissionDenied`] are constructed nowhere in this crate. Their
//! constructors are public and work, but no manzana operation returns one, so
//! a match arm expecting one from this crate is dead code.
//!
//! There is no variant for the Security framework. 0.3.0 deleted the
//! `secure_enclave` module and its Security FFI; manzana performs no
//! cryptography and makes no Security framework call. Code upgrading from
//! 0.2.0 that matched on `Error::Security` should use the
//! [`security-framework`](https://crates.io/crates/security-framework) crate
//! for those operations.
//!
//! [`MetalCompute::default_device`]: crate::metal::MetalCompute::default_device
//! [`MetalCompute::new`]: crate::metal::MetalCompute::new
//! [`AfterburnerMonitor::stats`]: crate::afterburner::AfterburnerMonitor::stats
//! [`UmaBuffer::new`]: crate::unified_memory::UmaBuffer::new
//! [`UmaBuffer::copy_from_slice`]: crate::unified_memory::UmaBuffer::copy_from_slice
//! [`Tensor::new`]: crate::neural_engine::Tensor::new
//! [`NeuralEngineSession::load`]: crate::neural_engine::NeuralEngineSession::load
//!
//! # Falsification Claims
//!
//! - F081: All errors implement std::error::Error
//! - F082: Error messages are human-readable
//! - F083: IOKit errors include kern_return_t
//! - F089: Error Display impl useful

use thiserror::Error;

/// The error type returned by every fallible operation in manzana.
///
/// `Error` is `Clone`, `PartialEq` and `Eq`, so a caller can compare a
/// returned error against an expected one directly. Every variant's `Display`
/// output names both the failure and its subject; see the module
/// documentation for which variants this crate actually produces.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The requested hardware is not present on this machine.
    ///
    /// A statement about the machine, not about manzana. It is a normal
    /// condition — Afterburner on anything but a Mac Pro with the card fitted,
    /// Metal on a host where no GPU could be enumerated — and a caller is
    /// expected to fall back rather than treat it as a fault.
    ///
    /// Distinct from [`Error::Unimplemented`], which says the hardware may be
    /// present and manzana cannot drive it.
    #[error("hardware not available: {subsystem}")]
    NotAvailable {
        /// The hardware subsystem that was requested.
        subsystem: Subsystem,
    },

    /// An IOKit call failed.
    ///
    /// `code` is the `kern_return_t` from the failing call, preserved so a
    /// caller can act on it rather than only print it. It is `0`
    /// (`KERN_SUCCESS`) where no IOKit call actually failed but the data could
    /// not be read anyway — the call returned a null property dictionary, or
    /// the registry entry carries none of the property manzana was asked for.
    /// `message` says which.
    ///
    /// Produced only by Afterburner statistics
    /// ([`AfterburnerMonitor::stats`](crate::afterburner::AfterburnerMonitor::stats)).
    #[error("IOKit error (code {code}): {message}")]
    IoKit {
        /// The kern_return_t error code.
        code: i32,
        /// Human-readable error message.
        message: String,
    },

    /// The Metal framework reported an error.
    ///
    /// No manzana operation returns this: the crate makes no Metal API call.
    /// Device enumeration shells out to `system_profiler` and reports "no
    /// devices" as an empty list, and every Metal compute operation returns
    /// [`Error::Unimplemented`]. The variant is kept for a future backend.
    #[error("Metal error: {message}")]
    Metal {
        /// Human-readable error message.
        message: String,
    },

    /// The CoreML framework reported an error.
    ///
    /// No manzana operation returns this: the crate makes no CoreML call.
    /// Model loading and inference return [`Error::Unimplemented`]. The
    /// variant is kept for a future backend.
    #[error("CoreML error: {message}")]
    CoreMl {
        /// Human-readable error message.
        message: String,
    },

    /// The arguments to an API were wrong.
    ///
    /// A caller-side bug: a zero-length or oversized buffer request, a source
    /// slice longer than its destination, a tensor whose data length disagrees
    /// with its shape, or a model path whose extension is neither `.mlmodel`
    /// nor `.mlmodelc`. Retrying unchanged — on any machine — fails the same
    /// way.
    #[error("invalid input: {reason}")]
    InvalidInput {
        /// Description of what was invalid.
        reason: String,
    },

    /// An operation exceeded its deadline.
    ///
    /// No manzana operation returns this: nothing in the crate waits on
    /// hardware with a deadline.
    #[error("operation timed out after {duration_ms}ms")]
    Timeout {
        /// How long we waited before timing out.
        duration_ms: u64,
    },

    /// The operation was refused for permission reasons.
    ///
    /// No manzana operation returns this. An IOKit call that fails, for
    /// whatever reason, surfaces as [`Error::IoKit`] carrying the underlying
    /// `kern_return_t`.
    #[error("permission denied: {operation}")]
    PermissionDenied {
        /// The operation that was denied.
        operation: String,
    },

    /// A lookup was performed and found nothing.
    ///
    /// Produced by
    /// [`MetalCompute::new`](crate::metal::MetalCompute::new) for a device
    /// index past the end of the enumerated device list.
    #[error("resource not found: {resource}")]
    NotFound {
        /// Description of the missing resource.
        resource: String,
    },

    /// An invariant inside manzana did not hold.
    ///
    /// Produced by
    /// [`UmaBuffer::new`](crate::unified_memory::UmaBuffer::new) when the host
    /// allocator returns null or rejects the computed
    /// [`Layout`](std::alloc::Layout). It is not a caller error; a
    /// reproducible occurrence is worth reporting.
    #[error("internal error: {details}")]
    Internal {
        /// Details about the internal error.
        details: String,
    },

    /// manzana has no backend for the operation.
    ///
    /// The hardware may be present and working — this says nothing about the
    /// machine, only that manzana cannot drive it. `Display` names the
    /// operation and the Apple API a real implementation would have to call:
    ///
    /// ```text
    /// operation not implemented: shader compilation (requires MTLDevice::newLibraryWithSource) (Metal GPU)
    /// ```
    ///
    /// Returned unconditionally in 0.3.0 by Metal shader compilation, buffer
    /// allocation and dispatch, and by CoreML model loading and inference. No
    /// machine, macOS version or feature flag makes them succeed.
    ///
    /// manzana returns this rather than a value that resembles a real result.
    /// Versions 0.1.0 and 0.2.0 returned fabricated results from these
    /// operations and were yanked (RUSTSEC-2026-0273).
    #[error("operation not implemented: {operation} ({subsystem})")]
    Unimplemented {
        /// Subsystem the operation belongs to.
        subsystem: Subsystem,
        /// The operation that is not implemented.
        operation: String,
    },
}

mod subsystem;

pub use subsystem::Subsystem;

/// The result type returned by fallible manzana operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Creates a [`Error::NotAvailable`] for `subsystem`.
    ///
    /// Use it when the hardware genuinely is not present. It is a claim about
    /// the machine, so it is the wrong error for "manzana has not implemented
    /// this" — that is [`Error::unimplemented`], and conflating the two hides
    /// which one actually happened.
    ///
    /// ```
    /// use manzana::error::{Error, Subsystem};
    ///
    /// let e = Error::not_available(Subsystem::Metal);
    /// assert!(e.is_not_available());
    /// assert!(!e.is_unimplemented());
    /// assert_eq!(e.to_string(), "hardware not available: Metal GPU");
    /// ```
    #[must_use]
    pub const fn not_available(subsystem: Subsystem) -> Self {
        Self::NotAvailable { subsystem }
    }

    /// Creates an [`Error::IoKit`] from a `kern_return_t` and a message.
    ///
    /// The code is preserved verbatim so a caller can match on it rather than
    /// only print it.
    ///
    /// ```
    /// use manzana::error::Error;
    ///
    /// let e = Error::iokit(-536_870_206, "service not found");
    /// assert_eq!(e.error_code(), Some(-536_870_206));
    /// assert_eq!(
    ///     e.to_string(),
    ///     "IOKit error (code -536870206): service not found"
    /// );
    /// ```
    #[must_use]
    pub fn iokit(code: i32, message: impl Into<String>) -> Self {
        Self::IoKit {
            code,
            message: message.into(),
        }
    }

    /// Creates an [`Error::Metal`].
    ///
    /// Nothing in manzana calls this: the crate makes no Metal API call, and
    /// unimplemented Metal operations return [`Error::unimplemented`].
    #[must_use]
    pub fn metal(message: impl Into<String>) -> Self {
        Self::Metal {
            message: message.into(),
        }
    }

    /// Creates an [`Error::CoreMl`].
    ///
    /// Nothing in manzana calls this: the crate makes no CoreML call, and
    /// unimplemented Neural Engine operations return [`Error::unimplemented`].
    #[must_use]
    pub fn coreml(message: impl Into<String>) -> Self {
        Self::CoreMl {
            message: message.into(),
        }
    }

    /// Creates an [`Error::InvalidInput`] with a reason.
    ///
    /// For a caller's mistake, as opposed to a missing backend
    /// ([`Error::unimplemented`]) or absent hardware
    /// ([`Error::not_available`]). State what was wrong with the argument, not
    /// what the function wanted to do.
    ///
    /// ```
    /// use manzana::error::Error;
    /// use manzana::unified_memory::UmaBuffer;
    ///
    /// let err = UmaBuffer::new(0).expect_err("a zero-length buffer is rejected");
    /// assert_eq!(err, Error::invalid_input("buffer length cannot be zero"));
    /// assert!(!err.is_unimplemented());
    /// assert_eq!(err.error_code(), None);
    /// ```
    #[must_use]
    pub fn invalid_input(reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            reason: reason.into(),
        }
    }

    /// Creates an [`Error::Timeout`] for a wait of `duration_ms` milliseconds.
    ///
    /// Nothing in manzana calls this; no operation in the crate waits on
    /// hardware with a deadline.
    ///
    /// ```
    /// use manzana::error::Error;
    ///
    /// let e = Error::timeout(5_000);
    /// assert!(e.is_timeout());
    /// assert_eq!(e.to_string(), "operation timed out after 5000ms");
    /// ```
    #[must_use]
    pub const fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    /// Creates an [`Error::PermissionDenied`] naming the refused operation.
    ///
    /// Nothing in manzana calls this; a failing IOKit call is reported as
    /// [`Error::IoKit`] with its `kern_return_t`.
    #[must_use]
    pub fn permission_denied(operation: impl Into<String>) -> Self {
        Self::PermissionDenied {
            operation: operation.into(),
        }
    }

    /// Creates an [`Error::NotFound`] describing what was looked for.
    ///
    /// It asserts that a lookup happened and came back empty, so do not use it
    /// for an operation that never looked — that is [`Error::unimplemented`].
    ///
    /// ```
    /// use manzana::error::Error;
    ///
    /// let e = Error::not_found("Metal device index 4 (only 1 devices available)");
    /// assert!(!e.is_unimplemented());
    /// assert!(e.to_string().starts_with("resource not found:"));
    /// ```
    #[must_use]
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Creates an [`Error::Internal`] describing a broken invariant.
    ///
    /// For failures that are manzana's own fault and that a caller cannot fix
    /// by changing its arguments or its hardware.
    #[must_use]
    pub fn internal(details: impl Into<String>) -> Self {
        Self::Internal {
            details: details.into(),
        }
    }

    /// Creates an [`Error::Unimplemented`] naming the subsystem and operation.
    ///
    /// This is the crate's governing rule: an operation that cannot reach the
    /// hardware it claims to use returns this, never a value that resembles a
    /// real result. Name the missing Apple API in `operation` where one exists,
    /// so a reader can see what is required rather than only that something is
    /// absent.
    ///
    /// ```
    /// use manzana::error::{Error, Subsystem};
    ///
    /// let e = Error::unimplemented(Subsystem::Metal, "compute dispatch");
    /// assert!(e.is_unimplemented());
    /// // Distinct from "the hardware is missing": the GPU may well be there.
    /// assert!(!e.is_not_available());
    /// assert_eq!(
    ///     e.to_string(),
    ///     "operation not implemented: compute dispatch (Metal GPU)"
    /// );
    /// ```
    #[must_use]
    pub fn unimplemented(subsystem: Subsystem, operation: impl Into<String>) -> Self {
        Self::Unimplemented {
            subsystem,
            operation: operation.into(),
        }
    }

    /// Returns `true` if the hardware is absent from this machine.
    ///
    /// The signal to fall back to another implementation. It says nothing
    /// about whether manzana could drive the hardware if it were present; see
    /// [`Error::is_unimplemented`].
    ///
    /// ```
    /// use manzana::error::{Error, Subsystem};
    ///
    /// assert!(Error::not_available(Subsystem::Afterburner).is_not_available());
    /// assert!(!Error::unimplemented(Subsystem::Metal, "dispatch").is_not_available());
    /// ```
    #[must_use]
    pub const fn is_not_available(&self) -> bool {
        matches!(self, Self::NotAvailable { .. })
    }

    /// Returns `true` if manzana has no backend for the operation.
    ///
    /// The predicate to branch on to tell "manzana cannot do this, on any
    /// machine" from every other failure — including from "your machine does
    /// not have this hardware" ([`Error::is_not_available`]).
    ///
    /// ```
    /// use manzana::error::{Error, Subsystem};
    ///
    /// assert!(Error::unimplemented(Subsystem::NeuralEngine, "inference").is_unimplemented());
    /// assert!(!Error::invalid_input("bad shape").is_unimplemented());
    /// assert!(!Error::not_available(Subsystem::Afterburner).is_unimplemented());
    /// ```
    #[must_use]
    pub const fn is_unimplemented(&self) -> bool {
        matches!(self, Self::Unimplemented { .. })
    }

    /// Returns `true` if this is an [`Error::Timeout`].
    ///
    /// Never true for an error returned by manzana 0.3.0; no operation in the
    /// crate times out.
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Returns `true` if this is an [`Error::PermissionDenied`].
    ///
    /// Never true for an error returned by manzana 0.3.0; the crate never maps
    /// a failure onto this variant.
    #[must_use]
    pub const fn is_permission_denied(&self) -> bool {
        matches!(self, Self::PermissionDenied { .. })
    }

    /// Returns the underlying OS error code, if the variant carries one.
    ///
    /// `Some` only for [`Error::IoKit`], which carries the `kern_return_t`
    /// returned by the failing IOKit call. `None` for every other variant —
    /// including every variant whose failure originated inside manzana rather
    /// than in an Apple framework.
    ///
    /// ```
    /// use manzana::error::{Error, Subsystem};
    ///
    /// assert_eq!(Error::iokit(-536_870_206, "service not found").error_code(),
    ///            Some(-536_870_206));
    /// assert_eq!(Error::unimplemented(Subsystem::Metal, "dispatch").error_code(), None);
    /// ```
    #[must_use]
    pub const fn error_code(&self) -> Option<i32> {
        match self {
            Self::IoKit { code, .. } => Some(*code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
