//! Hardware discovery example.
//!
//! Reports which Apple accelerators this machine has and, separately, what
//! manzana can actually do with each. Those are different questions with
//! different answers, so this program prints both instead of collapsing them
//! into one count of "available" accelerators.
//!
//! Run with `cargo run --example hardware_discovery`. It runs on any platform;
//! on a non-Apple host every panel reports absence.

use manzana::{
    afterburner::AfterburnerMonitor, metal::MetalCompute, neural_engine::NeuralEngineSession,
    unified_memory::UmaBuffer,
};

const TOP: &str = "┌─────────────────────────────────────────────────────────────┐";
const MID: &str = "├─────────────────────────────────────────────────────────────┤";
const BOT: &str = "└─────────────────────────────────────────────────────────────┘";
const WIDE: &str = "╔════════════════════════════════════════════════════════════╗";
const WIDE_END: &str = "╚════════════════════════════════════════════════════════════╝";

/// Width available for text inside a single-ruled panel.
const ROW: usize = 59;
/// Width available for text inside a double-ruled panel.
const WIDE_ROW: usize = 58;

fn main() {
    println!("{WIDE}");
    wide_row("         MANZANA - Apple Hardware Discovery");
    println!("{WIDE_END}");
    println!();

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

    afterburner_panel();
    neural_engine_panel();
    let devices = metal_panel();
    secure_enclave_panel();
    unified_memory_panel(&devices);
    summary_panel(&devices);
}

/// Afterburner: presence and statistics are both implemented, via `IOKit`.
fn afterburner_panel() {
    panel("Afterburner FPGA (Mac Pro 2019+)");
    if let Some(monitor) = AfterburnerMonitor::new() {
        row("Present: yes (IOKit service matched)");
        match monitor.stats() {
            Ok(stats) => {
                row(&format!(
                    "Active streams: {} of {}",
                    stats.streams_active, stats.streams_capacity
                ));
                row(&format!("Utilization: {:.1}%", stats.utilization_percent));
            }
            Err(e) => row(&format!("Statistics query failed: {e}")),
        }
    } else if cfg!(target_os = "macos") {
        row("Present: no card found (this Mac has no Afterburner fitted)");
    } else {
        // NOT "no card fitted". Verified on a MacPro7,1 running Linux with an
        // Afterburner installed: lspci shows it at 0f:00.0, Apple 106b:0205,
        // and manzana still cannot read it. Saying "no card" there would be a
        // false statement of a REASON -- the exact defect class this release
        // exists to remove.
        row("Cannot look: manzana reads the card through IOKit, which");
        row("exists only on macOS. A fitted card is unreadable here.");
    }
    row("");
    row("Implemented: presence and statistics, read from IOKit.");
    println!("{BOT}");
    println!();
}

/// Neural Engine: presence only. Nothing can be run on it through manzana.
fn neural_engine_panel() {
    panel("Apple Neural Engine (Apple Silicon)");
    if NeuralEngineSession::is_available() {
        row("Present: yes (build target is aarch64 macOS)");
        row("This is a compile-time check, not a hardware probe. It");
        row("is sound because every Apple Silicon part ships an ANE.");
    } else {
        row("Present: no (requires Apple Silicon)");
    }
    row("");
    row("Capabilities (TOPS, cores): not queried. capabilities()");
    row("returns None rather than an M1 datasheet figure.");
    row("Model loading and inference: not implemented.");
    println!("{BOT}");
    println!();
}

