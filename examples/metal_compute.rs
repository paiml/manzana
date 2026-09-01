//! Metal GPU example: what manzana can and cannot do with a Metal device.
//!
//! Enumerates the GPUs `system_profiler` reports, labels which printed fields
//! were read from that report and which were not, then calls the compute
//! entry points so their refusals are visible rather than merely described.
//!
//! Run with `cargo run --example metal_compute`. On a host with no Metal
//! device — any non-macOS machine — it says so and exits.

use manzana::metal::MetalCompute;

const TOP: &str = "┌─────────────────────────────────────────────────────────────┐";
const MID: &str = "├─────────────────────────────────────────────────────────────┤";
const BOT: &str = "└─────────────────────────────────────────────────────────────┘";

/// Width available for text inside a panel.
const ROW: usize = 59;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          MANZANA - Metal GPU Compute Demo                  ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    let devices = MetalCompute::devices();
    if devices.is_empty() {
        println!("Metal not available on this system.");
        println!("Enumeration runs `system_profiler SPDisplaysDataType`, which");
        println!("reported no GPU here. Requires macOS with a Metal-capable GPU.");
        return;
    }

    println!("Found {} Metal device(s):", devices.len());
    println!();

    for device in &devices {
        println!("{TOP}");
        row(&format!("GPU {}: {}", device.index, device.name));
        println!("{MID}");
        row("Read from system_profiler:");
        row(&format!("  Name:  {}", device.name));
        row(&format!("  VRAM:  {:.1} GB", device.vram_gb()));
        row("");
        row("Not queried from the device - synthesized or derived:");
        row(&format!(
            "  Registry ID:        {} (enumeration index + 1)",
            device.registry_id
        ));
        row(&format!(
            "  Max threads/group:  {} (hardcoded literal)",
            device.max_threads_per_threadgroup
        ));
        row(&format!(
            "  Headless:           {} (never determined)",
            device.is_headless
        ));
        row(&format!(
            "  Low power:          {} (from the name string)",
            device.is_low_power
        ));
        row(&format!(
            "  Unified memory:     {} (from the name / build target)",
            device.has_unified_memory
        ));
        println!("{BOT}");
        println!();
    }

    compute_pipeline();
}

/// Call each compute entry point and print exactly what it returns.
///
/// The refusals are printed rather than propagated with `?`, so that one
/// failure does not hide the ones after it.
fn compute_pipeline() {
    println!("Compute pipeline:");

    let compute = match MetalCompute::default_device() {
        Ok(compute) => compute,
        Err(e) => {
            println!("  default_device  -> {e}");
            return;
        }
    };
    println!("  default_device  -> {}", compute.device_name());

    match compute.compile_shader("kernel void vector_add() {}", "vector_add") {
        Ok(shader) => println!(
            "  compile_shader  -> unexpectedly compiled {}; verify the backend",
            shader.name()
        ),
        Err(e) => println!("  compile_shader  -> {e}"),
    }

    match compute.allocate_buffer(1024) {
        Ok(buffer) => println!(
            "  allocate_buffer -> unexpectedly allocated {} bytes; verify the backend",
            buffer.len()
        ),
        Err(e) => println!("  allocate_buffer -> {e}"),
    }

    println!("  dispatch        -> cannot be attempted: it takes a CompiledShader");
    println!("                     and MetalBuffers, and neither can be obtained.");
    println!("                     Called directly it returns the same refusal.");
    println!();
    println!("Enumeration above is real, parsed from `system_profiler`.");
    println!("Shader compilation, buffer allocation and dispatch are not");
    println!("implemented: they return Error::Unimplemented on every platform,");
    println!("for every argument, rather than a value that resembles a result.");
    println!("See docs/specifications/security-architecture-plan.md");
}

/// Print one line inside a panel, truncated so the border cannot be pushed out.
fn row(text: &str) {
    let text = if text.chars().count() <= ROW {
        text.to_string()
    } else {
        let kept: String = text.chars().take(ROW - 1).collect();
        format!("{kept}…")
    };
    println!("│ {text:<ROW$} │");
}
