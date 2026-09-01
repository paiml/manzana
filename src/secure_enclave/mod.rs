//! Secure Enclave cryptographic operations.
//!
//! # ⚠️ NOT IMPLEMENTED — DO NOT USE FOR CRYPTOGRAPHY ⚠️
//!
//! **This module performs no cryptography and no Secure Enclave operations.**
//! Every operation returns [`Error::Unimplemented`]. The types below describe
//! the intended future API surface; there is no backend behind them.
//!
//! For real Secure Enclave and Keychain access today, use the
//! [`security-framework`](https://crates.io/crates/security-framework) crate.
//!
//! ## Why this module is in this state
//!
//! Versions 0.1.0 and 0.2.0 shipped stubs that returned fabricated values
//! while documenting themselves as real hardware-backed cryptography:
//! `create()` returned a fixed public key byte-summed from the key tag,
//! `sign()` returned 32 copies of a byte-sum of the message and of the tag,
//! and `verify()` re-derived that same value and compared it — accepting
//! forgeries from anyone who knew the tag.
//!
//! Both versions are **yanked** and are the subject of
//! [RUSTSEC-2026-0273](https://rustsec.org/advisories/RUSTSEC-2026-0273.html).
//! See [issue #3](https://github.com/paiml/manzana/issues/3) and
//! `docs/specifications/security-architecture-plan.md` for the full analysis.
//!
//! Rather than fix the fabricated values, this release removes them. An
//! operation that cannot reach real hardware must fail loudly; a
//! plausible-looking result is more dangerous, because a caller cannot tell
//! it apart from a genuine one.
//!
//! # Example
//!
//! ```
//! use manzana::secure_enclave::{SecureEnclaveSigner, KeyConfig};
//!
//! // Reports whether manzana can perform Secure Enclave operations:
//! // false in this release, on every platform.
//! assert!(!SecureEnclaveSigner::is_available());
//!
//! let err = SecureEnclaveSigner::create(KeyConfig::new("com.example.signing"))
//!     .expect_err("Secure Enclave support is not implemented");
//! assert!(err.is_unimplemented());
//! ```
//!
//! # Intended Security Model (not yet delivered)
//!
//! Implemented on `Security.framework`, keys would never leave the hardware
//! security module, would be device-bound, could require biometric
//! authentication, and would resist extraction by root. None of those
//! properties hold today, because no key is created.

mod types;

pub use types::{PublicKey, Signature};

use crate::error::{Error, Result};

/// Elliptic curve algorithm for Secure Enclave operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Algorithm {
    /// NIST P-256 (secp256r1) - the only algorithm supported by Secure Enclave.
    #[default]
    P256,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::P256 => write!(f, "P-256 (secp256r1)"),
        }
    }
}

/// Access control requirements for key usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AccessControl {
    /// Key can be used without additional authentication.
    #[default]
    None,
    /// Requires device passcode.
    DevicePasscode,
    /// Requires biometric authentication (Touch ID / Face ID).
    Biometric,
    /// Requires biometric OR passcode.
    BiometricOrPasscode,
}

impl std::fmt::Display for AccessControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::DevicePasscode => write!(f, "Device Passcode"),
            Self::Biometric => write!(f, "Biometric"),
            Self::BiometricOrPasscode => write!(f, "Biometric or Passcode"),
        }
    }
}

/// Configuration for creating a Secure Enclave key.
#[derive(Debug, Clone)]
pub struct KeyConfig {
    /// Application tag identifying the key (e.g., "com.example.app.signing").
    pub tag: String,
    /// Algorithm to use (only P-256 supported).
    pub algorithm: Algorithm,
    /// Access control requirements.
    pub access_control: AccessControl,
    /// Human-readable label for the key.
    pub label: Option<String>,
}

impl KeyConfig {
    /// Create a new key configuration with default settings.
    ///
    /// Uses P-256 algorithm with no access control requirements.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            algorithm: Algorithm::P256,
            access_control: AccessControl::None,
            label: None,
        }
    }

    /// Set the access control requirement.
    #[must_use]
    pub const fn with_access_control(mut self, access_control: AccessControl) -> Self {
        self.access_control = access_control;
        self
    }

    /// Set a human-readable label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Secure Enclave signer for P-256 ECDSA operations.
