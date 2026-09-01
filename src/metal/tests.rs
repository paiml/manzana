//! Tests for the `metal` module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;

/// `devices()` must report what `system_profiler` said, and invent nothing.
///
/// This test previously asserted "on macOS, at least one device". That is a
/// HARDWARE assumption wearing a PLATFORM assumption's clothes, and it is
/// false: GitHub's macos-latest runner is a headless VM whose
/// `system_profiler SPDisplaysDataType` names no GPU, and the assertion failed
/// there the first time this crate's macOS lane ran in CI. Presence and
/// reachability are different questions -- which is the whole subject of this
/// release -- and "runs macOS" answers neither.
///
/// So the assertion is now the actual contract, and it is falsifiable on BOTH
/// kinds of host:
///   - a host whose system_profiler names a GPU must get a non-empty list
///     (fails if `devices()` is replaced by `Vec::new()`)
///   - a host whose system_profiler names none must get an EMPTY list
///     (fails if `devices()` regains the fabricated "Apple GPU" fallback,
///     which is the 0.2.0 defect)
#[test]
fn test_devices_reports_what_system_profiler_said() {
    let devices = MetalCompute::devices();

    #[cfg(not(target_os = "macos"))]
    assert!(
        devices.is_empty(),
        "no Metal enumeration exists off macOS; a device here would be invented"
    );

    #[cfg(target_os = "macos")]
    {
        let named_a_gpu = system_profiler_named_a_gpu();
        assert_eq!(
            !devices.is_empty(),
            named_a_gpu,
            "devices() disagrees with system_profiler: it named {} GPU(s) but \
             devices() returned {}",
            if named_a_gpu { "at least one" } else { "no" },
            devices.len()
        );
    }
}

/// Does this host's `system_profiler` actually name a GPU?
///
/// Runs the same command `detect_gpus_via_system_profiler` runs, and looks for
/// the marker every GPU stanza carries. Used to decide what the tests above are
/// entitled to assert on THIS machine, rather than assuming.
#[cfg(target_os = "macos")]
fn system_profiler_named_a_gpu() -> bool {
    std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("Chipset Model:"))
}

#[test]
fn test_is_available_consistent() {
    let available = MetalCompute::is_available();
    let devices = MetalCompute::devices();
    assert_eq!(available, !devices.is_empty());
}

#[test]
fn test_device_properties() {
    let devices = MetalCompute::devices();
    for device in &devices {
        assert!(!device.name.is_empty());
        assert!(device.max_threads_per_threadgroup > 0);
        assert!(device.max_buffer_length > 0);
        assert!(device.vram_gb() > 0.0);
    }
}

#[test]
fn test_new_invalid_index() {
    let result = MetalCompute::new(999);
    assert!(result.is_err());
}

// On macOS these now fail if `system_profiler` reports no GPU. That used
// to be masked by a fabricated fallback device; a Mac where GPU detection
// genuinely does not work is a real finding and should be visible.
// Detection is consistent with enumeration, on every platform. Asserting
// "a device exists" would break a GPU-less macOS CI runner now that
// fallback_device() no longer fabricates one; asserting the *relationship*
// holds everywhere and still fails if detection and construction disagree.
#[test]
fn test_device_construction_agrees_with_enumeration() {
    let n = MetalCompute::devices().len();
    assert_eq!(
        MetalCompute::new(0).is_ok(),
        n > 0,
        "new(0) must succeed exactly when enumeration reports devices (n={n})"
    );
    assert_eq!(MetalCompute::default_device().is_ok(), n > 0);
    // Out-of-range index must always fail.
    assert!(MetalCompute::new(n + 1).is_err());
}

