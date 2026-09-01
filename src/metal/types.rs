//! Value types describing an enumerated Metal device, a compiled
//! shader handle, and a buffer handle.
//!
//! Split out of `mod.rs` because their documentation is substantial: several
//! fields look like device properties and are not, and saying so precisely
//! takes more room than the fields themselves.

/// A GPU as reported by `system_profiler SPDisplaysDataType`.
///
/// Only [`name`](Self::name) and, when the report supplies one,
/// [`max_buffer_length`](Self::max_buffer_length) come from that report. The
/// other six fields are constants, derivations from the name and the build
/// target, or positions in the enumeration; each field's documentation says
/// which. Nothing here was queried from a Metal device.
///
/// Every field is public and the struct is not `#[non_exhaustive]`, so callers
/// can build one directly — which is how the crate's own tests exercise the
/// accessors on hosts that have no GPU.
///
/// # Examples
///
/// ```
/// use manzana::metal::MetalDevice;
///
/// let device = MetalDevice {
///     name: "Apple M4".to_string(),
///     registry_id: 1,
///     is_low_power: false,
///     is_headless: false,
///     max_threads_per_threadgroup: 1024,
///     max_buffer_length: 16 * 1024 * 1024 * 1024,
///     // None: nothing read this figure. On a real M4 this is also None,
///     // because system_profiler prints no VRAM line for unified memory.
///     reported_vram_bytes: None,
///     has_unified_memory: true,
///     index: 0,
/// };
///
/// assert_eq!(device.vram_gb(), 16.0);
/// assert!(device.is_apple_silicon());
/// ```
#[derive(Debug, Clone)]
pub struct MetalDevice {
    /// Device name as `system_profiler` printed it, with the trailing `:`
    /// removed.
    ///
    /// The one field read verbatim from the report.
    pub name: String,
    /// The device's position in the enumeration, plus one.
    ///
    /// **Synthesized, not queried.** This is not an IOKit registry ID and is
    /// not a property of the hardware: it is assigned from the order
    /// `system_profiler` happened to list the devices, so it changes if that
    /// order changes and collides across machines. manzana makes no IOKit call
    /// for Metal devices. Do not persist it or use it as a device identity.
    pub registry_id: u64,
    /// Whether this looks like an integrated GPU.
    ///
    /// **Derived from the name string:** `true` when [`name`](Self::name)
    /// contains `Intel` or `Integrated`. The device is not asked, and no
    /// power measurement is involved.
    pub is_low_power: bool,
    /// Whether the GPU drives no display.
    ///
    /// **Always `false`.** manzana does not determine this and the
    /// `system_profiler` report is not consulted for it, so `false` here
    /// carries no information about the device.
    pub is_headless: bool,
    /// Maximum threads per threadgroup.
    ///
    /// **Always the literal `1024`**, for every device on every platform. 1024
    /// is the figure Apple documents for current Apple GPUs, so it is usually
    /// the right number — but it is a specification figure copied into the
    /// struct, and this crate never queries the device or checks the device
    /// against it. Treat it as documentation, not as a measurement.
    pub max_threads_per_threadgroup: u32,
    /// Usable buffer bytes, as an approximation of VRAM.
    ///
    /// Set from the report's `VRAM` line for this device when there is one and
    /// its value parses as a whole number of `GB` or `MB`. Otherwise it is a
    /// hardcoded fallback: 16 GiB when the name contains `Apple` or the crate
    /// was built for `aarch64`, and 4 GiB otherwise. A fractional figure such
    /// as `1.5 GB` does not parse and takes the fallback.
    ///
    /// It is not a queried `maxBufferLength`, and nothing in manzana enforces
    /// it as a limit. To find out which of the two you got, read
    /// [`reported_vram_bytes`](Self::reported_vram_bytes) — until 0.3.0 the
    /// value alone did not tell you, and both shipped examples consequently
    /// printed the fallback constant under the label "(from system_profiler)".
    pub max_buffer_length: u64,
    /// The VRAM figure `system_profiler` actually printed, if it printed one.
    ///
    /// `Some(bytes)` when the report carried a `VRAM` line for this device that
    /// parsed as a whole number of `GB` or `MB`. `None` when it carried none,
    /// or one that did not parse — in which case
    /// [`max_buffer_length`](Self::max_buffer_length) is manzana's hardcoded
    /// default and describes no hardware.
    ///
    /// **`None` is the normal case on Apple Silicon**, which reports unified
    /// memory and no VRAM line at all. So on an M-series Mac
    /// `max_buffer_length` is the 16 GiB constant regardless of how much memory
    /// the machine has, and this field is what says so.
    ///
    /// This field exists because provenance has to be representable before it
    /// can be reported honestly. Both shipped examples labelled the fallback
    /// "(from system_profiler)" and the README carried that output as a
    /// captured M4 result — a crate constant presented as a hardware
    /// measurement with a named source, which is the defect class
    /// RUSTSEC-2026-0273 was filed over.
    pub reported_vram_bytes: Option<u64>,
    /// Whether the chip has a unified memory architecture.
    ///
    /// **Derived:** `true` when [`name`](Self::name) contains `Apple` or the
    /// crate was compiled for `aarch64` — so on an `aarch64` build, every
    /// enumerated device reports `true` whatever it is.
    ///
    /// This is a claim about the hardware, not about manzana. It does not mean
    /// this crate can give you a GPU-visible buffer; it cannot. See the
    /// module's [Unified memory](crate::metal#unified-memory) section.
    pub has_unified_memory: bool,
    /// Position in the vector returned by [`MetalCompute::devices`](crate::MetalCompute::devices).
    ///
    /// The value to pass to [`MetalCompute::new`](crate::MetalCompute::new). Valid only for the list this
    /// device came from — `devices()` re-enumerates on every call.
    pub index: usize,
}

