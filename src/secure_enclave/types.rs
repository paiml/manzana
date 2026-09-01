//! Key and signature value types for the Secure Enclave API.
//!
//! These types are pure data with no platform dependency, so they compile and
//! are tested on every target -- deliberately, since gating the cryptographic
//! surface behind `cfg(target_os = "macos")` is what let the fabricated
//! implementations ship with a green Linux CI lane.

use crate::error::{Error, Result};

/// A P-256 ECDSA signature from the Secure Enclave.
///
/// The signature is in DER format as returned by Security.framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Raw signature bytes (DER-encoded).
    bytes: Vec<u8>,
}

/// Parse a DER `ECDSA-Sig-Value` and return the `r` and `s` byte lengths.
///
/// Earlier versions accepted any 64..=72 byte vector, so `vec![0x30; 70]` was
/// a valid [`Signature`]. A signature type whose only invariant is its length
/// certifies nothing: the fabricated blobs shipped in 0.1.0/0.2.0 satisfied it
/// exactly, and so does arbitrary garbage. Checking the structure is the least
/// this type can do to earn its name.
///
/// This validates encoding only. It is *not* a cryptographic check — a
/// well-formed DER value proves nothing about whether the signature verifies.
fn parse_der_ecdsa_sig(b: &[u8]) -> Result<(usize, usize)> {
    let bad = |m: &str| Error::invalid_input(format!("malformed DER ECDSA signature: {m}"));

    if b.len() < 8 {
        return Err(bad("too short"));
    }
    if b[0] != 0x30 {
        return Err(bad("expected SEQUENCE tag 0x30"));
    }
    // Short-form length only: a P-256 signature is far below 128 bytes.
    let seq_len = b[1] as usize;
    if seq_len & 0x80 != 0 {
        return Err(bad("long-form length not valid for a P-256 signature"));
    }
    if seq_len + 2 != b.len() {
        return Err(bad("SEQUENCE length does not match buffer length"));
    }

    // INTEGER r
    if b[2] != 0x02 {
        return Err(bad("expected INTEGER tag for r"));
    }
    let r_len = b[3] as usize;
    if r_len == 0 || 4 + r_len > b.len() {
        return Err(bad("r length out of range"));
    }
    // INTEGER s
    let s_tag = 4 + r_len;
    if s_tag + 1 >= b.len() || b[s_tag] != 0x02 {
        return Err(bad("expected INTEGER tag for s"));
    }
    let s_len = b[s_tag + 1] as usize;
    if s_len == 0 || s_tag + 2 + s_len != b.len() {
        return Err(bad("s length does not consume the buffer"));
    }
    // DER integers are big-endian two's complement: a leading 0x00 is only
    // permitted to clear a high bit that would otherwise mean "negative".
    let r = &b[4..4 + r_len];
    let s = &b[s_tag + 2..s_tag + 2 + s_len];
    for (name, v) in [("r", r), ("s", s)] {
        if v[0] & 0x80 != 0 {
            return Err(bad(&format!("{name} is negative")));
        }
        if v.len() > 1 && v[0] == 0x00 && v[1] & 0x80 == 0 {
            return Err(bad(&format!("{name} has a non-minimal leading zero")));
        }
        if v.len() > 33 {
            return Err(bad(&format!("{name} exceeds 32 bytes for P-256")));
        }
    }
    Ok((r_len, s_len))
}

impl Signature {
    /// Create a signature from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the signature is empty or malformed.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        parse_der_ecdsa_sig(&bytes)?;
        Ok(Self { bytes })
    }

    /// Get the raw signature bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get the signature length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Check if the signature is empty (always false for valid signatures).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Public key from a Secure Enclave key pair.
///
/// Can be exported and used for verification on other systems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    /// Raw public key bytes (uncompressed point format).
    bytes: Vec<u8>,
}

impl PublicKey {
    /// Create a public key from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the key is malformed.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        // P-256 uncompressed public key: 04 || X (32 bytes) || Y (32 bytes) = 65 bytes
        if bytes.len() != 65 {
            return Err(Error::invalid_input(format!(
                "invalid P-256 public key length: {} (expected 65)",
                bytes.len()
            )));
        }

        // Check uncompressed point format marker
        if bytes[0] != 0x04 {
            return Err(Error::invalid_input(
                "public key must be in uncompressed point format (0x04 prefix)",
            ));
        }

        Ok(Self { bytes })
    }

    /// Get the raw public key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get the X coordinate of the public key point.
    #[must_use]
    pub fn x(&self) -> &[u8] {
        &self.bytes[1..33]
    }

    /// Get the Y coordinate of the public key point.
    #[must_use]
    pub fn y(&self) -> &[u8] {
        &self.bytes[33..65]
    }
}
