//! Error types for Manzana.
//!
//! All errors implement `std::error::Error` and provide human-readable messages.
//! Error variants are specific enough to allow programmatic handling.
//!
//! # Falsification Claims
//! - F081: All errors implement std::error::Error
//! - F082: Error messages are human-readable
//! - F083: IOKit errors include kern_return_t
//! - F089: Error Display impl useful

use std::fmt;
use thiserror::Error;

/// Primary error type for Manzana operations.
///
/// Each variant provides sufficient context for debugging while remaining
/// actionable for programmatic error handling.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Hardware is not available on this system.
    ///
    /// This is a normal condition on unsupported hardware (e.g., Afterburner
    /// on non-Mac Pro systems). Applications should handle this gracefully.
    #[error("hardware not available: {subsystem}")]
    NotAvailable {
        /// The hardware subsystem that was requested.
        subsystem: Subsystem,
    },

    /// IOKit framework returned an error.
    ///
    /// Contains the kern_return_t code for debugging.
    #[error("IOKit error (code {code}): {message}")]
    IoKit {
        /// The kern_return_t error code.
        code: i32,
        /// Human-readable error message.
        message: String,
    },

    /// Metal framework returned an error.
    #[error("Metal error: {message}")]
    Metal {
        /// Human-readable error message.
        message: String,
    },

    /// CoreML framework returned an error.
    #[error("CoreML error: {message}")]
    CoreMl {
        /// Human-readable error message.
        message: String,
    },

    /// Security framework returned an error.
    #[error("Security framework error (code {code})")]
    Security {
        /// The OSStatus error code.
        code: i32,
    },

    /// Invalid input was provided to an API.
    #[error("invalid input: {reason}")]
    InvalidInput {
        /// Description of what was invalid.
        reason: String,
    },

    /// Operation timed out.
    #[error("operation timed out after {duration_ms}ms")]
    Timeout {
        /// How long we waited before timing out.
        duration_ms: u64,
    },

    /// Permission denied for the requested operation.
    #[error("permission denied: {operation}")]
    PermissionDenied {
        /// The operation that was denied.
        operation: String,
    },

    /// Resource was not found.
    #[error("resource not found: {resource}")]
    NotFound {
        /// Description of the missing resource.
        resource: String,
    },

    /// Internal error (should not occur in normal operation).
    #[error("internal error: {details}")]
    Internal {
        /// Details about the internal error.
        details: String,
    },
}

/// Hardware subsystems supported by Manzana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subsystem {
    /// Apple Afterburner FPGA (Mac Pro 2019+).
    Afterburner,
    /// Apple Neural Engine (Apple Silicon).
    NeuralEngine,
    /// Metal GPU compute.
    Metal,
    /// Secure Enclave (T2/Apple Silicon).
    SecureEnclave,
    /// Unified Memory Architecture.
    UnifiedMemory,
}

impl fmt::Display for Subsystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Afterburner => write!(f, "Afterburner FPGA"),
            Self::NeuralEngine => write!(f, "Neural Engine"),
            Self::Metal => write!(f, "Metal GPU"),
            Self::SecureEnclave => write!(f, "Secure Enclave"),
            Self::UnifiedMemory => write!(f, "Unified Memory"),
        }
    }
}

