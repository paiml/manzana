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
    /// Construct a session for tests only, bypassing `load`.
    ///
    /// `load` always fails in this release, so no session can be obtained
    /// through the public API and `infer`/`model_path` are unreachable from
    /// any test. Mutation testing on the (now deleted) secure_enclave module
    /// showed what that costs: `delete -> Ok(())` survived because nothing
    /// could reach it. An unreachable method is not a safe method -- if a
    /// construction path returns, these bodies go live exactly as they are.
    #[cfg(test)]
    fn for_refusal_tests(model_path: &str) -> Self {
        Self {
            model_path: model_path.to_string(),
            _not_send_sync: std::marker::PhantomData,
        }
    }

    /// Check if Neural Engine is available on this system.
    ///
    /// Returns `true` on Apple Silicon Macs, `false` on Intel Macs.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_available() -> bool {
        // A compile-time target check, NOT a probe. No sysctl call exists;
        // an earlier comment here claimed one, which is the same
        // description-versus-behaviour gap the 0.3.0 release exists to close.
        // Sound as a presence claim because every Apple Silicon part ships an
        // ANE -- recorded and justified in manzana-charter.toml under
        // [capability_predicates].
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
mod tests;
