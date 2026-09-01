//! End-to-end tests against whatever hardware is actually present.
//!
//! These run on every platform and assert the properties that must hold
//! EVERYWHERE, plus the platform-specific expectations that make the test
//! meaningful on each. They are deliberately not `cfg`-gated as a whole:
//! gating the interesting assertions behind `target_os` is what let the
//! fabricating implementations ship with a green Linux lane.
//!
//! Run on the full matrix with `scripts/e2e_matrix.sh`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use manzana::afterburner::AfterburnerMonitor;
use manzana::error::Error;
use manzana::metal::MetalCompute;
use manzana::neural_engine::{NeuralEngineSession, Tensor};
use manzana::unified_memory::UmaBuffer;
use std::path::Path;

/// True when this build targets macOS. Used only to state the *expected*
/// platform behaviour, never to skip an assertion.
const IS_MACOS: bool = cfg!(target_os = "macos");
const IS_APPLE_SILICON: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

// ===========================================================================
// Invariants that must hold on EVERY platform.
// ===========================================================================

/// The crate's governing rule: no operation returns a fabricated success.
///
/// Every operation that cannot reach real hardware must return
/// `Error::Unimplemented`. This is the property the whole 0.3.0 release
/// exists to establish, so it is asserted directly, on every target.
#[test]
fn e2e_no_operation_fabricates_a_result() {
    // Neural Engine: model loading and inference are not implemented.
    let err = NeuralEngineSession::load(Path::new("model.mlmodelc"))
        .expect_err("CoreML model loading is not implemented");
    assert!(err.is_unimplemented(), "load() gave {err:?}");

    // Capability querying must not invent device specs.
    assert!(
        NeuralEngineSession::capabilities().is_none(),
        "capabilities() must not report figures it did not measure"
    );

    // Metal compute: every operation refuses.
    if let Ok(compute) = MetalCompute::default_device() {
        for (what, err) in [
            (
                "compile_shader",
                compute
                    .compile_shader("kernel void k() {}", "k")
                    .expect_err("shader compilation is not implemented"),
            ),
            (
                "allocate_buffer",
                compute
                    .allocate_buffer(1024)
                    .expect_err("buffer allocation is not implemented"),
            ),
        ] {
            assert!(err.is_unimplemented(), "{what} gave {err:?}");
        }
    }
}

/// Detection must agree with enumeration. A capability predicate that
/// disagrees with what the machine actually reports is the
/// `capability-without-probe` class.
#[test]
fn e2e_detection_agrees_with_enumeration() {
    let devices = MetalCompute::devices();
    assert_eq!(
        MetalCompute::is_available(),
        !devices.is_empty(),
        "is_available() must agree with devices(); got {} device(s)",
        devices.len()
    );
    assert_eq!(
        MetalCompute::default_device().is_ok(),
        !devices.is_empty(),
        "default_device() must succeed exactly when devices exist"
    );
    // An out-of-range index always fails, on every platform.
    assert!(MetalCompute::new(devices.len() + 1).is_err());
}

/// No fabricated device may appear. `fallback_device()` used to invent an
/// "Apple GPU" when detection failed, including on hosts with no Metal.
#[test]
fn e2e_no_fabricated_devices() {
    for d in MetalCompute::devices() {
        assert!(
            !d.name.is_empty(),
            "a device with no name is not a real one"
        );
        assert_ne!(
            d.name, "Unknown GPU",
            "\"Unknown GPU\" was the fabricated fallback name"
        );
    }
    // Off macOS there is no Metal at all, so an empty list is the only honest
    // answer. Asserted on the DATA rather than on a cfg constant, so this is a
    // real comparison on every target instead of a tautology on one.
    if !IS_MACOS {
        assert!(
            MetalCompute::devices().is_empty(),
            "there is no Metal outside macOS; got {:?}",
            MetalCompute::devices()
                .iter()
                .map(|d| d.name.clone())
                .collect::<Vec<_>>()
        );
    }
}

