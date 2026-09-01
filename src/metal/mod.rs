//! Metal GPU compute for macOS.
//!
//! Provides access to Apple's Metal framework for GPU compute operations.
//! Supports both discrete and integrated GPUs, including Apple Silicon.
//!
//! # Example
//!
//! ```no_run
//! use manzana::metal::MetalCompute;
//!
//! // Enumerate all Metal devices
//! let devices = MetalCompute::devices();
//! for (i, device) in devices.iter().enumerate() {
//!     println!("GPU {}: {} ({} MB)", i, device.name, device.max_buffer_length / 1_000_000);
//! }
//! ```
//!
//! # Falsification Claims
//!
//! - F046: All Metal devices enumerated
//! - F047: Device properties accurate
//!
//! Not claimed, and previously listed here in error: "F053: Multi-GPU dispatch
//! works" and "F058: Headless GPU works". `dispatch()` returns
//! [`Error::Unimplemented`] unconditionally, so neither claim is falsifiable
//! by any test and neither was ever satisfied.

use crate::error::{Error, Result, Subsystem};

/// Information about a Metal GPU device.
#[derive(Debug, Clone)]
pub struct MetalDevice {
    /// Human-readable device name.
    pub name: String,
    /// Unique registry ID for the device.
    /// **Synthesized, not queried.** This is the enumeration index + 1, not the
    /// IOKit registry ID. manzana makes no IOKit call for Metal devices.
    pub registry_id: u64,
    /// True if this is a low-power (integrated) GPU.
    /// Derived from whether the device name matches an integrated part, not
    /// queried from the device.
    pub is_low_power: bool,
    /// True if this is a headless (no display) GPU.
    /// **Always `false`.** `system_profiler SPDisplaysDataType` does not report
    /// this, and manzana does not determine it.
    pub is_headless: bool,
    /// Maximum threads per threadgroup.
    /// **A hardcoded 1024**, not a device query. 1024 is the documented Metal
    /// limit for current Apple GPUs, but this value was never read from the
    /// device -- so it is a published specification figure, not a measurement.
    pub max_threads_per_threadgroup: u32,
    /// Maximum buffer length in bytes.
    /// Derived from the VRAM figure `system_profiler` reports, when it reports
    /// one. Not a queried `maxBufferLength`.
    pub max_buffer_length: u64,
    /// Unified memory architecture (Apple Silicon).
    pub has_unified_memory: bool,
    /// Device index for selection.
    pub index: usize,
}

impl MetalDevice {
    /// Check if this device supports unified memory.
    #[must_use]
    pub const fn is_apple_silicon(&self) -> bool {
        self.has_unified_memory
    }

    /// Get approximate VRAM in gigabytes.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn vram_gb(&self) -> f64 {
        self.max_buffer_length as f64 / 1_073_741_824.0
    }
}

/// A compiled Metal shader (compute kernel).
#[derive(Debug)]
pub struct CompiledShader {
    name: String,
    #[allow(dead_code)]
    source_hash: u64,
}

impl CompiledShader {
    /// Get the shader function name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A Metal buffer for GPU data.
#[derive(Debug)]
pub struct MetalBuffer {
    length: usize,
    device_index: usize,
}

impl MetalBuffer {
    /// Get the buffer length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Check if the buffer is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Get the device this buffer is allocated on.
    #[must_use]
    pub const fn device_index(&self) -> usize {
        self.device_index
    }
}

/// Metal compute pipeline.
///
/// Provides GPU compute capabilities via Apple's Metal framework.
///
/// # Thread Safety
///
/// This type is `!Send` and `!Sync` because Metal command queues
/// are not thread-safe. Create pipelines on each thread that needs them.
pub struct MetalCompute {
    device_index: usize,
    device_name: String,
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl MetalCompute {
    /// Enumerate all available Metal devices.
    ///
    /// Uses `system_profiler` to detect real GPU hardware on macOS.
    /// Returns an empty vector on non-macOS platforms.
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

    #[cfg(target_os = "macos")]
    fn detect_gpus_via_system_profiler() -> Vec<MetalDevice> {
        use std::process::Command;

        let output = match Command::new("system_profiler")
            .args(["SPDisplaysDataType"])
            .output()
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return Self::fallback_device(),
        };

        let mut devices = Vec::new();
        let mut current_name = String::new();
        let mut current_vram: u64 = 0;
        let mut index = 0;

        for line in output.lines() {
            let line = line.trim();

            // GPU name line (e.g., "AMD Radeon Pro W5700X:")
            if line.ends_with(':')
                && !line.starts_with("Graphics")
                && !line.contains("Displays")
                && !line.contains("VRAM")
                && !line.contains("Vendor")
                && !line.contains("Device")
                && !line.contains("Bus")
                && !line.contains("Slot")
                && !line.contains("Metal")
            {
                // Save previous GPU if we have one
                if !current_name.is_empty() {
                    devices.push(Self::create_device(&current_name, current_vram, index));
                    index += 1;
                }
                current_name = line.trim_end_matches(':').to_string();
                current_vram = 0;
            }

            // VRAM line (e.g., "VRAM (Total): 16 GB")
            if line.starts_with("VRAM") {
                if let Some(vram_str) = line.split(':').nth(1) {
                    let vram_str = vram_str.trim();
                    if let Some(gb_pos) = vram_str.find(" GB") {
                        if let Ok(gb) = vram_str[..gb_pos].trim().parse::<u64>() {
                            current_vram = gb * 1_073_741_824; // Convert GB to bytes
                        }
                    } else if let Some(mb_pos) = vram_str.find(" MB") {
                        if let Ok(mb) = vram_str[..mb_pos].trim().parse::<u64>() {
                            current_vram = mb * 1_048_576; // Convert MB to bytes
                        }
                    }
                }
            }
        }

        // Don't forget the last GPU
        if !current_name.is_empty() {
            devices.push(Self::create_device(&current_name, current_vram, index));
        }

        if devices.is_empty() {
            Self::fallback_device()
        } else {
            devices
        }
    }

