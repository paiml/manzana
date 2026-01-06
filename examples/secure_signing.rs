//! Secure Enclave Signing Example
//!
//! Demonstrates P-256 ECDSA signing using the Secure Enclave.
//!
//! Run with: cargo run --example `secure_signing`

use manzana::secure_enclave::{AccessControl, KeyConfig, SecureEnclaveSigner};

fn main() -> Result<(), manzana::Error> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       MANZANA - Secure Enclave Signing Demo                ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Check availability
    if !SecureEnclaveSigner::is_available() {
        println!("❌ Secure Enclave not available on this system.");
        println!("   Requires: T2 Mac (2018+) or Apple Silicon");
        return Ok(());
    }

    println!("✓ Secure Enclave detected");
    println!();

    // Create a signing key
    println!("Creating signing key...");
    let config = KeyConfig::new("com.manzana.example.signing")
        .with_access_control(AccessControl::None)
        .with_label("Manzana Example Key");

    let signer = SecureEnclaveSigner::create(config)?;
    println!("✓ Key created: {}", signer.tag());
    println!();

    // Display public key info
    let pubkey = signer.public_key();
    println!("Public Key (P-256):");
    println!("  Length: {} bytes (uncompressed)", pubkey.as_bytes().len());
    println!("  X: {:02x}{:02x}...{:02x}{:02x}",
        pubkey.x()[0], pubkey.x()[1],
        pubkey.x()[30], pubkey.x()[31]);
    println!("  Y: {:02x}{:02x}...{:02x}{:02x}",
        pubkey.y()[0], pubkey.y()[1],
        pubkey.y()[30], pubkey.y()[31]);
    println!();

    // Sign some data
    let message = b"Hello, Sovereign AI!";
    println!("Signing message: \"{}\"", String::from_utf8_lossy(message));

    let signature = signer.sign(message)?;
    println!("✓ Signature created: {} bytes (DER format)", signature.len());
    println!();

    // Verify the signature
    println!("Verifying signature...");
    let valid = signer.verify(message, &signature)?;
    println!("{} Signature valid: {}", if valid { "✓" } else { "❌" }, valid);
    println!();

    // Demonstrate invalid signature detection
    println!("Testing tampered message...");
    let tampered = b"Hello, Tampered AI!";
    let still_valid = signer.verify(tampered, &signature)?;
    println!("{} Tampered message rejected: {}",
        if still_valid { "❌" } else { "✓" },
        !still_valid
    );
    println!();

    // Clean up
    println!("Deleting key...");
    signer.delete()?;
    println!("✓ Key deleted");
    println!();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    Ok(())
}
