//! Tests for the `neural_engine` module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;

// Tests for the `neural_engine` module.

// F031/F032: Platform detection.
//
// The previous version bound the result and discarded it, so it asserted
// nothing and could not fail. Detection here is a compile-time target
// check, so its expected value is known exactly on every platform.
#[test]
fn test_is_available_matches_build_target() {
    let expected = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    assert_eq!(
        NeuralEngineSession::is_available(),
        expected,
        "ANE availability must follow the build target exactly"
    );
}

#[test]
fn test_capabilities_never_fabricates_device_specs() {
    // Must be None on every platform: earlier versions returned the M1
    // baseline (15.8 TOPS, 16 cores) on any Apple Silicon chip, which is
    // a published figure for one device presented as a measurement of
    // whichever device the caller happens to be running on.
    assert!(
        NeuralEngineSession::capabilities().is_none(),
        "capability querying is not implemented; it must not guess"
    );
}

/// manzana must hand out no chip specification at all.
///
/// `capabilities()` returns `Option`, so an `impl Default for AneCapabilities`
/// made `capabilities().unwrap_or_default()` -- an unremarkable line of Rust
/// -- yield "15.8 TOPS, 16 cores" on any machine, including x86_64 Linux with
/// no Apple silicon in it. That impl was deleted and the figures moved behind
/// a constructor called `m1_baseline()`.
///
/// Then the review pointed out that 15.8 TOPS is the M2's published figure,
/// not the M1's. The constructor's whole justification was "the caller
/// deliberately wants the documented M1 baseline"; with the wrong chip's
/// number in it there was no justification left, and nothing in the crate
/// used it. It is gone too.
///
/// So this asserts the end state: no measurement, and no repeated vendor
/// specification either. There is nothing for a caller to mistake for one.
#[test]
fn test_no_chip_specification_is_reachable() {
    assert!(
        NeuralEngineSession::capabilities().is_none(),
        "capability querying is not implemented and must not guess"
    );

    // AneCapabilities survives as the SHAPE a real implementation would fill
    // in. Constructing one requires supplying every figure yourself, so any
    // number in it is the caller's claim, not manzana's.
    let caller_supplied = AneCapabilities {
        tops: 0.0,
        max_batch_size: 0,
        supported_ops: Vec::new(),
        chip_generation: String::from("unknown"),
        core_count: 0,
    };
    assert!(caller_supplied.tops.abs() < f64::EPSILON);
}

#[test]
fn test_ane_op_display() {
    assert_eq!(AneOp::Convolution.to_string(), "Convolution");
    assert_eq!(AneOp::MatMul.to_string(), "MatMul");
    assert_eq!(AneOp::Attention.to_string(), "Attention");
}

#[test]
fn test_tensor_new_valid() {
    let tensor = Tensor::new(vec![2, 3], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert!(tensor.is_ok());
    let tensor = tensor.unwrap();
    assert_eq!(tensor.numel(), 6);
    assert_eq!(tensor.ndim(), 2);
}

#[test]
fn test_tensor_new_invalid_size() {
    let tensor = Tensor::new(vec![2, 3], vec![1.0, 2.0]); // Wrong size
    assert!(tensor.is_err());
}

#[test]
fn test_tensor_zeros() {
    let tensor = Tensor::zeros(vec![2, 3, 4]);
    assert_eq!(tensor.numel(), 24);
    assert_eq!(tensor.ndim(), 3);
    assert!(tensor.data.iter().all(|&x| x == 0.0));
}

#[test]
fn test_load_is_unimplemented_not_fabricated() {
    // A well-formed path with a valid extension must still refuse: the
    // model is never opened, so reporting a loaded session would be a
    // statement about the filename, not the model.
    let err = NeuralEngineSession::load(Path::new("/nonexistent/model.mlmodel"))
        .expect_err("model loading is not implemented");
    assert!(err.is_unimplemented(), "got {err:?}");
}

#[test]
fn test_load_rejects_bad_extension_distinctly() {
    let err = NeuralEngineSession::load(Path::new("notes.txt"))
        .expect_err("wrong extension must be rejected");
    assert!(matches!(err, Error::InvalidInput { .. }), "got {err:?}");
}

#[test]
fn test_infer_refuses_rather_than_returning_zeros() {
    // Guards the specific defect: infer() must not return a shaped
    // all-zero tensor that a caller could mistake for a real result.
    // No session can be constructed, so the refusal is enforced upstream.
    assert!(NeuralEngineSession::load(Path::new("model.mlmodelc")).is_err());
}

#[test]
fn test_convenience_function() {
    assert_eq!(is_available(), NeuralEngineSession::is_available());
}

#[test]
fn test_ane_op_equality() {
    assert_eq!(AneOp::Convolution, AneOp::Convolution);
    assert_ne!(AneOp::Convolution, AneOp::MatMul);
}

#[test]
fn test_ane_op_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(AneOp::Convolution);
    set.insert(AneOp::MatMul);
    assert_eq!(set.len(), 2);
}

// Proves the `#[contract]` annotation on `Tensor::new` is BOUND.
//
// The macro expands to `option_env!("CONTRACT_MANZANA_TENSOR_V1_NEW")`.
// With no build script that is `None` and the annotation proves nothing --
// which is exactly what manzana shipped. build.rs now sets the variable
// only after checking the contract file and equation both exist, so a
// `Some` here is evidence the binding resolved.

#[test]
fn test_contract_binding_is_live() {
    assert_eq!(
        option_env!("CONTRACT_MANZANA_TENSOR_V1_NEW"),
        Some("bound"),
        "the manzana-tensor-v1/new contract binding did not resolve; the \
         #[contract] attribute on Tensor::new would be decorative"
    );
}

#[test]
fn test_ane_op_display_covers_every_variant() {
    let all = [
        (AneOp::Convolution, "Convolution"),
        (AneOp::MatMul, "MatMul"),
        (AneOp::Pooling, "Pooling"),
        (AneOp::Activation, "Activation"),
        (AneOp::Normalization, "Normalization"),
        (AneOp::Elementwise, "Elementwise"),
        (AneOp::Reshape, "Reshape"),
        (AneOp::Attention, "Attention"),
    ];
    for (op, expected) in all {
        assert_eq!(op.to_string(), expected);
    }
}

#[test]
fn test_infer_refuses_when_reachable() {
    let s = NeuralEngineSession::for_refusal_tests("model.mlmodelc");
    let input = Tensor::zeros(vec![1, 3, 8, 8]);
    let err = s
        .infer(&input)
        .expect_err("infer must not fabricate an output tensor");
    assert!(err.is_unimplemented(), "got {err:?}");
    // Specifically NOT a shaped all-zero tensor, which is what 0.2.0
    // returned and which a caller cannot tell from a real result.
}

#[test]
fn test_model_path_accessor() {
    let s = NeuralEngineSession::for_refusal_tests("/tmp/m.mlmodelc");
    assert_eq!(s.model_path(), "/tmp/m.mlmodelc");
}
