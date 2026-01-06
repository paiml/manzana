//! Metal GPU Compute Example
//!
//! Demonstrates Metal GPU device enumeration and compute setup.
//!
//! Run with: cargo run --example `metal_compute`

use manzana::metal::MetalCompute;

fn main() -> Result<(), manzana::Error> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          MANZANA - Metal GPU Compute Demo                  ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Check availability
    if !MetalCompute::is_available() {
        println!("❌ Metal not available on this system.");
        println!("   Requires: macOS with Metal-capable GPU");
        return Ok(());
    }

    // Enumerate all Metal devices
    let devices = MetalCompute::devices();
    println!("Found {} Metal device(s):", devices.len());
    println!();

    for (i, device) in devices.iter().enumerate() {
        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│ GPU {}: {:<52} │", i, &device.name);
        println!("├─────────────────────────────────────────────────────────────┤");
        println!("│ Registry ID: {:<46} │", device.registry_id);
        println!("│ VRAM: {:>6.1} GB                                            │", device.vram_gb());
        println!("│ Max Threads/Group: {:>6}                                   │", device.max_threads_per_threadgroup);
        println!("│ Low Power: {:<5}  Headless: {:<5}  UMA: {:<5}              │",
            if device.is_low_power { "Yes" } else { "No" },
            if device.is_headless { "Yes" } else { "No" },
            if device.has_unified_memory { "Yes" } else { "No" }
        );
        println!("│ Apple Silicon: {:<5}                                        │",
            if device.is_apple_silicon() { "Yes" } else { "No" }
        );
        println!("└─────────────────────────────────────────────────────────────┘");
        println!();
    }

    // Create compute pipeline on default device
    println!("Creating compute pipeline on default device...");
    let compute = MetalCompute::default_device()?;
    println!("✓ Pipeline created on: {}", compute.device_name());
    println!();

    // Compile a simple shader
    println!("Compiling shader...");
    let shader_source = r"
        kernel void vector_add(
            device float* a [[buffer(0)]],
            device float* b [[buffer(1)]],
            device float* result [[buffer(2)]],
            uint id [[thread_position_in_grid]]
        ) {
            result[id] = a[id] + b[id];
        }
    ";

    let shader = compute.compile_shader(shader_source, "vector_add")?;
    println!("✓ Shader compiled: {}", shader.name());
    println!();

    // Allocate buffers
    println!("Allocating GPU buffers...");
    let buffer_size = 1024 * 1024; // 1MB
    let buffer_a = compute.allocate_buffer(buffer_size)?;
    let buffer_b = compute.allocate_buffer(buffer_size)?;
    let buffer_result = compute.allocate_buffer(buffer_size)?;

    println!("✓ Allocated 3 buffers × {} KB = {} KB total",
        buffer_size / 1024,
        (buffer_size * 3) / 1024
    );
    println!();

    // Dispatch compute (stub - would execute on real Metal)
    println!("Dispatching compute kernel...");
    let elements = buffer_size / 4; // float = 4 bytes
    let threadgroup_size = 256;
    #[allow(clippy::cast_possible_truncation)]
    let grid_size = (elements / threadgroup_size) as u32;
    #[allow(clippy::cast_possible_truncation)]
    let threadgroup_size_u32 = threadgroup_size as u32;

    compute.dispatch(
        &shader,
        &[&buffer_a, &buffer_b, &buffer_result],
        (grid_size, 1, 1),
        (threadgroup_size_u32, 1, 1),
    )?;

    println!("✓ Dispatched {elements} threads in {grid_size} threadgroups");
    println!();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    Demo Complete                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    Ok(())
}