/// Result type alias for Manzana operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a new `NotAvailable` error.
    #[must_use]
    pub const fn not_available(subsystem: Subsystem) -> Self {
        Self::NotAvailable { subsystem }
    }

    /// Create a new `IoKit` error from a kern_return_t code.
    #[must_use]
    pub fn iokit(code: i32, message: impl Into<String>) -> Self {
        Self::IoKit {
            code,
            message: message.into(),
        }
    }

    /// Create a new `Metal` error.
    #[must_use]
    pub fn metal(message: impl Into<String>) -> Self {
        Self::Metal {
            message: message.into(),
        }
    }

    /// Create a new `CoreMl` error.
    #[must_use]
    pub fn coreml(message: impl Into<String>) -> Self {
        Self::CoreMl {
            message: message.into(),
        }
    }

    /// Create a new `Security` error.
    #[must_use]
    pub const fn security(code: i32) -> Self {
        Self::Security { code }
    }

    /// Create a new `InvalidInput` error.
    #[must_use]
    pub fn invalid_input(reason: impl Into<String>) -> Self {
        Self::InvalidInput {
            reason: reason.into(),
        }
    }

    /// Create a new `Timeout` error.
    #[must_use]
    pub const fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    /// Create a new `PermissionDenied` error.
    #[must_use]
    pub fn permission_denied(operation: impl Into<String>) -> Self {
        Self::PermissionDenied {
            operation: operation.into(),
        }
    }

    /// Create a new `NotFound` error.
    #[must_use]
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create a new `Internal` error.
    #[must_use]
    pub fn internal(details: impl Into<String>) -> Self {
        Self::Internal {
            details: details.into(),
        }
    }

    /// Check if this error indicates hardware is unavailable.
    #[must_use]
    pub const fn is_not_available(&self) -> bool {
        matches!(self, Self::NotAvailable { .. })
    }

    /// Check if this error is a timeout.
    #[must_use]
    pub const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Check if this error is a permission issue.
    #[must_use]
    pub const fn is_permission_denied(&self) -> bool {
        matches!(self, Self::PermissionDenied { .. })
    }

    /// Get the error code if this is an IOKit or Security error.
    #[must_use]
    pub const fn error_code(&self) -> Option<i32> {
        match self {
            Self::IoKit { code, .. } | Self::Security { code } => Some(*code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // F081: All errors implement std::error::Error
    #[test]
    fn test_error_implements_std_error() {
        fn assert_std_error<T: std::error::Error>() {}
        assert_std_error::<Error>();
    }

    // F082: Error messages are human-readable
    #[test]
    fn test_error_messages_are_readable() {
        let err = Error::not_available(Subsystem::Afterburner);
        let msg = err.to_string();
        assert!(msg.contains("Afterburner"));
        assert!(msg.contains("not available"));
    }

    // F083: IOKit errors include kern_return_t
    #[test]
    fn test_iokit_error_includes_code() {
        let err = Error::iokit(-536_870_206, "service not found");
        let msg = err.to_string();
        assert!(msg.contains("-536870206"));
        assert!(msg.contains("service not found"));
    }

    // F089: Error Display impl useful
    #[test]
    fn test_display_impl_not_generic() {
        let errors = vec![
            Error::not_available(Subsystem::Metal),
            Error::iokit(0, "test"),
            Error::metal("test"),
            Error::coreml("test"),
            Error::security(-1),
            Error::invalid_input("test"),
            Error::timeout(1000),
            Error::permission_denied("test"),
            Error::not_found("test"),
            Error::internal("test"),
        ];

        for err in errors {
            let msg = err.to_string();
            // No generic "Error" only messages
            assert!(msg.len() > 10, "Message too short: {msg}");
            assert!(!msg.eq_ignore_ascii_case("error"), "Generic message: {msg}");
        }
    }

    #[test]
    fn test_subsystem_display() {
        assert_eq!(Subsystem::Afterburner.to_string(), "Afterburner FPGA");
        assert_eq!(Subsystem::NeuralEngine.to_string(), "Neural Engine");
        assert_eq!(Subsystem::Metal.to_string(), "Metal GPU");
        assert_eq!(Subsystem::SecureEnclave.to_string(), "Secure Enclave");
        assert_eq!(Subsystem::UnifiedMemory.to_string(), "Unified Memory");
    }

    #[test]
    fn test_error_constructors() {
        let _ = Error::not_available(Subsystem::Afterburner);
        let _ = Error::iokit(0, "msg");
        let _ = Error::metal("msg");
        let _ = Error::coreml("msg");
        let _ = Error::security(0);
        let _ = Error::invalid_input("msg");
        let _ = Error::timeout(100);
        let _ = Error::permission_denied("op");
        let _ = Error::not_found("res");
        let _ = Error::internal("details");
    }

    #[test]
    fn test_error_predicates() {
        assert!(Error::not_available(Subsystem::Metal).is_not_available());
        assert!(!Error::timeout(100).is_not_available());

        assert!(Error::timeout(100).is_timeout());
        assert!(!Error::not_available(Subsystem::Metal).is_timeout());

        assert!(Error::permission_denied("op").is_permission_denied());
        assert!(!Error::timeout(100).is_permission_denied());
    }

    #[test]
    fn test_error_code_extraction() {
        assert_eq!(Error::iokit(42, "test").error_code(), Some(42));
        assert_eq!(Error::security(-1).error_code(), Some(-1));
        assert_eq!(Error::metal("test").error_code(), None);
    }

    #[test]
    fn test_error_equality() {
        let e1 = Error::not_available(Subsystem::Afterburner);
        let e2 = Error::not_available(Subsystem::Afterburner);
        let e3 = Error::not_available(Subsystem::Metal);

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_error_clone() {
        let e1 = Error::iokit(42, "test message");
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_error_debug() {
        let err = Error::not_available(Subsystem::Afterburner);
        let debug = format!("{err:?}");
        assert!(debug.contains("NotAvailable"));
        assert!(debug.contains("Afterburner"));
    }
}