impl MetalDevice {
    /// Returns [`has_unified_memory`](Self::has_unified_memory) unchanged.
    ///
    /// An alias for that field, with the same derivation and the same caveats:
    /// it follows the device name and the build target, and no device was
    /// asked. On an `aarch64` build it is `true` for every enumerated device.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalCompute;
    ///
    /// for device in MetalCompute::devices() {
    ///     assert_eq!(device.is_apple_silicon(), device.has_unified_memory);
    /// }
    /// ```
    #[must_use]
    pub const fn is_apple_silicon(&self) -> bool {
        self.has_unified_memory
    }

    /// [`max_buffer_length`](Self::max_buffer_length) in gibibytes.
    ///
    /// A division by 1 GiB (1_073_741_824). It carries exactly what
    /// `max_buffer_length` carries — a parsed report figure or a hardcoded
    /// fallback — and adds no measurement of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// use manzana::metal::MetalDevice;
    ///
    /// let mut device = MetalDevice {
    ///     name: "Test GPU".to_string(),
    ///     registry_id: 1,
    ///     is_low_power: false,
    ///     is_headless: false,
    ///     max_threads_per_threadgroup: 1024,
    ///     max_buffer_length: 8 * 1024 * 1024 * 1024,
    ///     reported_vram_bytes: Some(8 * 1024 * 1024 * 1024),
    ///     has_unified_memory: false,
    ///     index: 0,
    /// };
    /// assert_eq!(device.vram_gb(), 8.0);
    ///
    /// device.max_buffer_length = 0;
    /// assert_eq!(device.vram_gb(), 0.0);
    /// ```
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn vram_gb(&self) -> f64 {
        self.max_buffer_length as f64 / 1_073_741_824.0
    }
}

/// A handle to a compiled Metal shader.
///
/// No value of this type can be obtained through the public API.
/// [`MetalCompute::compile_shader`](crate::MetalCompute::compile_shader) is the only function that returns one, and
/// it always fails. The type exists so the signature a real Metal backend would
/// fill in is already in place.
#[derive(Debug)]
pub struct CompiledShader {
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) source_hash: u64,
}

impl CompiledShader {
    /// The kernel function name this handle was created for.
    ///
    /// Unreachable from outside the crate: see the type-level note.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A handle to a Metal buffer.
///
/// Holds a length and a device index. It owns no memory, on the GPU or
/// anywhere else. No value of this type can be obtained through the public API:
/// [`MetalCompute::allocate_buffer`](crate::MetalCompute::allocate_buffer) is the only function that returns one, and
/// it always fails.
///
/// For an allocation that does exist, see [`crate::unified_memory::UmaBuffer`]
/// — page-aligned host memory, freed on drop, and not GPU-visible.
#[derive(Debug)]
pub struct MetalBuffer {
    pub(crate) length: usize,
    pub(crate) device_index: usize,
}

impl MetalBuffer {
    /// The length in bytes recorded in this handle.
    ///
    /// A recorded number, not the size of an allocation; no memory is held.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Whether the recorded length is zero.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// The device index recorded in this handle.
    #[must_use]
    pub const fn device_index(&self) -> usize {
        self.device_index
    }
}