// The compute path must refuse rather than silently drop work.
//
// These are ungated: the refusal is platform-independent, so they run in
// the Linux CI lane too. Previously every compute assertion was behind
// `cfg(target_os = "macos")`, leaving nothing to fail by default.
// These previously began `let Ok(compute) = ... else { return; }`, so on a
// machine with no Metal device -- every Linux CI runner -- they returned
// early and passed without asserting anything. That is a silent pass: the
// exact defect class this release exists to remove, reintroduced by the
// fix. `MetalCompute` is constructed directly so the refusal is asserted
// on every platform.
fn any_compute() -> MetalCompute {
    MetalCompute::default_device().unwrap_or_else(|_| MetalCompute {
        device_index: 0,
        device_name: String::from("test-harness (no device required)"),
        _not_send_sync: std::marker::PhantomData,
    })
}

#[test]
fn test_compile_shader_is_unimplemented() {
    let err = any_compute()
        .compile_shader("kernel void add() {}", "add")
        .expect_err("shader compilation must not report success");
    assert!(err.is_unimplemented(), "got {err:?}");
}

#[test]
fn test_allocate_buffer_is_unimplemented() {
    // The removed version returned a MetalBuffer holding only a length,
    // so this call "succeeded" while allocating nothing at all.
    let err = any_compute()
        .allocate_buffer(1024)
        .expect_err("buffer allocation must not report success");
    assert!(err.is_unimplemented(), "got {err:?}");
}

#[test]
fn test_dispatch_is_unimplemented() {
    let err = any_compute()
        .dispatch(
            &CompiledShader {
                name: String::from("k"),
                source_hash: 0,
            },
            &[],
            (1, 1, 1),
            (1, 1, 1),
        )
        .expect_err("dispatch must not silently drop work");
    assert!(err.is_unimplemented(), "got {err:?}");
}

