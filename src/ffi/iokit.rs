//! IOKit bindings for Apple hardware discovery.
//!
//! # Safety
//!
//! This module contains unsafe FFI code. All unsafe blocks are documented
//! with SAFETY comments explaining why they are sound.
//!
//! # Thread Safety
//!
//! IOKit services are NOT thread-safe. The wrapper types implement `!Send`
//! and `!Sync` to prevent cross-thread usage.

use crate::error::Error;
use core_foundation::base::{kCFAllocatorDefault, CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use std::ffi::CStr;
use std::ptr;

// IOKit constants
const KERN_SUCCESS: i32 = 0;

// IOKit type aliases
type IoServiceT = u32;
type MachPortT = u32;

// Service class names for Afterburner
const AFTERBURNER_SERVICE_NAMES: &[&str] = &[
    "AppleProResAccelerator",
    "AppleAfterburner",
    "AFBAccelerator",
];

// External IOKit functions
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const i8) -> *mut core_foundation_sys::dictionary::CFDictionaryRef;
    fn IOServiceGetMatchingService(
        main_port: MachPortT,
        matching: *mut core_foundation_sys::dictionary::CFDictionaryRef,
    ) -> IoServiceT;
    fn IOObjectRelease(object: u32) -> i32;
    fn IORegistryEntryCreateCFProperties(
        entry: IoServiceT,
        properties: *mut core_foundation_sys::dictionary::CFDictionaryRef,
        allocator: core_foundation_sys::base::CFAllocatorRef,
        options: u32,
    ) -> i32;
    fn IORegistryEntryGetName(entry: IoServiceT, name: *mut i8) -> i32;
}

/// RAII wrapper for IOKit service.
///
/// Automatically releases the service on drop.
///
/// # Thread Safety
///
/// This type is `!Send` and `!Sync` because IOKit services are not thread-safe.
pub struct AfterburnerService {
    service: IoServiceT,
    // Prevent Send/Sync - IOKit services are not thread-safe
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl Drop for AfterburnerService {
    fn drop(&mut self) {
        if self.service != 0 {
            // SAFETY: service is a valid io_service_t obtained from IOServiceGetMatchingService.
            // IOObjectRelease is safe to call on any valid IOKit object.
            unsafe {
                IOObjectRelease(self.service);
            }
        }
    }
}

/// Raw statistics from the Afterburner FPGA.
///
/// These values are read directly from IOKit registry properties.
#[derive(Debug, Clone, Default)]
pub struct AfterburnerRawStats {
    /// Number of active decode streams.
    pub streams_active: u32,
    /// Maximum concurrent stream capacity.
    pub streams_capacity: u32,
    /// FPGA utilization percentage (0-100).
    pub utilization: f64,
    /// Total frames per second throughput.
    pub throughput_fps: f64,
    /// FPGA temperature in Celsius (if available).
    pub temperature: Option<f64>,
    /// Power consumption in watts (if available).
    pub power: Option<f64>,
}

/// Attempt to find the Afterburner IOKit service.
///
/// Tries multiple service class names in order of preference.
///
/// # Returns
///
/// - `Some(AfterburnerService)` if found
/// - `None` if not available (non-Mac Pro, card not installed)
pub fn find_afterburner_service() -> Option<AfterburnerService> {
    for service_name in AFTERBURNER_SERVICE_NAMES {
        if let Some(service) = find_service_by_name(service_name) {
            return Some(service);
        }
    }
    None
}

/// Find an IOKit service by class name.
fn find_service_by_name(name: &str) -> Option<AfterburnerService> {
    // SAFETY: IOServiceMatching takes a C string and returns a CFDictionary.
    // The dictionary is consumed by IOServiceGetMatchingService (no release needed).
    let service = unsafe {
        let name_cstr = std::ffi::CString::new(name).ok()?;
        let matching = IOServiceMatching(name_cstr.as_ptr());
        if matching.is_null() {
            return None;
        }
        // IOServiceGetMatchingService consumes the matching dictionary
        IOServiceGetMatchingService(0, matching)
    };

    if service == 0 {
        None
    } else {
        Some(AfterburnerService {
            service,
            _not_send_sync: std::marker::PhantomData,
        })
    }
}

impl AfterburnerService {
    /// Query current statistics from the Afterburner FPGA.
    ///
    /// # Errors
    ///
    /// Returns an error if IOKit registry access fails.
    pub fn get_stats(&self) -> Result<AfterburnerRawStats, Error> {
        let properties = self.get_properties()?;
        Ok(parse_afterburner_properties(&properties))
    }

