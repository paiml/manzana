//! GPU detection by parsing `system_profiler SPDisplaysDataType`.
//!
//! Kept apart from the `MetalCompute` API because it is a distinct concern:
//! text parsing of a shell-out, with its own failure mode (report nothing
//! rather than invent a device).

#[cfg(target_os = "macos")]
pub(super) fn detect_gpus_via_system_profiler() -> Vec<MetalDevice> {
    use std::process::Command;

    let output = match Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return Self::fallback_device(),
    };

    let mut devices = Vec::new();
    let mut current_name = String::new();
    let mut current_vram: u64 = 0;
    let mut index = 0;

    for line in output.lines() {
        let line = line.trim();

        // GPU name line (e.g., "AMD Radeon Pro W5700X:")
        if line.ends_with(':')
            && !line.starts_with("Graphics")
            && !line.contains("Displays")
            && !line.contains("VRAM")
            && !line.contains("Vendor")
            && !line.contains("Device")
            && !line.contains("Bus")
            && !line.contains("Slot")
            && !line.contains("Metal")
        {
            // Save previous GPU if we have one
            if !current_name.is_empty() {
                devices.push(Self::create_device(&current_name, current_vram, index));
                index += 1;
            }
            current_name = line.trim_end_matches(':').to_string();
            current_vram = 0;
        }

        // VRAM line (e.g., "VRAM (Total): 16 GB")
        if line.starts_with("VRAM") {
            if let Some(vram_str) = line.split(':').nth(1) {
                let vram_str = vram_str.trim();
                if let Some(gb_pos) = vram_str.find(" GB") {
                    if let Ok(gb) = vram_str[..gb_pos].trim().parse::<u64>() {
                        current_vram = gb * 1_073_741_824; // Convert GB to bytes
                    }
                } else if let Some(mb_pos) = vram_str.find(" MB") {
                    if let Ok(mb) = vram_str[..mb_pos].trim().parse::<u64>() {
                        current_vram = mb * 1_048_576; // Convert MB to bytes
                    }
                }
            }
        }
    }

    // Don't forget the last GPU
    if !current_name.is_empty() {
        devices.push(Self::create_device(&current_name, current_vram, index));
    }

    if devices.is_empty() {
        Self::fallback_device()
    } else {
        devices
    }
}

#[cfg(target_os = "macos")]
fn create_device(name: &str, vram_bytes: u64, index: usize) -> MetalDevice {
    let is_apple_silicon = name.contains("Apple") || cfg!(target_arch = "aarch64");
    let is_integrated = name.contains("Intel") || name.contains("Integrated");

    MetalDevice {
        name: name.to_string(),
        registry_id: (index + 1) as u64,
        is_low_power: is_integrated,
        is_headless: false,
        max_threads_per_threadgroup: 1024,
        max_buffer_length: if vram_bytes > 0 {
            vram_bytes
        } else if is_apple_silicon {
            17_179_869_184 // 16 GB default for Apple Silicon
        } else {
            4_294_967_296 // 4 GB default
        },
        has_unified_memory: is_apple_silicon,
        index,
    }
}

/// Returns no devices when detection fails.
///
/// Earlier versions fabricated a plausible `MetalDevice` here — named
/// "Apple GPU", reporting 1024 threads per threadgroup and a 4 GB maximum
/// buffer — whenever `system_profiler` was unavailable. That invented a
/// GPU on machines that have no Metal at all. An empty device list is the
/// honest answer to "detection did not work".
#[cfg(target_os = "macos")]
pub(super) fn fallback_device() -> Vec<MetalDevice> {
    Vec::new()
}
