//! Type definitions for a future Security.framework binding.
//!
//! # ⚠️ This module contains no FFI ⚠️
//!
//! Despite its name and location, this file declares **no `extern "C"` blocks
//! and executes no `unsafe` code**. It holds a handful of type aliases, an
//! `OSStatus` mapping, and a struct. Nothing here calls Security.framework.
//!
//! The `#![allow(unsafe_code)]` below is therefore vestigial — it grants a
//! permission this module never exercises. It is retained only so the
//! quarantine boundary stays declared where the real bindings will land.
//!
//! Earlier README text described this file as part of an "FFI QUARANTINE ZONE
//! — Audited, MIRI-verified". No audit or MIRI run could have covered code
//! that does not exist, and the `make miri` target that nominally backed the
//! claim suppressed its own failures and could not fail. Both have been
//! corrected.
//!
//! Note also that `src/unified_memory.rs` carries its own
//! `#![allow(unsafe_code)]`, so unsafe code in this crate is not confined to
//! `src/ffi/` as previously documented.
//!
//! # Safety
//!
//! No safety argument is required today, because no `unsafe` operation is
//! performed. When real bindings are added, each must carry its own
//! `// SAFETY:` justification covering CFType ownership (the Create/Get rule),
//! `SecKeyRef` lifetime and `CFRelease` discipline, and null-pointer handling.
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
/// Always returns `false`: hardware detection is not implemented.
///
/// The previous implementation returned `true` for **all** macOS builds,
/// including `x86_64` hosts with no T2 chip, on the reasoning that T2
/// detection "would require an IOKit query". Guessing `true` on a machine
/// with no Secure Enclave is the least safe of the available answers.
///
/// A real implementation matches the `AppleSEPManager` service via
/// `IOServiceMatching` (or checks for `AppleT2` on Intel hosts) and reports
/// what it actually found.
#[must_use]
pub const fn is_secure_enclave_available() -> bool {
    false
}

// No Security.framework functions are bound yet. A real implementation would
// use:
// - SecKeyCreateRandomKey for key creation
// - SecItemDelete for key deletion
// - SecKeyCreateSignature for signing
// - SecKeyVerifySignature for verification
// - SecKeyCopyExternalRepresentation for public key export

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_secure_enclave_available_never_guesses() {
        // Ungated on purpose: the answer is the same everywhere, and the
        // previous cfg-split asserted `true` on macOS without ever checking
        // for the hardware.
        assert!(
            !is_secure_enclave_available(),
            "detection is unimplemented; it must report false rather than assume"
        );
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
        assert_eq!(
            SecError::from_os_status(-25300),
            Some(SecError::ItemNotFound)
        );
        assert_eq!(SecError::from_os_status(-128), Some(SecError::UserCanceled));
        assert_eq!(SecError::from_os_status(99999), None);
    }
}
