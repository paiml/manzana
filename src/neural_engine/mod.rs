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
        // One arm per variant selecting a &str, rather than eight near-identical
        // `write!` calls. Same output, and the compiler still forces a new
        // variant to be handled here.
        f.write_str(match self {
            Self::Convolution => "Convolution",
            Self::MatMul => "MatMul",
            Self::Pooling => "Pooling",
            Self::Activation => "Activation",
            Self::Normalization => "Normalization",
            Self::Elementwise => "Elementwise",
            Self::Reshape => "Reshape",
            Self::Attention => "Attention",
        })
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

/// Simple tensor type for inference input/output.
#[derive(Debug, Clone)]
pub struct Tensor {
    /// Shape of the tensor (e.g., [1, 3, 224, 224]).
    pub shape: Vec<usize>,
    /// Flattened data.
    pub data: Vec<f32>,
}

/// Product of `shape`, or `None` if it overflows `usize`.
///
/// Checked, not `product()`. `shape.iter().product::<usize>()` PANICS in debug
/// (this crate denies panics in safe public API) and silently WRAPS in release,
/// so a length check against it could admit a `Tensor` whose shape misdescribes
/// its own contents. Named by the manzana-tensor-v1 obligation "new() never
/// panics, including on shape products that overflow".
fn shape_product(shape: &[usize]) -> Option<usize> {
    shape.iter().try_fold(1usize, |acc, &d| acc.checked_mul(d))
}

impl Tensor {
    /// Create a new tensor with the given shape and data.
    ///
    /// # Errors
    ///
    /// Returns an error if data length doesn't match shape.
    #[provable_contracts_macros::contract("manzana-tensor-v1", equation = "new")]
    pub fn new(shape: Vec<usize>, data: Vec<f32>) -> Result<Self> {
        let expected_len = shape_product(&shape).ok_or_else(|| {
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
        let len: usize = shape_product(&shape).unwrap_or(usize::MAX);
        Self {
            shape,
            data: vec![0.0; len],
        }
    }

    /// The product of [`shape`](Self::shape), **saturating** at `usize::MAX`.
    ///
    /// For any `Tensor` this crate hands you it equals `data.len()`:
    /// [`Tensor::new`] rejects an overflowing shape. It can still saturate,
    /// because the fields are public and you may build a `Tensor` whose shape
    /// does not describe its data -- there `usize::MAX` is a saturation
    /// marker, not a count.
    ///
    /// ```
    /// use manzana::Tensor;
    ///
    /// let bogus = Tensor { shape: vec![usize::MAX, 2], data: vec![] };
    /// assert_eq!(bogus.numel(), usize::MAX);
    /// assert_eq!(bogus.data.len(), 0);
    /// ```
    #[must_use]
    pub fn numel(&self) -> usize {
        shape_product(&self.shape).unwrap_or(usize::MAX)
    }

    /// Get the number of dimensions.
    #[must_use]
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }
}

/// Neural Engine inference session.
///
/// **A handle that refuses.** It does not provide access to the Neural Engine,
/// and there is no CPU fallback: [`load`](Self::load) and [`infer`](Self::infer)
/// return [`Error::Unimplemented`] on every platform, Intel Macs included.
///
/// Until 0.3.0 this doc read "Provides access to Apple's Neural Engine for
/// running CoreML models. On systems without ANE (Intel Macs), this gracefully
/// falls back to CPU execution." No such access and no such fallback has ever
/// existed in this crate.
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
    /// Earlier versions returned the M1 baseline of 15.8 TOPS and 16 cores on
    /// *every* Apple Silicon machine, with `chip_generation` left as the
    /// literal string `"Unknown"`. Those were published specification figures
    /// for one chip, presented as though they had been read from the device in
    /// front of the caller. A program sizing a workload from them would be
    /// wrong on every chip but an M1.
    ///
    /// Because this returns `Option`, there is deliberately no `Default` on
    /// [`AneCapabilities`] for `unwrap_or_default()` to reach, and no
    /// constructor that hands out a chip's published figures either. manzana
    /// states no TOPS or core count for any Apple part: it cannot measure one,
    /// and it declines to repeat one.
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
    /// - [`Error::InvalidInput`] if `model_path` does not end in `.mlmodel` or
    ///   `.mlmodelc`.
    /// - [`Error::Unimplemented`] in every other case.
    ///
    /// It does **not** return an error for a missing or corrupt file, because
    /// it never opens one: the extension check is the only thing that happens
    /// before the refusal. This list previously promised "the model file
    /// doesn't exist" and "the model is corrupted", which described a
    /// filesystem check the function does not perform -- a doc claiming work
    /// that is not done, which is the defect class this release exists to
    /// remove.
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
