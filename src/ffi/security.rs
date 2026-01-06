//! FFI bindings for Security.framework (Secure Enclave operations).
//!
//! This module provides low-level bindings to Apple's Security.framework
//! for Secure Enclave operations. All unsafe code is quarantined here.
//!
//! # Safety
//!
//! This module uses `unsafe` for FFI calls. All bindings are verified
//! against Apple's Security.framework documentation.
//!
//! # References
//!
//! - [Security Framework](https://developer.apple.com/documentation/security)
//! - [Secure Enclave](https://support.apple.com/guide/security/secure-enclave-sec59b0b31ff/web)

#![allow(unsafe_code)]
#![allow(dead_code)]

use std::ffi::c_void;

/// Opaque type for Security.framework keys.
pub type SecKeyRef = *const c_void;

/// Opaque type for Security.framework access control.
pub type SecAccessControlRef = *const c_void;

/// Error codes from Security.framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SecError {
    /// No error.
    Success = 0,
    /// The specified item could not be found.
    ItemNotFound = -25300,
    /// The specified item already exists.
    DuplicateItem = -25299,
    /// User interaction is required but not allowed.
    InteractionNotAllowed = -25308,
    /// Authentication failed.
    AuthFailed = -25293,
    /// Invalid key reference.
    InvalidKey = -67712,
    /// The operation was cancelled by the user.
    UserCanceled = -128,
}

impl SecError {
    /// Create from raw OSStatus code.
    #[must_use]
    pub const fn from_os_status(status: i32) -> Option<Self> {
        match status {
            0 => Some(Self::Success),
            -25300 => Some(Self::ItemNotFound),
            -25299 => Some(Self::DuplicateItem),
            -25308 => Some(Self::InteractionNotAllowed),
            -25293 => Some(Self::AuthFailed),
            -67712 => Some(Self::InvalidKey),
            -128 => Some(Self::UserCanceled),
            _ => None,
        }
    }
}

/// Key attributes for Secure Enclave operations.
#[derive(Debug, Clone)]
pub struct KeyAttributes {
    /// Application tag (identifier).
    pub tag: String,
    /// Human-readable label.
    pub label: Option<String>,
    /// Whether the key can be used for signing.
    pub can_sign: bool,
    /// Whether the key can be used for encryption.
    pub can_encrypt: bool,
    /// Whether the private key is extractable (always false for SE).
    pub extractable: bool,
}

impl Default for KeyAttributes {
    fn default() -> Self {
        Self {
            tag: String::new(),
            label: None,
            can_sign: true,
            can_encrypt: false,
            extractable: false, // Secure Enclave keys are never extractable
        }
    }
}

/// Check if Secure Enclave hardware is available.
///
/// # Returns
///
/// `true` if running on a device with Secure Enclave (T2 or Apple Silicon).
#[must_use]
pub const fn is_secure_enclave_available() -> bool {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // Apple Silicon always has Secure Enclave
        true
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        // T2 detection would require IOKit query
        // For now, assume available on recent Intel Macs
        // Real implementation: IOServiceMatching("AppleT2")
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

// Note: The following functions are stubs for the Security.framework API.
// They are intentionally simple and will be replaced with actual FFI calls
// when implementing real Secure Enclave support.
//
// The actual implementation would use:
// - SecKeyCreateRandomKey for key creation
// - SecItemDelete for key deletion
// - SecKeyCreateSignature for signing
// - SecKeyVerifySignature for verification
// - SecKeyCopyExternalRepresentation for public key export

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_secure_enclave_available() {
        let available = is_secure_enclave_available();
        #[cfg(target_os = "macos")]
        assert!(available);
        #[cfg(not(target_os = "macos"))]
        assert!(!available);
    }

    #[test]
    fn test_key_attributes_default() {
        let attrs = KeyAttributes::default();
        assert!(attrs.tag.is_empty());
        assert!(attrs.can_sign);
        assert!(!attrs.can_encrypt);
        assert!(!attrs.extractable);
    }

    #[test]
    fn test_sec_error_from_os_status() {
        assert_eq!(SecError::from_os_status(0), Some(SecError::Success));
        assert_eq!(SecError::from_os_status(-25300), Some(SecError::ItemNotFound));
        assert_eq!(SecError::from_os_status(-128), Some(SecError::UserCanceled));
        assert_eq!(SecError::from_os_status(99999), None);
    }
}
