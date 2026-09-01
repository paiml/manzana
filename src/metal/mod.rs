//! Metal GPU device enumeration for macOS.
//!
//! This module reports which GPUs macOS says are installed. It performs no GPU
//! compute: [`MetalCompute::compile_shader`], [`MetalCompute::allocate_buffer`]
//! and [`MetalCompute::dispatch`] return [`Error::Unimplemented`] on every
//! call, on every platform, for every argument.
//!
//! manzana does not link, load, or call the Metal framework. The only Apple
//! framework this crate links is IOKit, and nothing here uses it. The `metal`
//! Cargo feature gates nothing in this module; there is no `#[cfg(feature)]`
//! anywhere in `src/`.
//!
//! # What enumeration actually does
//!
//! On macOS, [`MetalCompute::devices`] runs the `system_profiler
//! SPDisplaysDataType` child process and parses its human-readable text. On
//! every other platform it returns an empty vector without spawning anything.
//!
//! Two things are read from that report: a device **name**, and a **VRAM**
//! figure when the report gives one for that device as a whole number of `GB`
//! or `MB`. Every other field of [`MetalDevice`] is a constant, is derived from
//! the name string and the build target, or is assigned by the enumeration
//! itself. The per-field documentation says which, for each one. No field is
//! queried from a Metal device, because no Metal device is ever opened.
//!
//! The parser is a flat line scan with no awareness of the report's nesting: it
//! trims each line's indentation, then treats any line ending in `:` as the
//! start of a new device unless that line begins with `Graphics` or contains
//! `Displays`, `VRAM`, `Vendor`, `Device`, `Bus`, `Slot`, or `Metal`. Any other
//! `:`-terminated line anywhere in the report — including one nested beneath a
//! device — becomes another [`MetalDevice`]. The parsing code is not a separate
//! function and no test feeds it known input, so the device list has never been
//! checked against a report the crate did not read from the machine it was
//! running on.
//!
//! When `system_profiler` cannot be run, exits non-zero, or yields no device,
//! `devices()` returns an empty vector. It does not substitute a placeholder.
//!
//! # Examples
//!
//! ```
//! use manzana::metal::MetalCompute;
//!
//! // Empty off macOS; on macOS, whatever `system_profiler` reported.
//! let devices = MetalCompute::devices();
//! for device in &devices {
//!     // vram_gb divides by 2^30; the unit is GiB despite the name.
//!     println!("{}: {:.1} GiB", device.name, device.vram_gb());
//! }
//! assert_eq!(MetalCompute::is_available(), !devices.is_empty());
//! ```
//!
//! # Unified memory
//!
//! [`MetalDevice::has_unified_memory`] and
//! [`crate::unified_memory::is_available`] answer different questions and
//! disagree on Apple Silicon. Both answers are correct.
//!
//! - `has_unified_memory` is a claim about the *chip*, derived from the device
//!   name and the build target. On an Apple Silicon Mac it is `true`.
//! - `unified_memory::is_available` is a claim about *manzana*, and always
//!   returns `false`: this crate cannot hand out a GPU-visible allocation.
//!
//! So a program that prints "unified memory: yes" for a device and "unified
//! memory: not available" for the subsystem is reporting both facts accurately.
//! The same distinction applies to the crate as a whole:
//! [`crate::is_acceleration_available`] reports presence, and
//! [`crate::is_acceleration_usable`] reports what can actually be driven
//! through manzana — which, for Metal, is nothing beyond enumeration.
//!
//! # Falsification claims
//!
//! What this module's tests can actually falsify:
//!
//! - `devices()` returns an empty vector on non-macOS targets, and on no target
//!   returns a fabricated placeholder device.
//! - [`MetalCompute::new`] and [`MetalCompute::default_device`] succeed exactly
//!   when enumeration reported at least one device, and an out-of-range index
//!   always fails.
//! - `compile_shader`, `allocate_buffer` and `dispatch` return an error for
//!   which [`Error::is_unimplemented`] holds. These are asserted on every
//!   platform, not only macOS.
//!
//! Two claims previously listed here are withdrawn, not restated:
//!
//! - "F046: All Metal devices enumerated" — nothing compares the list against
//!   an independent enumeration. `system_profiler` is both the input and the
//!   only oracle, so the test can confirm the list is non-empty on macOS and
//!   nothing more.
//! - "F047: Device properties accurate" — the test for it asserts `name` is
//!   non-empty and that `max_threads_per_threadgroup` and `max_buffer_length`
//!   exceed zero. None of those three can fail: a device is only constructed
//!   from a non-empty name, `max_threads_per_threadgroup` is the literal
//!   `1024`, and every branch of the buffer-length fallback is non-zero. No
//!   field is compared to the device it describes.
//!
//! Also not claimed, and previously listed here in error: "F053: Multi-GPU
//! dispatch works" and "F058: Headless GPU works". `dispatch()` returns
//! [`Error::Unimplemented`] unconditionally, so neither claim is falsifiable by
//! any test and neither was ever satisfied.

