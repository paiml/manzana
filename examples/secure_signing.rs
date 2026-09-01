//! Secure Enclave Status Example
//!
//! Reports what this crate can and cannot do with the Secure Enclave.
//!
//! There is deliberately no signing demonstration here. Until a real
//! `Security.framework` backend exists, a runnable "signing demo" could only
//! demonstrate something manzana did not actually do.
//!
//! Run with: cargo run --example `secure_signing`

use manzana::secure_enclave::{KeyConfig, SecureEnclaveSigner};

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       MANZANA - Secure Enclave Status                      ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    println!(
        "Secure Enclave operations available: {}",
        SecureEnclaveSigner::is_available()
    );
    println!();
    println!("manzana does not implement Secure Enclave cryptography.");
    println!("Every operation below fails, by design, rather than returning");
    println!("a value that could be mistaken for a real one.");
    println!();

    // Each call reports precisely why it cannot proceed.
    let config = KeyConfig::new("com.manzana.example.signing");
    match SecureEnclaveSigner::create(config) {
        Ok(_) => {
            // Unreachable today. If this ever prints, a construction path was
            // reintroduced and the guarantees in this example no longer hold.
            println!("⚠️  create() unexpectedly succeeded — verify the backend is real.");
        }
        Err(e) => println!("  create()  -> {e}"),
    }

    match SecureEnclaveSigner::load("com.manzana.example.signing") {
        Ok(_) => println!("⚠️  load() unexpectedly succeeded — verify the backend is real."),
        Err(e) => println!("  load()    -> {e}"),
    }

    println!();
    println!("What versions 0.1.0 and 0.2.0 did instead (both now yanked):");
    println!("  create()  returned a fixed public key, byte-summed from the tag");
    println!("  sign()    returned r = 32 copies of a byte-sum of the message,");
    println!("            s = 32 copies of a byte-sum of the tag");
    println!("  verify()  recomputed that value and compared it, so it accepted");
    println!("            forgeries from anyone who knew the tag");
    println!("  delete()  returned success without deleting anything");
    println!();
    println!("For real Secure Enclave access today, use the `security-framework`");
    println!("crate: https://crates.io/crates/security-framework");
    println!();
    println!("Background: https://github.com/paiml/manzana/issues/3");
    println!("Plan:       docs/specifications/security-architecture-plan.md");
}