    /// Get the IOKit registry properties for this service.
    fn get_properties(&self) -> Result<CFDictionary<CFString, CFType>, Error> {
        let mut properties_ref: core_foundation_sys::dictionary::CFDictionaryRef = ptr::null_mut();

        // SAFETY: IORegistryEntryCreateCFProperties reads registry properties into a CFDictionary.
        // We own the returned dictionary and must release it (handled by CFDictionary wrapper).
        let result = unsafe {
            IORegistryEntryCreateCFProperties(
                self.service,
                &mut properties_ref,
                kCFAllocatorDefault,
                0,
            )
        };

        if result != KERN_SUCCESS {
            return Err(Error::iokit(result, "failed to get registry properties"));
        }

        if properties_ref.is_null() {
            return Err(Error::iokit(0, "registry properties returned null"));
        }

        // SAFETY: properties_ref is a valid CFDictionary from IORegistryEntryCreateCFProperties.
        // CFDictionary::wrap_under_create_rule takes ownership and will release on drop.
        let properties: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_create_rule(properties_ref) };

        Ok(properties)
    }

    /// Get the service name for debugging.
    #[allow(dead_code)]
    pub fn name(&self) -> Option<String> {
        let mut name_buf = [0i8; 128];

        // SAFETY: IORegistryEntryGetName writes a null-terminated C string to the buffer.
        // Buffer is 128 bytes which is sufficient for IOKit service names.
        let result = unsafe { IORegistryEntryGetName(self.service, name_buf.as_mut_ptr()) };

        if result != KERN_SUCCESS {
            return None;
        }

        // SAFETY: IORegistryEntryGetName guarantees null-termination on success.
        let name_cstr = unsafe { CStr::from_ptr(name_buf.as_ptr()) };
        name_cstr.to_str().ok().map(String::from)
    }
}

/// Parse Afterburner properties from IOKit registry.
///
/// Returns default values for any properties not found.
fn parse_afterburner_properties(
    properties: &CFDictionary<CFString, CFType>,
) -> AfterburnerRawStats {
    // Property keys (discovered via ioreg -l)
    let streams_active = get_u32_property(properties, "StreamsActive").unwrap_or(0);
    let streams_capacity = get_u32_property(properties, "StreamsCapacity").unwrap_or(23);
    let utilization = get_f64_property(properties, "Utilization").unwrap_or(0.0);
    let throughput_fps = get_f64_property(properties, "ThroughputFPS").unwrap_or(0.0);
    let temperature = get_f64_property(properties, "Temperature");
    let power = get_f64_property(properties, "PowerWatts");

    AfterburnerRawStats {
        streams_active,
        streams_capacity,
        utilization,
        throughput_fps,
        temperature,
        power,
    }
}

/// Extract a u32 property from IOKit dictionary.
fn get_u32_property(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<u32> {
    let cf_key = CFString::new(key);
    dict.find(&cf_key).and_then(|value| {
        value
            .downcast::<core_foundation::number::CFNumber>()
            .and_then(|num| num.to_i32().and_then(|v| u32::try_from(v).ok()))
    })
}

/// Extract a f64 property from IOKit dictionary.
fn get_f64_property(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<f64> {
    let cf_key = CFString::new(key);
    dict.find(&cf_key).and_then(|value| {
        value
            .downcast::<core_foundation::number::CFNumber>()
            .and_then(|num| num.to_f64())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_names_not_empty() {
        assert!(!AFTERBURNER_SERVICE_NAMES.is_empty());
    }

    #[test]
    fn test_find_afterburner_graceful_on_missing() {
        // This should return None gracefully, not panic
        let result = find_afterburner_service();
        // We can't assert the result since it depends on hardware,
        // but we verify it doesn't panic
        drop(result);
    }

    #[test]
    fn test_raw_stats_default() {
        let stats = AfterburnerRawStats::default();
        assert_eq!(stats.streams_active, 0);
        assert_eq!(stats.streams_capacity, 0);
        assert!((stats.utilization - 0.0).abs() < f64::EPSILON);
        assert!((stats.throughput_fps - 0.0).abs() < f64::EPSILON);
        assert!(stats.temperature.is_none());
        assert!(stats.power.is_none());
    }

    #[test]
    fn test_raw_stats_clone() {
        let stats = AfterburnerRawStats {
            streams_active: 5,
            streams_capacity: 23,
            utilization: 45.5,
            throughput_fps: 120.0,
            temperature: Some(65.0),
            power: Some(25.0),
        };
        let cloned = stats.clone();
        assert_eq!(stats.streams_active, cloned.streams_active);
        assert_eq!(stats.streams_capacity, cloned.streams_capacity);
    }

    #[test]
    fn test_raw_stats_debug() {
        let stats = AfterburnerRawStats::default();
        let debug = format!("{stats:?}");
        assert!(debug.contains("AfterburnerRawStats"));
    }
}