    #[cfg(target_os = "macos")]
    fn create_device(name: &str, vram_bytes: u64, index: usize) -> MetalDevice {
        let is_apple_silicon = name.contains("Apple") || cfg!(target_arch = "aarch64");
        let is_integrated = name.contains("Intel") || name.contains("Integrated");

        MetalDevice {
            name: name.to_string(),
            registry_id: (index + 1) as u64,
            is_low_power: is_integrated,
            is_headless: false,
            max_threads_per_threadgroup: 1024,
            max_buffer_length: if vram_bytes > 0 {
                vram_bytes
            } else if is_apple_silicon {
                17_179_869_184 // 16 GB default for Apple Silicon
            } else {
                4_294_967_296 // 4 GB default
            },
            has_unified_memory: is_apple_silicon,
            index,
        }
    }

    #[cfg(target_os = "macos")]
    /// Returns no devices when detection fails.
    ///
    /// Earlier versions fabricated a plausible `MetalDevice` here — named
    /// "Apple GPU", reporting 1024 threads per threadgroup and a 4 GB maximum
    /// buffer — whenever `system_profiler` was unavailable. That invented a
    /// GPU on machines that have no Metal at all, including non-macOS hosts.
    /// Reporting an empty device list is the honest answer to "detection did
    /// not work".
    fn fallback_device() -> Vec<MetalDevice> {
        Vec::new()
    }

    /// Check if any Metal device is available.
    #[must_use]
    pub fn is_available() -> bool {
        !Self::devices().is_empty()
    }

    /// Create a compute pipeline on the specified device.
    ///
    /// # Arguments
    ///
    /// * `device_index` - Index into the devices list from `devices()`
    ///
    /// # Errors
    ///
    /// Returns an error if the device index is out of bounds.
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

    /// Create a compute pipeline on the default (first) device.
    ///
    /// # Errors
    ///
    /// Returns an error if no Metal devices are available.
    pub fn default_device() -> Result<Self> {
        if Self::devices().is_empty() {
            return Err(Error::not_available(Subsystem::Metal));
        }
        Self::new(0)
    }

    /// Get the device name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get the device index.
    #[must_use]
    pub const fn device_index(&self) -> usize {
        self.device_index
    }

    /// Compile a Metal shader from source.
    ///
    /// # Arguments
    ///
    /// * `source` - Metal Shading Language (MSL) source code
    /// * `function_name` - Name of the kernel function to compile
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// Earlier versions returned a `CompiledShader` whose `source_hash` was a
    /// 64-bit string hash of the source. Nothing was compiled, so invalid MSL
    /// was accepted as readily as valid MSL.
    pub fn compile_shader(&self, source: &str, function_name: &str) -> Result<CompiledShader> {
        let _ = (source, function_name);
        Err(Error::unimplemented(
            Subsystem::Metal,
            "shader compilation (requires MTLDevice::newLibraryWithSource)",
        ))
    }

    /// Allocate a buffer on the GPU.
    ///
    /// # Arguments
    ///
    /// * `length` - Size in bytes
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// Earlier versions returned a `MetalBuffer` holding only a length and a
    /// device index. No GPU memory — and in fact no memory at all — was ever
    /// allocated, so writes had nowhere to go and reads had nothing to return.
    pub fn allocate_buffer(&self, length: usize) -> Result<MetalBuffer> {
        let _ = length;
        Err(Error::unimplemented(
            Subsystem::Metal,
            "buffer allocation (requires MTLDevice::newBufferWithLength)",
        ))
    }

    /// Dispatch a compute shader.
    ///
    /// # Arguments
    ///
    /// * `shader` - Compiled shader to execute
    /// * `buffers` - Buffers to bind to the shader
    /// * `grid_size` - Total number of threads (width, height, depth)
    /// * `threadgroup_size` - Threads per threadgroup (width, height, depth)
    ///
    /// # Errors
    ///
    /// Always returns [`Error::Unimplemented`] in this release.
    ///
    /// Earlier versions validated the grid and threadgroup arguments and then
    /// returned `Ok(())` having dispatched nothing. A caller would read its
    /// output buffer and find whatever was there before — silently wrong
    /// results rather than a reported failure.
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

/// Check if Metal is available.
///
/// Convenience function equivalent to `MetalCompute::is_available()`.
#[must_use]
pub fn is_available() -> bool {
    MetalCompute::is_available()
}

#[cfg(test)]
mod tests;
