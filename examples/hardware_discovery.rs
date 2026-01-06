//! Hardware Discovery Example
//!
//! Discovers and reports all available Apple hardware accelerators.
//!
//! Run with: cargo run --example `hardware_discovery`

use manzana::{
    afterburner::AfterburnerMonitor,
    metal::MetalCompute,
    neural_engine::NeuralEngineSession,
    secure_enclave::SecureEnclaveSigner,
    unified_memory::UmaBuffer,
};

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          MANZANA - Apple Hardware Discovery                ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Check platform
    println!("Platform: {}", if manzana::is_macos() { "macOS" } else { "Other" });
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
                println!("│ Active Streams: {:>3} / {:>3}                                 │",
                    stats.streams_active, stats.streams_capacity);
                println!("│ Utilization: {:>5.1}%                                        │",
                    stats.utilization_percent);
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
            println!("│ Performance: {:>5.1} TOPS                                    │", caps.tops);
            println!("│ Cores: {:>2}                                                  │", caps.core_count);
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
            println!("│   VRAM: {:>6.1} GB | UMA: {}                              │",
                device.vram_gb(),
                if device.has_unified_memory { "Yes" } else { "No " }
            );
        }
    } else {
        println!("│ Status: ✗ Not available                                     │");
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Secure Enclave
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Secure Enclave (T2 / Apple Silicon)                         │");
    println!("├─────────────────────────────────────────────────────────────┤");
    if SecureEnclaveSigner::is_available() {
        println!("│ Status: ✓ AVAILABLE                                         │");
        println!("│ Algorithm: P-256 ECDSA                                      │");
    } else {
        println!("│ Status: ✗ Not available                                     │");
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Unified Memory
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ Unified Memory Architecture (Apple Silicon)                 │");
    println!("├─────────────────────────────────────────────────────────────┤");
    if UmaBuffer::is_uma_available() {
        println!("│ Status: ✓ AVAILABLE                                         │");
        if let Ok(buffer) = UmaBuffer::new(4096) {
            println!("│ Page Size: 4096 bytes                                       │");
            println!("│ Test Allocation: {} (aligned: {})                        │",
                if buffer.len() == 4096 { "OK" } else { "FAIL" },
                if buffer.is_aligned() { "yes" } else { "no" }
            );
        }
    } else {
        println!("│ Status: ✗ Not available (requires Apple Silicon)            │");
    }
    println!("└─────────────────────────────────────────────────────────────┘");
    println!();

    // Summary
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║ Summary: {} accelerator(s) available                        ║",
        [
            AfterburnerMonitor::is_available(),
            NeuralEngineSession::is_available(),
            MetalCompute::is_available(),
            SecureEnclaveSigner::is_available(),
            UmaBuffer::is_uma_available(),
        ].iter().filter(|&&x| x).count()
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
