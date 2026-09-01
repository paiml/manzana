//! FIXTURE 14 -- must produce GREEN (discrimination case).
//!
//! Limb (b): refuses rather than fabricating. This is what manzana 0.3.0 ships.

pub fn sign(&self, data: &[u8]) -> Result<Signature> {
    let _ = data;
    Err(Error::unimplemented(
        crate::error::Subsystem::SecureEnclave,
        "signing (requires SecKeyCreateSignature)",
    ))
}