///
/// This type wraps a key stored in the Secure Enclave and provides
/// signing and verification operations.
///
/// # Thread Safety
///
/// This type is `!Send` and `!Sync` because Security.framework
/// operations are not thread-safe. Create signers on each thread.
pub struct SecureEnclaveSigner {
    tag: String,
    public_key: PublicKey,
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl std::fmt::Debug for SecureEnclaveSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecureEnclaveSigner")
            .field("tag", &self.tag)
            .field("public_key_len", &self.public_key.as_bytes().len())
            .finish_non_exhaustive()
    }
}

impl SecureEnclaveSigner {
    /// Check whether manzana can perform Secure Enclave operations.
    ///
    /// **Always returns `false` in this release**, on every platform,
    /// because no Secure Enclave backend is implemented.
    ///
    /// This reports *capability*, not *hardware presence*: it answers "can
    /// this library sign something for me right now?", which is the question
    /// a caller actually needs answered. Reporting the presence of hardware
    /// that manzana cannot reach would invite exactly the misuse that
    /// [issue #3](https://github.com/paiml/manzana/issues/3) described — and
    /// the previous implementation compounded it by returning `true`
    /// unconditionally on Intel macOS, where no T2 chip may exist at all.
    #[must_use]
    pub const fn is_available() -> bool {
        false
    }

    /// Create a new key in the Secure Enclave.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the new key
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release. No Secure
    /// Enclave backend exists, and this function will not fabricate a key.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::secure_enclave::{SecureEnclaveSigner, KeyConfig};
    ///
    /// let err = SecureEnclaveSigner::create(KeyConfig::new("com.example.signing"))
    ///     .expect_err("not implemented");
    /// assert!(err.is_unimplemented());
    /// ```
    // Taken by value deliberately: a real backend consumes the config.
    // (No `reason = ` here -- that lint-attribute field needs Rust 1.81 and
    // this crate's MSRV is 1.75.)
    #[allow(clippy::needless_pass_by_value)]
    pub fn create(config: KeyConfig) -> Result<Self> {
        drop(config);
        Err(Error::unimplemented(
            crate::error::Subsystem::SecureEnclave,
            "key creation (requires SecKeyCreateRandomKey)",
        ))
    }

    /// Load an existing key from the Secure Enclave.
    ///
    /// # Arguments
    ///
    /// * `tag` - The tag used when creating the key
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// Note that earlier versions returned [`Error::NotFound`] here, which was
    /// itself misleading: it implied a keychain had been searched and the key
    /// was genuinely absent, when in fact nothing was ever queried.
    pub fn load(tag: impl Into<String>) -> Result<Self> {
        let _ = tag.into();
        Err(Error::unimplemented(
            crate::error::Subsystem::SecureEnclave,
            "key lookup (requires SecItemCopyMatching)",
        ))
    }

    /// Delete the key from the Secure Enclave.
    ///
    /// # Warning
    ///
    /// This permanently deletes the private key. Any data encrypted
    /// or signed with this key will be unrecoverable.
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// Earlier versions returned `Ok(())` without deleting anything. Reporting
    /// success for a destructive security operation that did not occur is its
    /// own critical defect: a caller told "the key is destroyed" may go on to
    /// decommission a device or publish a revocation on that basis.
    pub fn delete(self) -> Result<()> {
        Err(Error::unimplemented(
            crate::error::Subsystem::SecureEnclave,
            "key deletion (requires SecItemDelete)",
        ))
    }

    /// Get the public key.
    #[must_use]
    pub const fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Get the key tag.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Sign data using the Secure Enclave private key.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to sign (will be SHA-256 hashed internally)
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// The removed implementation produced a DER-shaped value whose `r` was 32
    /// copies of a wrapping byte-sum of `data` and whose `s` was 32 copies of a
    /// wrapping byte-sum of the key tag — roughly 8 bits of entropy in each
    /// half, and trivially forgeable by anyone who knew the tag.
    pub fn sign(&self, data: &[u8]) -> Result<Signature> {
        let _ = data;
        Err(Error::unimplemented(
            crate::error::Subsystem::SecureEnclave,
            "signing (requires SecKeyCreateSignature)",
        ))
    }