use crate::error::{Error, Result, Subsystem};

mod detect;
mod types;

pub use types::{CompiledShader, MetalBuffer, MetalDevice};

/// A selected Metal device.
///
/// Built by [`MetalCompute::new`] or [`MetalCompute::default_device`], which
/// validate a device index against [`MetalCompute::devices`] and store that
/// index with the device's name. No Metal object is created: the value holds a
/// `usize` and a `String`.
///
/// Every compute method on it — [`MetalCompute::compile_shader`],
/// [`MetalCompute::allocate_buffer`], [`MetalCompute::dispatch`] — returns
/// [`Error::Unimplemented`]. What the type is good for today is naming which
/// enumerated device a caller intends to use.
///
/// # Thread safety
///
/// The type is deliberately `!Send` and `!Sync`, by way of a
/// `PhantomData<*const ()>` field. It holds nothing thread-affine today; the
/// marker is there so callers structure code the way a real Metal command
/// queue, which is not thread-safe, would require. Create one per thread.
///
/// # Examples
///
/// ```
/// use manzana::metal::MetalCompute;
///
/// match MetalCompute::default_device() {
///     Ok(gpu) => println!("selected {} (index {})", gpu.device_name(), gpu.device_index()),
///     Err(e) => println!("no Metal device: {e}"),
/// }
/// ```
pub struct MetalCompute {
    device_index: usize,
    device_name: String,
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl MetalCompute {
    /// Enumerate the GPUs macOS reports.
    ///
    /// On macOS this spawns `system_profiler SPDisplaysDataType` and parses its
    /// text output; the module documentation gives the parsing rule and lists
    /// which [`MetalDevice`] fields come from the report and which do not. On
    /// every other platform this returns an empty vector without spawning
    /// anything.
    ///
    /// Returns an empty vector — never a placeholder device — when
    /// `system_profiler` cannot be run, exits non-zero, or yields no device.
    ///
    /// Nothing is cached: every call re-runs the child process on macOS, so
    /// indices are only valid against the list they came from.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// let devices = MetalCompute::devices();
    /// for (i, device) in devices.iter().enumerate() {
    ///     assert_eq!(device.index, i);
    ///     println!("GPU {i}: {}", device.name);
    /// }
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn devices() -> Vec<MetalDevice> {
        #[cfg(target_os = "macos")]
        {
            Self::detect_gpus_via_system_profiler()
        }

        #[cfg(not(target_os = "macos"))]
        {
            Vec::new()
        }
    }

    /// Enumerate GPUs by parsing `system_profiler`. See [`detect`].
    ///
    /// macOS only: there is no `system_profiler` elsewhere, so `devices()`
    /// returns an empty vector on other targets without calling this.
    #[cfg(target_os = "macos")]
    fn detect_gpus_via_system_profiler() -> Vec<MetalDevice> {
        detect::detect_gpus_via_system_profiler()
    }

    /// Whether enumeration reported at least one device.
    ///
    /// Equivalent to `!devices().is_empty()`, and it enumerates the same way —
    /// spawning `system_profiler` on macOS, returning `false` immediately
    /// elsewhere.
    ///
    /// `true` means a GPU was reported, not that anything can be done with it:
    /// every compute operation in this module returns
    /// [`Error::Unimplemented`]. For "can manzana actually drive an
    /// accelerator", see [`crate::is_acceleration_usable`].
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// assert_eq!(MetalCompute::is_available(), !MetalCompute::devices().is_empty());
    /// ```
    #[must_use]
    pub fn is_available() -> bool {
        !Self::devices().is_empty()
    }

    /// Select an enumerated device by index.
    ///
    /// Validates `device_index` against [`MetalCompute::devices`] and records
    /// the index and that device's name. No Metal object is created and no
    /// pipeline is compiled, despite the name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] when `device_index` is not less than the
    /// number of devices enumeration reports. On a host with no Metal device —
    /// every non-macOS target — that is every index, including `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// // No enumeration can produce this many devices.
    /// let result = MetalCompute::new(usize::MAX);
    /// assert!(result.is_err());
    /// if let Err(err) = result {
    ///     assert!(err.to_string().starts_with("resource not found"));
    /// }
    /// ```
    pub fn new(device_index: usize) -> Result<Self> {
        let devices = Self::devices();
        if device_index >= devices.len() {
            return Err(Error::not_found(format!(
                "Metal device index {device_index} (only {} devices available)",
                devices.len()
            )));
        }

        Ok(Self {
            device_index,
            device_name: devices[device_index].name.clone(),
            _not_send_sync: std::marker::PhantomData,
        })
    }

    /// Select the first enumerated device.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotAvailable`] for [`Subsystem::Metal`] when
    /// enumeration reported no devices, which is always the case off macOS.
    /// There is no other failure: index `0` is valid whenever the list is
    /// non-empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// match MetalCompute::default_device() {
    ///     Ok(gpu) => assert!(!gpu.device_name().is_empty()),
    ///     Err(e) => assert!(e.is_not_available()),
    /// }
    /// ```
    pub fn default_device() -> Result<Self> {
        if Self::devices().is_empty() {
            return Err(Error::not_available(Subsystem::Metal));
        }
        Self::new(0)
    }

    /// The name of the selected device.
    ///
    /// The string copied from the [`MetalDevice`] at selection time; it is not
    /// re-read from the system afterwards.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// The index this pipeline was created with.
    ///
    /// An index into the enumeration that was current when
    /// [`MetalCompute::new`] ran.
    #[must_use]
    pub const fn device_index(&self) -> usize {
        self.device_index
    }

    /// Compile a Metal Shading Language kernel. **Not implemented.**
    ///
    /// # Arguments
    ///
    /// * `source` — MSL source code. Ignored.
    /// * `function_name` — kernel entry point. Ignored.
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`], on every platform and for every
    /// argument. Nothing parses the source, so invalid MSL is refused for the
    /// same reason valid MSL is: a real implementation needs
    /// `MTLDevice::newLibraryWithSource`, which this crate does not call.
    ///
    /// 0.2.0 returned a `CompiledShader` whose `source_hash` was a 64-bit
    /// string hash of the source. Nothing was compiled, so invalid MSL was
    /// accepted as readily as valid MSL.
    ///
    /// # Examples
    ///
    /// The refusal, on a host that has a device to select:
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// if let Ok(gpu) = MetalCompute::default_device() {
    ///     let err = gpu
    ///         .compile_shader("kernel void add() {}", "add")
    ///         .expect_err("shader compilation is not implemented");
    ///     assert!(err.is_unimplemented());
    ///     assert_eq!(
    ///         err.to_string(),
    ///         "operation not implemented: shader compilation \
    ///          (requires MTLDevice::newLibraryWithSource) (Metal GPU)"
    ///     );
    /// }
    /// ```
    pub fn compile_shader(&self, source: &str, function_name: &str) -> Result<CompiledShader> {
        let _ = (source, function_name);
        Err(Error::unimplemented(
            Subsystem::Metal,
            "shader compilation (requires MTLDevice::newLibraryWithSource)",
        ))
    }

    /// Allocate a buffer on the GPU. **Not implemented.**
    ///
    /// # Arguments
    ///
    /// * `length` — size in bytes. Ignored.
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`], on every platform and for every
    /// length, including zero. A real implementation needs
    /// `MTLDevice::newBufferWithLength`, which this crate does not call.
    ///
    /// 0.2.0 returned a `MetalBuffer` holding only a length and a device index.
    /// No GPU memory — in fact no memory at all — was allocated, so writes had
    /// nowhere to go and reads had nothing to return.
    ///
    /// For host memory you can actually use, see
    /// [`crate::unified_memory::UmaBuffer`]. It is not GPU-visible.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// if let Ok(gpu) = MetalCompute::default_device() {
    ///     let err = gpu
    ///         .allocate_buffer(1024)
    ///         .expect_err("buffer allocation is not implemented");
    ///     assert!(err.is_unimplemented());
    ///     assert_eq!(
    ///         err.to_string(),
    ///         "operation not implemented: buffer allocation \
    ///          (requires MTLDevice::newBufferWithLength) (Metal GPU)"
    ///     );
    /// }
    /// ```
    pub fn allocate_buffer(&self, length: usize) -> Result<MetalBuffer> {
        let _ = length;
        Err(Error::unimplemented(
            Subsystem::Metal,
            "buffer allocation (requires MTLDevice::newBufferWithLength)",
        ))
    }

    /// Dispatch a compute kernel. **Not implemented.**
    ///
    /// # Arguments
    ///
    /// * `shader` — compiled kernel. Ignored.
    /// * `buffers` — buffers to bind. Ignored.
    /// * `grid_size` — total threads, as (width, height, depth). Ignored, and
    ///   not validated.
    /// * `threadgroup_size` — threads per threadgroup. Ignored, and not checked
    ///   against [`MetalDevice::max_threads_per_threadgroup`].
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`], on every platform and for every
    /// argument. A real implementation needs `MTLCommandBuffer` and
    /// `MTLComputeCommandEncoder`, which this crate does not call.
    ///
    /// 0.2.0 validated the grid and threadgroup arguments and then returned
    /// `Ok(())` having dispatched nothing. A caller would read its output
    /// buffer and find whatever was there before — silently wrong results
    /// rather than a reported failure.
    ///
    /// # Examples
    ///
    /// None, because this method cannot be called from outside the crate: it
    /// requires a [`CompiledShader`], and the only function that returns one is
    /// [`MetalCompute::compile_shader`], which always fails. The refusal is
    /// asserted in this module's own tests, which construct the handle
    /// directly.
    pub fn dispatch(
        &self,
        shader: &CompiledShader,
        buffers: &[&MetalBuffer],
        grid_size: (u32, u32, u32),
        threadgroup_size: (u32, u32, u32),
    ) -> Result<()> {
        let _ = (shader, buffers, grid_size, threadgroup_size);
        Err(Error::unimplemented(
            Subsystem::Metal,
            "compute dispatch (requires MTLCommandBuffer/MTLComputeCommandEncoder)",
        ))
    }
}

/// Whether enumeration reported at least one Metal device.
///
/// Equivalent to [`MetalCompute::is_available`], and enumerates the same way on
/// each call. It reports presence, not usability: every compute operation in
/// this module returns [`Error::Unimplemented`].
///
/// # Examples
///
/// ```
/// assert_eq!(manzana::metal::is_available(), !manzana::metal::MetalCompute::devices().is_empty());
/// ```
#[must_use]
pub fn is_available() -> bool {
    MetalCompute::is_available()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod parse_tests;
