//! FIXTURE 13 -- must produce GREEN (discrimination case).
//!
//! Reaches an allowlisted external boundary. If this is also rejected, the
//! gate is "refuse everything", which reads green while catching nothing.

pub fn create(config: KeyConfig) -> Result<Self> {
    let mut opts = security_framework::key::GenerateKeyOptions::default();
    opts.set_key_type(security_framework::key::KeyType::ec())
        .set_size_in_bits(256)
        .set_token(security_framework::key::Token::SecureEnclave);
    let key = security_framework::key::SecKey::new(&opts)
        .map_err(|e| Error::security(e.code()))?;
    Ok(Self { tag: config.tag, key })
}
