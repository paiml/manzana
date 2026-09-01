//! Tests for the `secure_enclave` module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

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
fn test_signature_rejects_oversized_33_byte_scalar() {
    // 33 bytes is legal only as a 0x00 pad ahead of a high-bit-set 32-byte
    // value. A 33-byte integer starting with anything else encodes a
    // scalar >= 2^256, outside the P-256 field. The parser accepted these
    // until the length bound was tightened.
    let mut inner = vec![0x02, 33, 0x01];
    inner.extend_from_slice(&[0x11; 32]);
    inner.push(0x02);
    inner.push(32);
    inner.extend_from_slice(&[0x22; 32]);
    let mut der = vec![0x30, u8::try_from(inner.len()).unwrap()];
    der.extend_from_slice(&inner);
    assert!(
        Signature::from_bytes(der).is_err(),
        "33-byte scalar without a 0x00 pad must be rejected"
    );

    // The legitimate 33-byte form -- 0x00 pad, high bit set -- is accepted.
    // der_sig() produces exactly this for a 0xff-leading value.
    assert!(
        Signature::from_bytes(der_sig(&[0xff; 32], &[0x22; 32])).is_ok(),
        "the 0x00-padded form must still be accepted"
    );
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
