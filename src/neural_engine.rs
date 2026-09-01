//! Apple Neural Engine (ANE) inference sessions.
//!
//! # ⚠️ INFERENCE IS NOT IMPLEMENTED ⚠️
//!
//! This module can detect that an Apple Neural Engine is present. It cannot
//! load a CoreML model or run inference; those operations return
//! [`Error::Unimplemented`].
//!
//! Versions 0.1.0 and 0.2.0 (both **yanked**) returned
//! `Tensor::zeros(input.shape)` from [`NeuralEngineSession::infer`] — a
//! correctly-shaped all-zero tensor, returned silently, indistinguishable from
//! a model whose real output happened to be zeros. See
//! `docs/specifications/security-architecture-plan.md`.
//!
//! The Apple Neural Engine is a dedicated machine learning accelerator
//! available on Apple Silicon Macs.
//!
//! # Example
//!
//! ```
//! use manzana::neural_engine::NeuralEngineSession;
//!
//! // Detection is real; inference is not implemented.
//! let _present = NeuralEngineSession::is_available();
//! ```
//!
//! # Falsification Claims
//!
//! - F031: ANE detected on Apple Silicon
//! - F032: Returns None on Intel Mac

use crate::error::{Error, Result};
use std::path::Path;

/// Apple Neural Engine operation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AneOp {
    /// Convolution operations.
    Convolution,
    /// Matrix multiplication.
    MatMul,
    /// Pooling operations (max, average).
    Pooling,
    /// Activation functions (ReLU, sigmoid, etc.).
    Activation,
    /// Normalization (batch norm, layer norm).
    Normalization,
    /// Element-wise operations.
    Elementwise,
    /// Reshape and transpose.
    Reshape,
    /// Attention mechanisms.
    Attention,
}

impl std::fmt::Display for AneOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Convolution => write!(f, "Convolution"),
            Self::MatMul => write!(f, "MatMul"),
            Self::Pooling => write!(f, "Pooling"),
            Self::Activation => write!(f, "Activation"),
            Self::Normalization => write!(f, "Normalization"),
            Self::Elementwise => write!(f, "Elementwise"),
            Self::Reshape => write!(f, "Reshape"),
            Self::Attention => write!(f, "Attention"),
        }
    }
}

/// Capabilities of the Apple Neural Engine.
#[derive(Debug, Clone)]
pub struct AneCapabilities {
    /// Tera operations per second.
    pub tops: f64,
    /// Maximum batch size supported.
    pub max_batch_size: u32,
    /// Supported operations.
    pub supported_ops: Vec<AneOp>,
    /// Chip generation (M1, M2, M3, etc.).
    pub chip_generation: String,
    /// Number of neural engine cores.
    pub core_count: u32,
}

impl Default for AneCapabilities {
    fn default() -> Self {
        Self {
            tops: 15.8, // M1 baseline
            max_batch_size: 32,
            supported_ops: vec![
                AneOp::Convolution,
                AneOp::MatMul,
                AneOp::Pooling,
                AneOp::Activation,
                AneOp::Normalization,
                AneOp::Elementwise,
                AneOp::Reshape,
                AneOp::Attention,
            ],
            chip_generation: "Unknown".to_string(),
            core_count: 16,
        }
    }
}

/// Simple tensor type for inference input/output.
#[derive(Debug, Clone)]
pub struct Tensor {
    /// Shape of the tensor (e.g., [1, 3, 224, 224]).
    pub shape: Vec<usize>,
    /// Flattened data.
    pub data: Vec<f32>,
}

impl Tensor {
    /// Create a new tensor with the given shape and data.
    ///
    /// # Errors
    ///
    /// Returns an error if data length doesn't match shape.
    #[provable_contracts_macros::contract("manzana-tensor-v1", equation = "new")]
    pub fn new(shape: Vec<usize>, data: Vec<f32>) -> Result<Self> {
        // Checked, not `product()`. `shape.iter().product::<usize>()` overflows
        // for e.g. [usize::MAX, 2]: it PANICS in debug (this crate sets
        // panic = "deny", and the constructor is safe and public) and silently
        // WRAPS in release, so the length check below would compare against a
        // wrapped value and could admit a Tensor whose shape misdescribes its
        // own contents. Found by the manzana-tensor-v1 contract obligation
        // "new() never panics, including on shape products that overflow".
        let expected_len = shape
            .iter()
            .try_fold(1usize, |acc, &d| acc.checked_mul(d))
            .ok_or_else(|| {
                Error::invalid_input(format!("shape {shape:?} overflows usize when multiplied"))
            })?;
        if data.len() != expected_len {
            return Err(Error::invalid_input(format!(
                "data length {} doesn't match shape {shape:?} (expected {expected_len})",
                data.len(),
            )));
        }
        Ok(Self { shape, data })
    }

    /// Create a tensor filled with zeros.
    #[must_use]
    pub fn zeros(shape: Vec<usize>) -> Self {
        // Saturating rather than checked: `zeros` returns Self, not Result, so
        // it has no channel to report overflow. Saturating turns a panic into
        // an allocation failure, which is at least a real failure rather than a
        // wrapped length. `new` is the checked constructor.
        let len: usize = shape
            .iter()
            .try_fold(1usize, |acc, &d| acc.checked_mul(d))
            .unwrap_or(usize::MAX);
        Self {
            shape,
            data: vec![0.0; len],
        }
    }

    /// Get the total number of elements.
    #[must_use]
    pub fn numel(&self) -> usize {
        // Cannot overflow for a Tensor built by `new`, which rejects shapes
        // whose product does. Checked anyway: `zeros` saturates, and a future
        // constructor must not turn this accessor into a panic.
        self.shape
            .iter()
            .try_fold(1usize, |acc, &d| acc.checked_mul(d))
            .unwrap_or(usize::MAX)
    }