    /// Verify a signature against data.
    ///
    /// # Arguments
    ///
    /// * `data` - Original data that was signed
    /// * `signature` - Signature to verify
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// This function never returns `Ok(false)` to mean "invalid signature",
    /// because it cannot distinguish valid from invalid. The removed
    /// implementation re-derived the fake signature and compared bytes, so it
    /// accepted forgeries and had no cryptographic meaning whatsoever.
    pub fn verify(&self, data: &[u8], signature: &Signature) -> Result<bool> {
        let _ = (data, signature);
        Err(Error::unimplemented(
            crate::error::Subsystem::SecureEnclave,
            "signature verification (requires SecKeyVerifySignature)",
        ))
    }
}

/// Check if Secure Enclave is available.
///
/// Convenience function equivalent to `SecureEnclaveSigner::is_available()`.
#[must_use]
pub const fn is_available() -> bool {
    SecureEnclaveSigner::is_available()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // F061: Secure Enclave detected on T2/Apple Silicon
    #[test]
    fn test_is_available_platform_detection() {
        // Capability, not hardware presence: manzana cannot perform Secure
        // Enclave operations in this release on any platform.
        assert!(
            !SecureEnclaveSigner::is_available(),
            "is_available() must report false while no backend is implemented"
        );
    }

    /// Every operation must fail loudly rather than fabricate a result.
    ///
    /// These tests are deliberately NOT gated on `target_os = "macos"`. The
    /// previous suite gated the entire cryptographic surface behind macOS, so
    /// the Linux test matrix for it was empty and CI stayed green no matter
    /// what those functions returned.
    #[test]
    fn test_create_is_unimplemented_not_fabricated() {
        let err = SecureEnclaveSigner::create(KeyConfig::new("com.manzana.test.creation"))
            .expect_err("create() must not manufacture a key");
        assert!(
            err.is_unimplemented(),
            "expected Unimplemented, got {err:?}"
        );
    }

    #[test]
    fn test_create_rejects_every_tag_including_empty() {
        // No tag, however well-formed, may yield a signer.
        for tag in ["", "com.manzana.test", "a", "com.example.app.signing"] {
            let err = SecureEnclaveSigner::create(KeyConfig::new(tag))
                .expect_err("create() must never succeed");
            assert!(err.is_unimplemented(), "tag {tag:?} produced {err:?}");
        }
    }

    #[test]
    fn test_load_is_unimplemented() {
        let err = SecureEnclaveSigner::load("com.manzana.nonexistent.key")
            .expect_err("load() must not succeed");
        assert!(
            err.is_unimplemented(),
            "load() must report Unimplemented rather than NotFound, which would \
             falsely imply a keychain was searched; got {err:?}"
        );
        assert!(!matches!(err, Error::NotFound { .. }));
    }

    /// The core refutation: no `SecureEnclaveSigner` can be obtained at all,
    /// so `sign`/`verify`/`delete` are unreachable through the public API.
    /// If this test ever fails, a construction path has been reintroduced and
    /// the sign/verify/delete guarantees below must be re-proven.
    #[test]
    fn test_no_construction_path_exists() {
        assert!(SecureEnclaveSigner::create(KeyConfig::new("x")).is_err());
        assert!(SecureEnclaveSigner::load("x").is_err());
    }

    #[test]
    fn test_public_key_rejects_fabricated_stub_key() {
        // The exact fake key v0.1.0/v0.2.0 produced for tag "test": an
        // all-zero X coordinate and an all-ones Y coordinate. It is a
        // well-formed 65-byte uncompressed point, so `PublicKey::from_bytes`
        // still accepts it structurally -- which is precisely why structural
        // validation was never enough to catch the defect.
        let mut fake = vec![0x04];
        fake.extend_from_slice(&[0u8; 32]);
        fake.extend_from_slice(&[1u8; 32]);
        let pk = PublicKey::from_bytes(fake).expect("structurally valid");
        assert_eq!(pk.as_bytes().len(), 65);
        // The point is not on the P-256 curve, but manzana does no curve
        // arithmetic, so it cannot say so. Documented as a known limitation.
    }

    #[test]
    fn test_public_key_structure() {
        // Valid P-256 uncompressed public key (65 bytes, starts with 0x04)
        let mut bytes = vec![0x04];
        bytes.extend_from_slice(&[0xAB; 32]); // X
        bytes.extend_from_slice(&[0xCD; 32]); // Y

        let pk = PublicKey::from_bytes(bytes).unwrap();
        assert_eq!(pk.as_bytes().len(), 65);
        assert_eq!(pk.x(), &[0xAB; 32]);
        assert_eq!(pk.y(), &[0xCD; 32]);
    }

    #[test]
    fn test_public_key_invalid_length() {
        let result = PublicKey::from_bytes(vec![0x04; 33]); // Wrong length
        assert!(result.is_err());
    }

    #[test]
    fn test_public_key_invalid_format() {
        let mut bytes = vec![0x02]; // Compressed format (not supported)
        bytes.extend_from_slice(&[0x00; 64]);

        let result = PublicKey::from_bytes(bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_validation() {
        // Too short
        let result = Signature::from_bytes(vec![0; 50]);
        assert!(result.is_err());

        // Too long
        let result = Signature::from_bytes(vec![0; 100]);
        assert!(result.is_err());

        // Empty
        let result = Signature::from_bytes(vec![]);
        assert!(result.is_err());

        // `vec![0x30; 70]` used to be accepted: the only invariant was
        // length. It is not DER and must now be rejected.
        assert!(Signature::from_bytes(vec![0x30; 70]).is_err());

        // A well-formed DER ECDSA-Sig-Value is accepted.
        assert!(Signature::from_bytes(der_sig(&[0x11; 32], &[0x22; 32])).is_ok());
    }

    /// Build a DER `ECDSA-Sig-Value` from raw big-endian r and s.
    fn der_sig(r: &[u8], s: &[u8]) -> Vec<u8> {
        fn int(v: &[u8]) -> Vec<u8> {
            let mut body = v.to_vec();
            if body[0] & 0x80 != 0 {
                body.insert(0, 0x00); // keep it positive
            }
            let mut out = vec![0x02, u8::try_from(body.len()).unwrap()];
            out.extend_from_slice(&body);
            out
        }
        let mut inner = int(r);
        inner.extend_from_slice(&int(s));
        let mut out = vec![0x30, u8::try_from(inner.len()).unwrap()];
        out.extend_from_slice(&inner);
        out
    }

    #[test]
    fn test_signature_rejects_the_shipped_forgery_shape() {
        // The exact shape 0.1.0/0.2.0 emitted: SEQUENCE(0x44) with r = 32
        // copies of a byte-sum and s = 32 copies of a byte-sum. This one IS
        // structurally valid DER -- structure alone was never going to catch
        // the fabrication, which is why sign() had to be removed rather than
        // validated. Recorded so the distinction is not lost.
        let forged = der_sig(&[0x07; 32], &[0x2a; 32]);
        assert!(
            Signature::from_bytes(forged).is_ok(),
            "the old forgery was well-formed DER; only removing sign() fixes it"
        );

        // But arbitrary garbage of the right length no longer passes.
        for junk in [
            vec![0x30; 70],
            vec![0xff; 70],
            vec![0x30, 0x44],
            vec![0u8; 70],
        ] {
            assert!(Signature::from_bytes(junk).is_err());
        }
    }

    #[test]
    fn test_algorithm_display() {
        assert_eq!(Algorithm::P256.to_string(), "P-256 (secp256r1)");
    }

    #[test]
    fn test_access_control_display() {
        assert_eq!(AccessControl::None.to_string(), "None");
        assert_eq!(AccessControl::DevicePasscode.to_string(), "Device Passcode");
        assert_eq!(AccessControl::Biometric.to_string(), "Biometric");
        assert_eq!(
            AccessControl::BiometricOrPasscode.to_string(),
            "Biometric or Passcode"
        );
    }

    #[test]
    fn test_convenience_function() {
        assert_eq!(is_available(), SecureEnclaveSigner::is_available());
    }

    // `test_signer_debug` and `test_public_key_extraction` were removed: both
    // obtained a signer from `create()`, which no longer manufactures one.
    // They are covered by `test_no_construction_path_exists` above, which
    // asserts the stronger property that no signer can be obtained at all.
    // They should return alongside a real backend, exercising a genuine key.
}
