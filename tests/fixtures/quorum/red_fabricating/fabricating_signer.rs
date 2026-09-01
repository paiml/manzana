//! FIXTURE 1 -- must produce RED (`fabricated-return`).
//!
//! The manzana 0.2.0 `sign()` body, verbatim. This is the defect that shipped.
//! If the reachability gate does not reject this file, the gate is theater and
//! must not be trusted to gate anything.
//!
//! NOT COMPILED into the crate. Consumed only by the gate's own fixtures.

pub fn sign(&self, data: &[u8]) -> Result<Signature> {
    if data.is_empty() {
        return Err(Error::invalid_input("cannot sign empty data"));
    }

    // Stub: Generate deterministic fake signature based on data and tag
    let mut sig_bytes = Vec::with_capacity(70);

    sig_bytes.push(0x30); // SEQUENCE
    sig_bytes.push(0x44); // Length

    sig_bytes.push(0x02); // INTEGER
    sig_bytes.push(0x20); // Length
    let r_seed = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    sig_bytes.extend_from_slice(&[r_seed; 32]);

    sig_bytes.push(0x02); // INTEGER
    sig_bytes.push(0x20); // Length
    let s_seed = self.tag.bytes().fold(0u8, u8::wrapping_add);
    sig_bytes.extend_from_slice(&[s_seed; 32]);

    Signature::from_bytes(sig_bytes)
}