    /// Get the number of dimensions.
    #[must_use]
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }
}

/// Neural Engine inference session.
///
/// Provides access to Apple's Neural Engine for running CoreML models.
/// On systems without ANE (Intel Macs), this gracefully falls back
/// to CPU execution.
///
/// # Thread Safety
///
/// This type is `!Send` and `!Sync` because CoreML sessions are not
/// thread-safe. Create sessions on each thread that needs them.
#[derive(Debug)]
pub struct NeuralEngineSession {
    model_path: String,
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl NeuralEngineSession {
    /// Check if Neural Engine is available on this system.
    ///
    /// Returns `true` on Apple Silicon Macs, `false` on Intel Macs.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_available() -> bool {
        // Check for Apple Silicon via sysctl
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            true
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            false
        }
    }

    /// Query Neural Engine capabilities.
    ///
    /// **Always returns `None` in this release.** Capability querying is not
    /// implemented.
    ///
    /// Earlier versions returned [`AneCapabilities::default`] — the M1
    /// baseline of 15.8 TOPS and 16 cores — on *every* Apple Silicon machine,
    /// with `chip_generation` left as the literal string `"Unknown"`. Those
    /// were published specification figures for one chip, presented as though
    /// they had been read from the device in front of the caller. A program
    /// sizing a workload from them would be wrong on every chip but an M1.
    ///
    /// [`AneCapabilities::default`] remains available for callers who
    /// deliberately want the documented M1 baseline as a placeholder.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn capabilities() -> Option<AneCapabilities> {
        None
    }

    /// Load a CoreML model for inference.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to a `.mlmodel` or `.mlmodelc` file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The model file doesn't exist
    /// - The model is corrupted
    /// - The model format is unsupported
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::neural_engine::NeuralEngineSession;
    /// use std::path::Path;
    ///
    /// let err = NeuralEngineSession::load(Path::new("model.mlmodelc"))
    ///     .expect_err("CoreML model loading is not implemented");
    /// assert!(err.is_unimplemented());
    /// ```
    pub fn load(model_path: &Path) -> Result<Self> {
        // Report obviously-wrong input as such; a caller passing a .txt file
        // has a different bug from a caller hitting the unimplemented backend.
        let ext = model_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if ext != "mlmodel" && ext != "mlmodelc" {
            return Err(Error::invalid_input(format!(
                "unsupported model format: .{ext} (expected .mlmodel or .mlmodelc)"
            )));
        }

        // Earlier versions returned a session here after checking only that the
        // path existed and had the right extension. The file was never opened,
        // let alone parsed or compiled, so "model loaded successfully" was a
        // statement about a filename.
        Err(Error::unimplemented(
            crate::error::Subsystem::NeuralEngine,
            "CoreML model loading (requires MLModel compileModelAtURL)",
        ))
    }

    /// Run inference on the loaded model.
    ///
    /// # Arguments
    ///
    /// * `input` - Input tensor matching the model's expected input shape
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release. No CoreML
    /// backend exists, and this function will not fabricate an output tensor.
    ///
    /// Earlier versions returned `Tensor::zeros(input.shape)` — an
    /// all-zero tensor of the right shape, silently, as if the model had run.
    /// A caller had no way to distinguish that from a model whose genuine
    /// output happened to be zeros.
    pub fn infer(&self, input: &Tensor) -> Result<Tensor> {
        let _ = input;
        Err(Error::unimplemented(
            crate::error::Subsystem::NeuralEngine,
            "inference (requires CoreML MLModel prediction)",
        ))
    }

    /// Get the model path.
    #[must_use]
    pub fn model_path(&self) -> &str {
        &self.model_path
    }
}

/// Check if Neural Engine is available.
///
/// Convenience function equivalent to `NeuralEngineSession::is_available()`.
#[must_use]
#[allow(clippy::missing_const_for_fn)]
pub fn is_available() -> bool {
    NeuralEngineSession::is_available()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

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

    #[test]
    fn test_capabilities_legacy_shape_still_available_as_placeholder() {
        // AneCapabilities::default() remains for callers who explicitly want
        // the documented M1 baseline. That is fine: they asked for a constant.
        let baseline = AneCapabilities::default();
        assert!((baseline.tops - 15.8).abs() < f64::EPSILON);
        assert_eq!(baseline.chip_generation, "Unknown");
    }

    #[test]
    fn test_capabilities_default_values() {
        let caps = AneCapabilities::default();
        assert!(caps.tops > 0.0);
        assert!(caps.max_batch_size > 0);
        assert!(!caps.supported_ops.is_empty());
        assert!(caps.core_count > 0);
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
}

#[cfg(test)]
mod contract_binding {
    //! Proves the `#[contract]` annotation on `Tensor::new` is BOUND.
    //!
    //! The macro expands to `option_env!("CONTRACT_MANZANA_TENSOR_V1_NEW")`.
    //! With no build script that is `None` and the annotation proves nothing --
    //! which is exactly what manzana shipped. build.rs now sets the variable
    //! only after checking the contract file and equation both exist, so a
    //! `Some` here is evidence the binding resolved.

    #[test]
    fn test_contract_binding_is_live() {
        assert_eq!(
            option_env!("CONTRACT_MANZANA_TENSOR_V1_NEW"),
            Some("bound"),
            "the manzana-tensor-v1/new contract binding did not resolve; the \
             #[contract] attribute on Tensor::new would be decorative"
        );
    }
}