/// `UmaBuffer` is one of the few genuinely implemented paths. Exercise it for
/// real: allocate, write, read back, and confirm the allocation is initialized.
#[test]
fn e2e_uma_buffer_is_a_real_allocation() {
    for len in [1usize, 4095, 4096, 4097, 1 << 20] {
        let mut buf = UmaBuffer::new(len).expect("host allocation should succeed");
        assert_eq!(buf.len(), len);
        assert!(
            buf.is_aligned(),
            "page alignment is the documented guarantee"
        );
        assert!(buf.allocated_size() >= len);

        // Freshly allocated memory must be initialized: `as_slice` is safe and
        // a &[u8] over uninitialized bytes is UB.
        assert!(
            buf.as_slice().iter().all(|&b| b == 0),
            "new() must return zeroed memory"
        );

        // Round-trip through the mutable view. At len == 1 the first and last
        // byte are the same byte, so write distinct values only when they are
        // distinct positions.
        let s = buf.as_mut_slice();
        s[0] = 0xAB;
        if len > 1 {
            s[len - 1] = 0xCD;
        }
        assert_eq!(buf.as_slice()[0], 0xAB);
        if len > 1 {
            assert_eq!(buf.as_slice()[len - 1], 0xCD);
        }
        // Untouched interior bytes stay zero: the write went where it was
        // aimed and nowhere else.
        if len > 2 {
            assert_eq!(buf.as_slice()[len / 2], 0);
        }
    }

    // manzana provides no GPU-visible memory on any platform.
    assert!(
        !UmaBuffer::is_uma_available(),
        "UmaBuffer is a host allocation; it is not GPU-visible anywhere"
    );
}

/// `Tensor::new` is contract-bound (manzana-tensor-v1). Exercise the contract
/// end to end, including the overflow case that used to panic.
#[test]
fn e2e_tensor_contract_holds() {
    let t = Tensor::new(vec![2, 3], vec![0.0; 6]).expect("well-formed tensor");
    assert_eq!(t.numel(), 6);
    assert_eq!(t.ndim(), 2);

    // len(data) != prod(shape) is rejected.
    assert!(Tensor::new(vec![2, 3], vec![0.0; 5]).is_err());

    // An overflowing shape product must be an error, not a panic or a wrap.
    let err = Tensor::new(vec![usize::MAX, 2], vec![0.0; 4])
        .expect_err("an overflowing shape product must be rejected");
    assert!(matches!(err, Error::InvalidInput { .. }), "got {err:?}");
}

/// Errors must be distinguishable programmatically. A caller has to be able to
/// tell "not implemented" from "invalid input" from "hardware absent".
#[test]
fn e2e_errors_are_actionable() {
    let unimpl = NeuralEngineSession::load(Path::new("m.mlmodelc")).unwrap_err();
    assert!(unimpl.is_unimplemented());
    assert!(!unimpl.is_not_available());
    assert!(!unimpl.to_string().is_empty());

    let bad_input = NeuralEngineSession::load(Path::new("notes.txt")).unwrap_err();
    assert!(matches!(bad_input, Error::InvalidInput { .. }));
    assert!(
        !bad_input.is_unimplemented(),
        "a wrong file extension is a caller bug, not a missing backend; \
         collapsing the two would hide which one happened"
    );
}

// ===========================================================================
// Platform-specific expectations. These make the run meaningful on each host
// rather than merely non-failing.
// ===========================================================================

#[test]
fn e2e_platform_expectations() {
    // Neural Engine presence follows the build target exactly.
    assert_eq!(NeuralEngineSession::is_available(), IS_APPLE_SILICON);

    // Afterburner is queried through real IOKit FFI on macOS and is absent
    // everywhere else. Presence depends on the machine, so assert only what is
    // knowable: off macOS it must be absent.
    if !IS_MACOS {
        assert!(
            !AfterburnerMonitor::is_available(),
            "there is no IOKit outside macOS"
        );
        assert!(AfterburnerMonitor::new().is_none());
    }

    // Whatever is reported, asking for stats must never panic.
    if let Some(mon) = AfterburnerMonitor::new() {
        let _ = mon.stats();
    }
}

/// On macOS, Metal enumeration is a genuinely implemented path, so it must
/// agree with what `system_profiler` actually reported on THIS host.
///
/// It must not assume a GPU exists. "Runs macOS" is not "has an enumerable
/// display GPU" -- GitHub's macos-latest runner is a headless VM that has the
/// first and not the second, and an earlier version of this assertion failed
/// there the first time the macOS lane ran. Conflating a platform with the
/// hardware attached to it is the same mistake as conflating presence with
/// reachability, which is the subject of this whole release.
///
/// Falsifiable either way: on a host with a GPU it fails against an empty
/// list, and on a headless host it fails against the fabricated fallback
/// device this release removed.
#[test]
#[cfg(target_os = "macos")]
fn e2e_macos_metal_enumeration_is_real() {
    let reported = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("Chipset Model:"));

    let devices = MetalCompute::devices();
    assert_eq!(
        !devices.is_empty(),
        reported,
        "devices() disagrees with system_profiler on this host: it named {} \
         GPU(s), devices() returned {}. Neither an empty list on real hardware \
         nor an invented device on a headless host is acceptable.",
        if reported { "at least one" } else { "no" },
        devices.len()
    );

    for d in &devices {
        assert!(d.max_threads_per_threadgroup > 0);
        assert!(d.max_buffer_length > 0);
        assert!(d.vram_gb() >= 0.0);
    }
}
