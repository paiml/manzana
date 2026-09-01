//! GPU detection by parsing `system_profiler SPDisplaysDataType`.
//!
//! Kept apart from the `MetalCompute` API because it is a distinct concern:
//! text parsing of a shell-out, with its own failure mode (report nothing
//! rather than invent a device).

// `MetalDevice` is used by the un-gated parser below, so this import is NOT
// cfg-gated. `Command` is used only by the macOS-only shell-out and is.
//
// An earlier revision gated this import to match the functions then in this
// file, and `cargo fix` -- run on Linux, where it really was unused -- deleted
// it, breaking the macOS build while the Linux build stayed green. That is the
// single-platform-lane defect this release exists to remove, reintroduced by a
// tool run on one platform.
#[cfg(any(target_os = "macos", test))]
use super::MetalDevice;
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
pub(super) fn detect_gpus_via_system_profiler() -> Vec<MetalDevice> {
    Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(fallback_device, |o| {
            parse_displays(&String::from_utf8_lossy(&o.stdout))
        })
}

/// Parse `system_profiler SPDisplaysDataType` output into devices.
///
/// Deliberately NOT `#[cfg(target_os = "macos")]`, although its only caller is.
/// This is pure text parsing with no platform dependency, and gating it would
/// put the crate's only real parser on the macOS-only side of the lane split --
/// which is how 26 tests came to assert nothing on the Linux lane. Un-gated, it
/// is exercised on every platform in CI.
///
/// Returns an empty vector when the output names no GPU. It never invents one:
/// that is the property [`fallback_device`] exists to hold.
///
/// `cfg(any(target_os = "macos", test))` rather than `cfg(target_os = "macos")`:
/// its only non-test caller is macOS-only, so a Linux RELEASE build would carry
/// it as dead code -- but a Linux TEST build must have it, because these are the
/// crate's only tests of its one real parser and confining them to macOS is
/// precisely the lane split this release exists to close.
#[cfg(any(target_os = "macos", test))]
pub(super) fn parse_displays(output: &str) -> Vec<MetalDevice> {
    // Keyed on `Chipset Model:`, the field every GPU stanza carries -- a
    // WHITELIST.
    //
    // This was a blacklist: any line ending in `:` that was not one of a
    // handful of known headings was taken as a GPU name. That invents hardware
    // out of arbitrary text. Given the `Software:` / `System Software Overview:`
    // stanza shape it produced two devices, "Software" and "System Software
    // Overview", each with a 4 GiB max_buffer_length -- well-formed, plausible
    // and entirely fictional. Found by
    // `test_parse_invents_no_device_when_nothing_is_reported`.
    //
    // A blacklist is the wrong shape here: it defaults to "yes, a GPU" for
    // input nobody anticipated, and the subject of this release is that the
    // default answer must be "no, and I will say so".
    let mut names_and_vram: Vec<(&str, u64)> = Vec::new();

    for line in output.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix("Chipset Model:") {
            let name = name.trim();
            if !name.is_empty() {
                names_and_vram.push((name, 0));
            }
        } else if line.starts_with("VRAM") {
            // Belongs to the stanza it appears in; a VRAM line before any
            // `Chipset Model:` belongs to no device and is dropped.
            if let (Some(bytes), Some(last)) = (parse_vram_bytes(line), names_and_vram.last_mut()) {
                last.1 = bytes;
            }
        }
    }

    names_and_vram
        .into_iter()
        .enumerate()
        .map(|(index, (name, vram))| create_device(name, vram, index))
        .collect()
}

/// Bytes from a `VRAM (Total): 16 GB` line, or `None` if it does not parse.
///
/// `system_profiler` prints `GB` where Apple means GiB, so the multiplier is
/// 2^30 and not 10^9. A fractional figure such as `1.5 GB` does not parse and
/// yields `None`, which leaves the caller on its documented default rather than
/// on a number invented from the text.
#[cfg(any(target_os = "macos", test))]
fn parse_vram_bytes(line: &str) -> Option<u64> {
    let value = line.split(':').nth(1)?.trim();
    [(" GB", 1_073_741_824u64), (" MB", 1_048_576)]
        .into_iter()
        .find_map(|(unit, multiplier)| {
            let digits = value[..value.find(unit)?].trim();
            digits.parse::<u64>().ok()?.checked_mul(multiplier)
        })
}

#[cfg(any(target_os = "macos", test))]
fn create_device(name: &str, vram_bytes: u64, index: usize) -> MetalDevice {
    let is_apple_silicon = name.contains("Apple") || cfg!(target_arch = "aarch64");
    let is_integrated = name.contains("Intel") || name.contains("Integrated");

    MetalDevice {
        name: name.to_string(),
        reported_vram_bytes: if vram_bytes > 0 {
            Some(vram_bytes)
        } else {
            None
        },
        registry_id: (index + 1) as u64,
        is_low_power: is_integrated,
        is_headless: false,
        max_threads_per_threadgroup: 1024,
        max_buffer_length: if vram_bytes > 0 {
            vram_bytes
        } else if is_apple_silicon {
            17_179_869_184 // 16 GiB default for Apple Silicon
        } else {
            4_294_967_296 // 4 GiB default
        },
        has_unified_memory: is_apple_silicon,
        index,
    }
}

/// Returns no devices when detection fails.
///
/// Earlier versions fabricated a plausible `MetalDevice` here — named
/// "Apple GPU", reporting 1024 threads per threadgroup and a 4 GiB maximum
/// buffer — whenever `system_profiler` was unavailable. That invented a
/// GPU on machines that have no Metal at all. An empty device list is the
/// honest answer to "detection did not work".
#[cfg(target_os = "macos")]
pub(super) fn fallback_device() -> Vec<MetalDevice> {
    Vec::new()
}
