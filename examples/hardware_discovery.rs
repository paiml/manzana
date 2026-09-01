#![allow(clippy::too_many_lines)]
//! Hardware Discovery Example
//!
//! Discovers and reports all available Apple hardware accelerators.
//!
//! Run with: cargo run --example `hardware_discovery`

use manzana::{
    afterburner::AfterburnerMonitor, metal::MetalCompute, neural_engine::NeuralEngineSession,
    unified_memory::UmaBuffer,
};

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          MANZANA - Apple Hardware Discovery                ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Check platform
    println!(
        "Platform: {}",
        if manzana::is_macos() {
            "macOS"
        } else {
            "Other"
        }
    );
    println!("Manzana Version: {}", manzana::VERSION);
    println!();

    // Afterburner FPGA (Mac Pro 2019+)
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Afterburner FPGA (Mac Pro 2019+)                            │");
    println!("├─────────────────────────────────────────────────────────────┤");
    if AfterburnerMonitor::is_available() {
        println!("│ Status: ✓ AVAILABLE                                         │");
        if let Some(monitor) = AfterburnerMonitor::new() {
            if let Ok(stats) = monitor.stats() {
                println!(
                    "│ Active Streams: {:>3} / {:>3}                                 │",
                    stats.streams_active, stats.streams_capacity
                );
                println!(
                    "│ Utilization: {:>5.1}%                                        │",
                    stats.utilization_percent
                );
            }
        }
    } else {
        println!("│ Status: ✗ Not available (requires Mac Pro with Afterburner) │");
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Neural Engine (Apple Silicon)
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Apple Neural Engine (Apple Silicon)                         │");
    println!("├─────────────────────────────────────────────────────────────┤");
    if NeuralEngineSession::is_available() {
        println!("│ Status: ✓ AVAILABLE                                         │");
        if let Some(caps) = NeuralEngineSession::capabilities() {
            println!(
                "│ Performance: {:>5.1} TOPS                                    │",
                caps.tops
            );
            println!(
                "│ Cores: {:>2}                                                  │",
                caps.core_count
            );
        }
    } else {
        println!("│ Status: ✗ Not available (requires Apple Silicon)            │");
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Metal GPU
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Metal GPU Compute                                           │");
    println!("├─────────────────────────────────────────────────────────────┤");
    if MetalCompute::is_available() {
        println!("│ Status: ✓ AVAILABLE                                         │");
        let devices = MetalCompute::devices();
        for (i, device) in devices.iter().enumerate() {
            println!("│ GPU {}: {:<50} │", i, truncate(&device.name, 50));
            println!(
                "│   VRAM: {:>6.1} GB | UMA: {}                              │",
                device.vram_gb(),
                if device.has_unified_memory {
                    "Yes"
                } else {
                    "No "
                }
            );
        }
    } else {
        println!("│ Status: ✗ Not available                                     │");
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Secure Enclave support was REMOVED in 0.3.0. manzana ships no
    // cryptography; use the `security-framework` crate.
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Secure Enclave                                              │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│ Removed in 0.3.0 — manzana implements no cryptography.      │");
    println!("│ Use the `security-framework` crate.                         │");
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Unified Memory
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Unified Memory Architecture (Apple Silicon)                 │");
    println!("├─────────────────────────────────────────────────────────────┤");
    // The chip's unified memory and manzana's ability to USE it are different
    // questions, and on Apple Silicon they have different answers. Printing
    // only "Not available" next to a Metal panel reading "UMA: Yes" made this
    // program appear to contradict itself.
    let chip_uma = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "yes (Apple Silicon)"
    } else {
        "no"
    };
    println!("│ Chip has unified memory:  {chip_uma:<34}│");
    println!(
        "│ GPU-visible via manzana:  {:<34}│",
        "no (not implemented)"
    );
    println!("│{:<61}│", "");
    println!(
        "│{:<61}│",
        " UmaBuffer is a page-aligned HOST allocation. It is not an"
    );
    println!("│{:<61}│", " MTLBuffer and no GPU can read it.");
    match UmaBuffer::new(4096) {
        Ok(buffer) => println!(
            "│ Host allocation: {:<43}│",
            format!(
                "{} bytes, page-aligned: {}",
                buffer.len(),
                if buffer.is_aligned() { "yes" } else { "NO" }
            )
        ),
        Err(e) => println!("│ Host allocation failed: {:<36}│", e.to_string()),
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Summary
    println!("╔════════════════════════════════════════════════════════════╗");
    // PRESENT and USABLE are counted separately. Reporting "2 accelerators
    // available" for an ANE and a GPU on which no operation can be performed
    // is the shape of claim this release exists to remove.
    let present = [
        AfterburnerMonitor::is_available(),
        NeuralEngineSession::is_available(),
        MetalCompute::is_available(),
    ]
    .iter()
    .filter(|&&x| x)
    .count();
    println!(
        "║ Detected: {:<49}║",
        format!("{present} accelerator(s) present")
    );
    println!(
        "║ Usable:   {:<49}║",
        format!(
            "{} through manzana today",
            if manzana::is_acceleration_usable() {
                "some"
            } else {
                "none"
            }
        )
    );
    println!("║{:<60}║", "");
    println!(
        "║{:<60}║",
        " Implemented: Afterburner stats, Metal enumeration, and"
    );
    println!(
        "║{:<60}║",
        " page-aligned host buffers. Metal compute, CoreML"
    );
    println!(
        "║{:<60}║",
        " inference and ANE capability querying are not."
    );
    println!("╚════════════════════════════════════════════════════════════╝");
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{s:<max_len$}")
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