/// Metal: device enumeration is implemented; compute on those devices is not.
///
/// Returns the enumerated devices so later panels can answer from the same
/// data rather than from a second, possibly disagreeing, source.
fn metal_panel() -> Vec<manzana::MetalDevice> {
    let devices = MetalCompute::devices();
    panel("Metal GPU");
    if devices.is_empty() {
        row("Present: no device enumerated");
        row("Enumeration shells out to `system_profiler`; when that");
        row("reports nothing, manzana reports nothing.");
    } else {
        row(&format!(
            "Present: yes ({} device(s) enumerated)",
            devices.len()
        ));
        for (i, device) in devices.iter().enumerate() {
            row("");
            row(&format!("GPU {i}: {}", device.name));
            // The provenance label must follow the value's actual source.
            // Printing "(from system_profiler)" unconditionally attributed
            // manzana's own 16 GiB constant to the report -- and on Apple
            // Silicon, which prints no VRAM line at all, that was EVERY run.
            row(&format!(
                "  VRAM: {:.1} GiB {}",
                device.vram_gb(),
                if device.reported_vram_bytes.is_some() {
                    "(read from the system_profiler VRAM line)"
                } else {
                    "(manzana's default -- system_profiler printed no VRAM line)"
                }
            ));
            row(&format!(
                "  Unified memory: {} (inferred from the name)",
                yes_no(device.has_unified_memory)
            ));
            row("  registry_id, thread limits and headless flag are");
            row("  synthesized or hardcoded; see the MetalDevice docs.");
        }
    }
    row("");
    row("Implemented: enumeration (name, VRAM). Shader compilation,");
    row("buffer allocation and dispatch are not - see the");
    row("metal_compute example for their refusals.");
    println!("{BOT}");
    println!();
    devices
}

/// Secure Enclave support was removed in 0.3.0; manzana ships no cryptography.
fn secure_enclave_panel() {
    panel("Secure Enclave");
    row("Removed in 0.3.0. manzana implements no cryptography and");
    row("makes no Security framework call. For Secure Enclave and");
    row("Keychain access use the `security-framework` crate.");
    println!("{BOT}");
    println!();
}

/// Unified memory: a chip property and a manzana capability, kept apart.
///
/// The chip answer is read from the same enumerated devices the Metal panel
/// printed, so the two panels cannot disagree.
fn unified_memory_panel(devices: &[manzana::MetalDevice]) {
    let chip_has_uma = devices.iter().any(|d| d.has_unified_memory);

    panel("Unified Memory");
    row(&format!(
        "This chip has unified memory:  {}",
        yes_no(chip_has_uma)
    ));
    row("manzana can hand you GPU-visible memory:  no");
    row("");
    row("Those are different questions. On Apple Silicon the CPU and");
    row("GPU do share physical memory, but a host allocation becomes");
    row("GPU-visible only once it is wrapped in an MTLBuffer, and");
    row("manzana never does that. UmaBuffer is a page-aligned HOST");
    row("allocation: real, zeroed, freed on drop, and readable only");
    row("by the CPU.");
    row("");
    match UmaBuffer::new(4096) {
        Ok(buffer) => row(&format!(
            "Host allocation: {} bytes, page-aligned: {}",
            buffer.len(),
            yes_no(buffer.is_aligned())
        )),
        Err(e) => row(&format!("Host allocation failed: {e}")),
    }
    println!("{BOT}");
    println!();
}

/// Counts detected hardware, then says what can be done with it.
fn summary_panel(devices: &[manzana::MetalDevice]) {
    let detected = [
        AfterburnerMonitor::is_available(),
        NeuralEngineSession::is_available(),
        !devices.is_empty(),
    ]
    .iter()
    .filter(|&&present| present)
    .count();

    println!("{WIDE}");
    wide_row(&format!("Accelerators detected: {detected} of 3"));
    wide_row("");
    wide_row("Detection is not usability. Implemented today:");
    wide_row("  Afterburner    presence and statistics (IOKit)");
    wide_row("  Metal GPU      enumeration only (name, VRAM)");
    wide_row("  Neural Engine  presence only");
    wide_row("  Host memory    page-aligned buffers (any platform)");
    wide_row("");
    wide_row("Metal compute, CoreML inference, ANE capability queries");
    wide_row("and GPU-visible memory are not implemented. They return");
    wide_row("Error::Unimplemented rather than a fabricated result.");
    println!("{WIDE_END}");
}

/// Open a single-ruled panel with `title` as its heading.
fn panel(title: &str) {
    println!("{TOP}");
    row(title);
    println!("{MID}");
}

/// Print one line inside a single-ruled panel.
fn row(text: &str) {
    println!("│ {:<ROW$} │", fit(text, ROW));
}

/// Print one line inside a double-ruled panel.
fn wide_row(text: &str) {
    println!("║ {:<WIDE_ROW$} ║", fit(text, WIDE_ROW));
}

/// Truncate `text` to `max` characters so a panel border cannot be pushed out.
fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let kept: String = text.chars().take(max - 1).collect();
        format!("{kept}…")
    }
}

/// Render a boolean as the word this program uses for it everywhere.
const fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}
