//! Tests for `detect::parse_displays`, the `system_profiler` output parser.
//!
//! Split from `tests.rs` to keep both files inside the 500-line health limit.
//!
//! These run on EVERY platform, because `parse_displays` is deliberately not
//! `cfg(target_os = "macos")`: it is pure text parsing, and gating it would put
//! this crate's only real parser on the macOS-only side of the lane split --
//! which is how 26 tests came to assert nothing on the Linux lane.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;

// ---------------------------------------------------------------------------
// Parser tests.
//
// These run on EVERY platform, because `parse_displays` is deliberately not
// cfg-gated: it is pure text parsing, and gating it would have put this crate's
// only real parser on the macOS-only side of the lane split -- which is exactly
// how 26 tests came to assert nothing on the Linux lane.
//
// The inputs below are SYNTHETIC. They encode the format `system_profiler
// SPDisplaysDataType` emits; they are not captures from a specific machine and
// are not presented as measurements of one. Their job is to pin the parser's
// behaviour deterministically. The cross-check against whatever this host
// really reports lives in `test_devices_reports_what_system_profiler_said`,
// which needs no fixture -- so a fixture that misdescribed the real format
// would be caught there rather than papered over here.
// ---------------------------------------------------------------------------

/// Discrete GPU with an explicit VRAM line, the Mac Pro shape.
const SP_DISCRETE: &str = "\
Graphics/Displays:

    AMD Radeon Pro W5700X:

      Chipset Model: AMD Radeon Pro W5700X
      Type: GPU
      Bus: PCIe
      VRAM (Total): 16 GB
      Vendor: AMD (0x1002)
      Metal Support: Metal 3
";

/// Apple Silicon shape: no VRAM line at all, because memory is unified.
const SP_APPLE_SILICON: &str = "\
Graphics/Displays:

    Apple M4:

      Chipset Model: Apple M4
      Type: GPU
      Bus: Built-In
      Total Number of Cores: 10
      Vendor: Apple (0x106b)
      Metal Support: Metal 3
";

#[test]
fn test_parse_reads_the_vram_line_when_there_is_one() {
    let devices = detect::parse_displays(SP_DISCRETE);
    assert_eq!(devices.len(), 1, "one GPU stanza, one device");
    let d = &devices[0];
    assert_eq!(d.name, "AMD Radeon Pro W5700X");
    // 16 GiB, because system_profiler prints "GB" and Apple means GiB.
    assert_eq!(d.max_buffer_length, 17_179_869_184);
    assert!((d.vram_gb() - 16.0).abs() < f64::EPSILON);
    assert_eq!(
        d.reported_vram_bytes,
        Some(17_179_869_184),
        "a parsed VRAM line is a measurement and must be recorded as one"
    );
    assert_eq!(
        d.has_unified_memory,
        cfg!(target_arch = "aarch64"),
        "has_unified_memory is DERIVED FROM THE BUILD TARGET, not the device: \
         on an aarch64 build even this PCIe AMD card reports unified memory. \
         That is documented in the README's field table, and this assertion \
         exists so the derivation cannot quietly change into a measurement \
         claim -- an earlier version asserted `false` unconditionally and \
         passed only because it never ran on Apple Silicon."
    );
}

#[test]
fn test_parse_falls_back_when_there_is_no_vram_line() {
    let devices = detect::parse_displays(SP_APPLE_SILICON);
    assert_eq!(devices.len(), 1);
    let d = &devices[0];
    assert_eq!(d.name, "Apple M4");
    // Nothing was read: this is the documented hardcoded default, not a
    // measurement. The README and the field docs both say so.
    assert_eq!(d.max_buffer_length, 17_179_869_184);
    assert!(d.has_unified_memory);
}

#[test]
fn test_parse_finds_every_gpu_not_just_the_first() {
    let two = format!(
        "{SP_DISCRETE}\n{}",
        SP_APPLE_SILICON.replace("Graphics/Displays:\n\n", "")
    );
    let devices = detect::parse_displays(&two);
    assert_eq!(
        devices.len(),
        2,
        "both stanzas must be parsed, got {devices:?}"
    );
    assert_eq!(devices[0].name, "AMD Radeon Pro W5700X");
    assert_eq!(devices[1].name, "Apple M4");
    // registry_id is the enumeration index plus one -- documented as NOT a
    // device property. Pin it so the doc stays true.
    assert_eq!(devices[0].registry_id, 1);
    assert_eq!(devices[1].registry_id, 2);
}

/// The anti-fabrication property, tested directly on the parser.
///
/// Every one of these inputs names no GPU. Each must yield NO device. This is
/// the test that fails if the fabricated "Apple GPU" fallback ever returns --
/// it is the 0.2.0 defect in its smallest reproducible form.
#[test]
fn test_parse_invents_no_device_when_nothing_is_reported() {
    for (label, input) in [
        ("empty", ""),
        ("header only", "Graphics/Displays:\n"),
        ("whitespace", "   \n\n  \n"),
        (
            "no stanza",
            "Graphics/Displays:\n\n      Metal Support: Metal 3\n",
        ),
        (
            "unrelated output",
            "Software:\n\n    System Software Overview:\n",
        ),
    ] {
        let devices = detect::parse_displays(input);
        assert!(
            devices.is_empty(),
            "{label}: parser invented {} device(s) from output naming none: {devices:?}",
            devices.len()
        );
    }
}