#[test]
fn test_no_fabricated_device_when_detection_fails() {
    // fallback_device() must return no devices rather than inventing an
    // "Apple GPU". The whole body of this test used to sit under
    // cfg(not(target_os = "macos")), so on macOS -- the ONLY host where
    // fallback_device() is reachable at all -- it was empty and passed
    // vacuously.
    //
    // It calls detect::fallback_device directly. The MetalCompute wrapper it
    // used to call was dead code (the macOS-only dead-code warning that
    // contradicted the README's "Clippy: 0 warnings") and is gone.
    //
    // The same anti-fabrication property is ALSO asserted cross-platform by
    // parse_tests::test_parse_invents_no_device_when_nothing_is_reported,
    // which needs no macOS host.
    #[cfg(target_os = "macos")]
    {
        assert!(
            detect::fallback_device().is_empty(),
            "fallback_device() must fabricate nothing; it invented an \"Apple GPU\" in 0.2.0"
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        assert!(
            MetalCompute::devices().is_empty(),
            "must not fabricate a Metal device on a platform without Metal"
        );
        assert!(MetalCompute::default_device().is_err());
    }
}

#[test]
fn test_convenience_function() {
    assert_eq!(is_available(), MetalCompute::is_available());
}

#[test]
#[cfg(target_os = "macos")]
fn test_detect_real_gpus() {
    // Device enumeration via system_profiler is one of the few genuinely
    // implemented paths in this module. If it returns nothing, that is a
    // real detection failure and must be visible, not papered over with a
    // fabricated fallback device.
    let devices = MetalCompute::devices();
    if !system_profiler_named_a_gpu() {
        // A headless host (GitHub's macOS runner is one) genuinely has no GPU
        // to enumerate. The honest assertion there is that we invented none --
        // and that is NOT vacuous: it fails against the fabricated fallback
        // device this release removed.
        assert!(
            devices.is_empty(),
            "system_profiler named no GPU, so any device here was invented: {devices:?}"
        );
        return;
    }
    assert!(
        !devices.is_empty(),
        "system_profiler named a GPU but devices() returned none"
    );

    // Device name should be real, not stub
    let first = &devices[0];
    assert!(
        // The fabricated fallback names, not an arbitrary vendor string. The
        // old guard was `!name.contains("Intel UHD")`, which a device named
        // "Apple GPU" or "Unknown GPU" passes -- so it did not guard against
        // the fabrication it was written to catch.
        first.name != "Apple GPU" && first.name != "Unknown GPU",
        "detected the fabricated fallback device name, not a real GPU. Got: {}",
        first.name
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_detect_gpu_vram() {
    // The `if !devices.is_empty()` guard this test used to carry made it
    // VACUOUS: replace `MetalCompute::devices()` with `Vec::new()` and the
    // body never runs, so the test passes green over a constant. That is the
    // F3 shape -- a test that cannot fail is not evidence -- and it is the
    // shape that let 0.2.0's fabrications through a green suite.
    //
    // It does not assert non-emptiness unconditionally. An earlier revision
    // did, on the reasoning that `test_detect_real_gpus` indexes `devices[0]`
    // unguarded anyway -- but that sibling is now host-aware too, so the
    // reasoning no longer holds and this comment used to still assert it.
    let devices = MetalCompute::devices();
    if !system_profiler_named_a_gpu() {
        assert!(
            devices.is_empty(),
            "system_profiler named no GPU, so any device here was invented"
        );
        return;
    }
    assert!(
        !devices.is_empty(),
        "system_profiler named a GPU but devices() returned none"
    );

    let first = &devices[0];
    // At least 1 GiB. Note this is NOT necessarily a measurement: on Apple
    // Silicon system_profiler prints no VRAM line and the figure is manzana's
    // 16 GiB constant, so on that path this asserts the constant is sane
    // rather than that the device reported anything. `reported_vram_bytes`
    // is what separates the two, and parse_tests pins that directly.
    assert!(
        first.vram_gb() >= 1.0,
        "GPU should report at least 1 GiB VRAM, got: {} GiB",
        first.vram_gb()
    );
}

#[test]
fn test_metal_buffer_methods() {
    let buffer = MetalBuffer {
        length: 1024,
        device_index: 0,
    };
    assert_eq!(buffer.len(), 1024);
    assert!(!buffer.is_empty());
    assert_eq!(buffer.device_index(), 0);

    let empty_buffer = MetalBuffer {
        length: 0,
        device_index: 0,
    };
    assert!(empty_buffer.is_empty());
}

// ---------------------------------------------------------------------------
// Pure accessors on the value types. On a host with no Metal device these are
// unreachable through `devices()`, so they are constructed directly here.
// They compute from data they were handed and claim no hardware.
// ---------------------------------------------------------------------------

fn sample_device(unified: bool, max_buffer: u64) -> MetalDevice {
    MetalDevice {
        name: String::from("Test GPU"),
        registry_id: 42,
        is_low_power: false,
        is_headless: false,
        max_threads_per_threadgroup: 1024,
        max_buffer_length: max_buffer,
        // A hand-built device reports no measured VRAM: nothing read it.
        reported_vram_bytes: None,
        has_unified_memory: unified,
        index: 0,
    }
}

#[test]
fn test_device_is_apple_silicon_follows_unified_memory() {
    assert!(sample_device(true, 1 << 30).is_apple_silicon());
    assert!(!sample_device(false, 1 << 30).is_apple_silicon());
}

#[test]
fn test_device_vram_gb_converts_bytes() {
    let one_gib = 1_073_741_824u64;
    assert!((sample_device(true, one_gib).vram_gb() - 1.0).abs() < f64::EPSILON);
    assert!((sample_device(true, one_gib * 8).vram_gb() - 8.0).abs() < f64::EPSILON);
    assert!((sample_device(true, 0).vram_gb() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_compiled_shader_name_accessor() {
    let sh = CompiledShader {
        name: String::from("vector_add"),
        source_hash: 0,
    };
    assert_eq!(sh.name(), "vector_add");
}

#[test]
fn test_compute_accessors() {
    let c = any_compute();
    // device_index is whatever it was constructed with; device_name is non-empty.
    assert_eq!(c.device_index(), 0);
    assert!(!c.device_name().is_empty());
}

#[test]
fn test_metal_buffer_accessors() {
    let b = MetalBuffer {
        length: 2048,
        device_index: 3,
    };
    assert_eq!(b.len(), 2048);
    assert!(!b.is_empty());
    assert_eq!(b.device_index(), 3);
    let empty = MetalBuffer {
        length: 0,
        device_index: 0,
    };
    assert!(empty.is_empty());
}
