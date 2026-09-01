//! Metal GPU Compute Example
//!
//! Demonstrates Metal GPU device enumeration, which is implemented, and
//! reports the compute operations that are not.
//!
//! Run with: cargo run --example `metal_compute`

use manzana::metal::MetalCompute;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          MANZANA - Metal GPU Compute Demo                  ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Check availability
    if !MetalCompute::is_available() {
        println!("❌ Metal not available on this system.");
        println!("   Requires: macOS with Metal-capable GPU");
        return;
    }

    // Enumerate all Metal devices
    let devices = MetalCompute::devices();
    println!("Found {} Metal device(s):", devices.len());
    println!();

    for (i, device) in devices.iter().enumerate() {
        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│ GPU {}: {:<52} │", i, device.name);
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│ Registry ID: {:<46} │", device.registry_id);
        println!(
            "│ VRAM: {:>6.1} GB                                            │",
            device.vram_gb()
        );
        println!(
            "│ Max Threads/Group: {:>6}                                   │",
            device.max_threads_per_threadgroup
        );
        println!(
            "│ Low Power: {:<5}  Headless: {:<5}  UMA: {:<5}              │",
            if device.is_low_power { "Yes" } else { "No" },
            if device.is_headless { "Yes" } else { "No" },
            if device.has_unified_memory {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "│ Apple Silicon: {:<5}                                        │",
            if device.is_apple_silicon() {
                "Yes"
            } else {
                "No"
            }
        );
        println!("└─────────────────────────────────────────────────────────────┘");
        println!();
    }

    // Everything past enumeration is unimplemented. Report that instead of
    // aborting the demo halfway through a `?`, which is what happened when
    // these calls were changed to return Error::Unimplemented.
    println!("Compute pipeline:");
    match MetalCompute::default_device() {
        Ok(compute) => {
            println!("  device        -> {}", compute.device_name());
            match compute.compile_shader("kernel void vector_add() {}", "vector_add") {
                Ok(_) => println!(
                    "  compile_shader-> unexpectedly succeeded; verify the backend is real"
                ),
                Err(e) => println!("  compile_shader-> {e}"),
            }
            match compute.allocate_buffer(1024) {
                Ok(_) => println!(
                    "  allocate_buffer-> unexpectedly succeeded; verify the backend is real"
                ),
                Err(e) => println!("  allocate_buffer-> {e}"),
            }
        }
        Err(e) => println!("  no default device: {e}"),
    }
    println!();
    println!("Device enumeration is implemented and real, via `system_profiler`.");
    println!("Shader compilation, buffer allocation and dispatch are not");
    println!("implemented and return Error::Unimplemented rather than pretending.");
    println!("See docs/specifications/security-architecture-plan.md");
}