/// An older discrete card reporting VRAM in MB, and a non-Apple name.
///
/// Covers the `MB` branch and the non-Apple-Silicon buffer-length default,
/// neither of which either host in the e2e matrix exercises: the Mac Pro runs
/// Linux (so `detect` is cfg'd out there) and the M4 reports no VRAM line at
/// all. Without this, both paths shipped untested.
const SP_MB_CARD: &str = "\
Graphics/Displays:

    NVIDIA GeForce GT 750M:

      Chipset Model: NVIDIA GeForce GT 750M
      Type: GPU
      Bus: PCIe
      VRAM (Total): 2048 MB
      Vendor: NVIDIA (0x10de)
";

#[test]
fn test_parse_reads_a_vram_line_in_mb() {
    let devices = detect::parse_displays(SP_MB_CARD);
    assert_eq!(devices.len(), 1);
    let d = &devices[0];
    assert_eq!(d.name, "NVIDIA GeForce GT 750M");
    assert_eq!(d.max_buffer_length, 2048 * 1_048_576);
    assert_eq!(
        d.reported_vram_bytes,
        Some(2048 * 1_048_576),
        "the MB branch is a measurement too"
    );
    assert_eq!(
        d.has_unified_memory,
        cfg!(target_arch = "aarch64"),
        "derived from the build target, not the device -- see the sibling test"
    );
}

/// A VRAM figure the parser cannot read must leave the default in place, not
/// produce a wrong number.
#[test]
fn test_parse_leaves_the_default_when_vram_does_not_parse() {
    for bad in [
        "VRAM (Total): 1.5 GB",         // fractional: does not parse as u64
        "VRAM (Total): lots",           // no unit at all
        "VRAM (Total):",                // empty value
        "VRAM (Dynamic, Max): 1536 KB", // a unit the parser does not know
    ] {
        let input = format!(
            "Graphics/Displays:\n\n    Card:\n\n      Chipset Model: Some Discrete Card\n      {bad}\n"
        );
        let devices = detect::parse_displays(&input);
        assert_eq!(devices.len(), 1, "{bad}: expected exactly one device");

        // The property that matters, and it is platform-independent: NOTHING
        // was measured, so nothing may be reported as measured.
        assert_eq!(
            devices[0].reported_vram_bytes, None,
            "{bad}: an unreadable VRAM line is not a measurement"
        );

        // The usable figure falls to a documented CONSTANT. Which constant
        // depends on the build target, because `create_device` treats any
        // aarch64 build as Apple Silicon -- so this cannot be a single
        // hardcoded number, and asserting 4 GiB unconditionally is what made
        // this test fail the moment it first ran on the M4.
        let expected = if cfg!(target_arch = "aarch64") {
            17_179_869_184_u64
        } else {
            4_294_967_296
        };
        assert_eq!(
            devices[0].max_buffer_length, expected,
            "{bad}: must fall to the documented default for this build"
        );
        // And in particular it is NOT a number derived from the text: 1.5 GB
        // would be 1_610_612_736.
        assert_ne!(devices[0].max_buffer_length, 1_610_612_736);
    }
}

/// A `Chipset Model:` line with nothing after it names no device.
#[test]
fn test_parse_ignores_an_empty_chipset_model() {
    let devices = detect::parse_displays(
        "Graphics/Displays:\n\n      Chipset Model:\n      VRAM (Total): 8 GB\n",
    );
    assert!(
        devices.is_empty(),
        "an empty chipset model names nothing: {devices:?}"
    );
}

/// A VRAM line before any device stanza belongs to no device and is dropped.
#[test]
fn test_parse_drops_a_vram_line_with_no_device() {
    let devices = detect::parse_displays(
        "Graphics/Displays:\n      VRAM (Total): 8 GB\n\n      Chipset Model: Apple M4\n",
    );
    assert_eq!(devices.len(), 1);
    // The stray 8 GB must NOT have been attached to the M4.
    assert_eq!(
        devices[0].max_buffer_length, 17_179_869_184,
        "a VRAM figure from before the stanza was attributed to it"
    );
}

/// The fallback figure must be distinguishable from a measured one.
///
/// Both shipped examples printed `max_buffer_length` under a
/// "(from system_profiler)" label, and the README carried that line as
/// captured M4 output. On Apple Silicon `system_profiler` prints no VRAM line
/// at all, so the labelled figure was manzana's 16 GiB constant on every run --
/// a crate constant presented as a hardware measurement with a named source,
/// which is the defect class RUSTSEC-2026-0273 was filed over.
///
/// `reported_vram_bytes` makes the difference representable, which is what has
/// to be true before it can be reported honestly.
#[test]
fn test_reported_vram_distinguishes_measurement_from_default() {
    let measured = detect::parse_displays(SP_DISCRETE);
    assert_eq!(
        measured[0].reported_vram_bytes,
        Some(17_179_869_184),
        "a parsed VRAM line must be recorded as measured"
    );

    let defaulted = detect::parse_displays(SP_APPLE_SILICON);
    assert_eq!(
        defaulted[0].reported_vram_bytes, None,
        "Apple Silicon prints no VRAM line, so nothing was measured"
    );
    // ... and the usable figure is still populated, from the constant.
    assert_eq!(defaulted[0].max_buffer_length, 17_179_869_184);
    assert!(
        (defaulted[0].vram_gb() - 16.0).abs() < f64::EPSILON,
        "vram_gb() reports the constant -- which is exactly why the provenance \
         field has to exist"
    );

    // The two are indistinguishable by value alone: same number, different
    // provenance. That is the whole point.
    assert_eq!(
        measured[0].max_buffer_length,
        defaulted[0].max_buffer_length
    );
    assert_ne!(
        measured[0].reported_vram_bytes,
        defaulted[0].reported_vram_bytes
    );
}
