//! IOKit bindings for Afterburner discovery and statistics (macOS only).
//!
//! This is the whole of manzana's foreign-function surface. It binds five
//! IOKit entry points and uses them for exactly two jobs: find the
//! Afterburner's service in the IO registry, and copy that service's property
//! dictionary. No other Apple framework is bound anywhere in the crate.
//!
//! Everything here is private to the crate. [`crate::afterburner`] is the
//! public face of it.
//!
//! # Safety
//!
//! Every `unsafe` block in this file carries a `// SAFETY:` comment naming the
//! precondition it relies on; `clippy::undocumented_unsafe_blocks = "deny"`
//! keeps that true. The two ownership facts the whole file rests on:
//!
//! - `IOServiceGetMatchingService` **consumes** the matching dictionary it is
//!   given, so that dictionary must not be released by this code.
//! - `IORegistryEntryCreateCFProperties` follows Core Foundation's Create
//!   Rule, so the dictionary it produces **must** be released exactly once —
//!   which `CFDictionary::wrap_under_create_rule` arranges via `Drop`.
//!
//! No raw pointer leaves this module, and nothing here calls `transmute`.
//!
//! # Thread safety
//!
//! IOKit service handles are not thread-safe. [`AfterburnerService`] holds a
//! `PhantomData<*const ()>` so that it is `!Send` and `!Sync`, which stops a
//! handle being moved to or shared with another thread at compile time.

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

/// IOKit class names tried, in order, when looking for the card.
///
/// These names are unverified in this repository: no machine with an
/// Afterburner installed appears in the test evidence. If none of them matches
/// the real class, `find_afterburner_service` returns `None` and the card
/// reads as absent. That failure is silent, but it is a false negative, not a
/// fabricated success.
const AFTERBURNER_SERVICE_NAMES: &[&str] = &[
    "AppleProResAccelerator",
    "AppleAfterburner",
    "AFBAccelerator",
];

// IOKit entry points.
//
// A note on the two dictionary signatures below. In the framework headers
// `IOServiceMatching` returns `CFMutableDictionaryRef` and
// `IOServiceGetMatchingService` takes `CFDictionaryRef`; both of those C types
// are already a single pointer. These declarations spell them
// `*mut CFDictionaryRef`, one level of indirection deeper. The calls are still
// correct as written — pointers are all one word wide, and the value
// `IOServiceMatching` returns is only null-checked and handed straight back to
// `IOServiceGetMatchingService`, never dereferenced through the extra level —
// but the declared types do not match the headers, and any future code that
// dereferences the value would be unsound. `IORegistryEntryCreateCFProperties`
// is unaffected: its `properties` parameter genuinely is an out-pointer to a
// `CFDictionaryRef`.
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

/// An owned, retained IOKit service handle for the Afterburner.
///
/// Construct it only through [`find_afterburner_service`], which guarantees
/// the invariant every `unsafe` block here depends on: `service` is a non-zero
/// `io_service_t` returned by `IOServiceGetMatchingService`, retained on our
/// behalf and not yet released. `Drop` releases it exactly once.
///
/// # Thread safety
///
/// `!Send` and `!Sync`, enforced by the `PhantomData<*const ()>` field, because
/// IOKit service handles are not thread-safe.
pub struct AfterburnerService {
    service: IoServiceT,
    // Prevent Send/Sync - IOKit services are not thread-safe
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl Drop for AfterburnerService {
    fn drop(&mut self) {
        if self.service != 0 {
            // SAFETY: `self.service` is a non-zero `io_service_t` produced by
            // `IOServiceGetMatchingService` in `find_service_by_name`, which
            // hands back a retained object. This is the only `IOObjectRelease`
            // call on it anywhere in the crate, and `Drop` runs at most once,
            // so the retain is balanced exactly. `IOObjectRelease` accepts any
            // valid IOKit object.
            unsafe {
                IOObjectRelease(self.service);
            }
        }
    }
}

/// Statistics as read from the IO registry, before range checking.
///
/// A field here is whatever the registry held, or the fallback that
/// [`parse_afterburner_properties`] substituted when the property was missing —
/// the two are not distinguished at this layer. `crate::afterburner` applies
/// the range checks and documents the result for callers.
#[derive(Debug, Clone, Default)]
pub struct AfterburnerRawStats {
    /// Active decode streams, or `0` if the property was absent.
    pub streams_active: Option<u32>,
    /// Maximum concurrent stream capacity, or `23` if the property was absent.
    pub streams_capacity: Option<u32>,
    /// FPGA utilization as reported, unclamped; `0.0` if absent.
    pub utilization: Option<f64>,
    /// Decode throughput in frames per second as reported, unclamped; `0.0` if
    /// absent.
    pub throughput_fps: Option<f64>,
    /// FPGA temperature in Celsius as reported, unfiltered.
    pub temperature: Option<f64>,
    /// Power draw in watts as reported, unfiltered.
    pub power: Option<f64>,
}

/// Searches the IO registry for the Afterburner service.
///
/// Tries each name in [`AFTERBURNER_SERVICE_NAMES`] in order and returns a
/// handle to the first match. Returns `None` if no name matches — which is the
/// result on any Mac without the card, and is indistinguishable from the case
/// where the card is present under a class name this crate does not know.
///
/// Every call performs a fresh registry search; nothing is cached.
pub fn find_afterburner_service() -> Option<AfterburnerService> {
    for service_name in AFTERBURNER_SERVICE_NAMES {
        if let Some(service) = find_service_by_name(service_name) {
            return Some(service);
        }
    }
    None
}

/// Looks up a single IOKit service by class name.
///
/// Returns `None` if `name` contains an interior NUL, if `IOServiceMatching`
/// cannot build a matching dictionary, or if no service matches.
fn find_service_by_name(name: &str) -> Option<AfterburnerService> {
    // SAFETY:
    // - `name_cstr` lives for the whole call, so `as_ptr()` is a valid pointer
    //   to a NUL-terminated string for as long as `IOServiceMatching` reads it.
    //   `CString::new` rejects an interior NUL, so the string is well-formed.
    //   IOKit does not retain the pointer past the call.
    // - `IOServiceMatching` returns NULL on failure; the null check below runs
    //   before the value is used for anything else.
    // - `IOServiceGetMatchingService` consumes one reference to the matching
    //   dictionary, so it must not be released here. This is the only path that
    //   reaches it, and it is reached exactly once per successful
    //   `IOServiceMatching`, so nothing leaks and nothing is over-released.
    // - A main port of 0 is `kIOMainPortDefault`, which selects the default
    //   port. No port is created, so none needs releasing.
    // - The returned `io_service_t` is retained on our behalf. Ownership moves
    //   into the `AfterburnerService` below, whose `Drop` releases it once.
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
    /// Reads the service's registry properties and parses the statistics out
    /// of them.
    ///
    /// # Errors
    ///
    /// [`Error::IoKit`] if `IORegistryEntryCreateCFProperties` fails, or if it
    /// succeeds and returns a null dictionary. Nothing else fails: a registry
    /// missing every property manzana looks for parses into
    /// [`AfterburnerRawStats`] fallbacks rather than an error.
    pub fn get_stats(&self) -> Result<AfterburnerRawStats, Error> {
        let properties = self.get_properties()?;
        Ok(parse_afterburner_properties(&properties))
    }

    /// Copies this service's entire property dictionary out of the IO
    /// registry.
    ///
    /// # Errors
    ///
    /// [`Error::IoKit`] carrying the `kern_return_t` when the call fails, or
    /// carrying code `0` when it reports success but produces a null
    /// dictionary.
    fn get_properties(&self) -> Result<CFDictionary<CFString, CFType>, Error> {
        let mut properties_ref: core_foundation_sys::dictionary::CFDictionaryRef = ptr::null_mut();

        // SAFETY: `self.service` upholds the type invariant (a live, retained
        // `io_service_t`). `properties_ref` is a live, uniquely borrowed slot
        // for the whole call, which is what the out-parameter requires, and
        // `kCFAllocatorDefault` is a valid allocator. The dictionary is
        // produced under the Create Rule, so the +1 reference it hands back
        // must be released once; that is done by the wrapper below. The slot is
        // not read unless the call returned KERN_SUCCESS.
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

        // SAFETY: `properties_ref` was written by a KERN_SUCCESS return from
        // `IORegistryEntryCreateCFProperties` and null-checked above, so it is
        // a valid CFDictionary that we own. `wrap_under_create_rule` takes that
        // +1 reference without adding another and releases it on drop,
        // balancing the Create Rule exactly once. The key and value types are
        // only a compile-time view of an untyped CF dictionary; every read
        // below goes through `downcast`, so a registry whose contents differ
        // yields `None` rather than a bad cast.
        let properties: CFDictionary<CFString, CFType> =
            unsafe { CFDictionary::wrap_under_create_rule(properties_ref) };

        Ok(properties)
    }

    /// Reads the service's IO registry entry name, for debugging.
    ///
    /// Returns `None` if `IORegistryEntryGetName` fails or if the name is not
    /// valid UTF-8. Currently unused — kept for diagnostics, hence the
    /// `dead_code` allowance — and not reachable from manzana's public API.
    #[allow(dead_code)]
    pub fn name(&self) -> Option<String> {
        let mut name_buf = [0i8; 128];

        // SAFETY: `self.service` upholds the type invariant. The second
        // parameter is declared `io_name_t` in the IOKit headers, which is
        // `char[128]`; `name_buf` is exactly 128 bytes, so the callee cannot
        // write out of bounds. The buffer is uniquely borrowed for the call.
        let result = unsafe { IORegistryEntryGetName(self.service, name_buf.as_mut_ptr()) };

        if result != KERN_SUCCESS {
            return None;
        }

        // SAFETY: on KERN_SUCCESS the callee has written a NUL-terminated
        // string into the 128-byte buffer, so a NUL is guaranteed to be found
        // in bounds. `name_buf` outlives `name_cstr`, which borrows from it.
        let name_cstr = unsafe { CStr::from_ptr(name_buf.as_ptr()) };
        name_cstr.to_str().ok().map(String::from)
    }
}

/// Reads the Afterburner statistics out of a registry property dictionary.
///
/// A property that is absent, or that is not a `CFNumber`, is replaced by the
/// fallback shown below rather than reported as missing. That means a registry
/// whose key names differ from the ones looked up here parses into a
/// plausible idle card. The substitution is deliberate — a card that reports
/// nothing is not an error — but it is why
/// [`crate::afterburner::AfterburnerStats`] documents each field's fallback:
/// callers cannot otherwise tell a default from a reading.
///
/// The key names below, like the service class names, are unverified in this
/// repository. No Afterburner hardware appears in the test evidence.
fn parse_afterburner_properties(
    properties: &CFDictionary<CFString, CFType>,
) -> AfterburnerRawStats {
    let streams_active = get_u32_property(properties, "StreamsActive");
    let streams_capacity = get_u32_property(properties, "StreamsCapacity");
    let utilization = get_f64_property(properties, "Utilization");
    let throughput_fps = get_f64_property(properties, "ThroughputFPS");
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

/// Reads one property as a `u32`.
///
/// Returns `None` if the key is absent, the value is not a `CFNumber`, the
/// number does not fit an `i32`, or the `i32` is negative. All four cases are
/// reported the same way.
fn get_u32_property(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<u32> {
    let cf_key = CFString::new(key);
    dict.find(&cf_key).and_then(|value| {
        value
            .downcast::<core_foundation::number::CFNumber>()
            .and_then(|num| num.to_i32().and_then(|v| u32::try_from(v).ok()))
    })
}

/// Reads one property as an `f64`.
///
/// Returns `None` if the key is absent, the value is not a `CFNumber`, or the
/// number cannot be represented as an `f64`. No range checking happens here;
/// `crate::afterburner::convert_raw_stats` applies that.
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
    fn test_raw_stats_default_reports_nothing_measured() {
        // Default now means "IOKit reported nothing", not "the card is idle".
        // Every field is None so an absent reading stays distinguishable from
        // a measured zero -- the whole point of removing the unwrap_or(23).
        let stats = AfterburnerRawStats::default();
        assert!(stats.streams_active.is_none());
        assert!(stats.streams_capacity.is_none());
        assert!(stats.utilization.is_none());
        assert!(stats.throughput_fps.is_none());
        assert!(stats.temperature.is_none());
        assert!(stats.power.is_none());
    }

    #[test]
    fn test_raw_stats_clone() {
        let stats = AfterburnerRawStats {
            streams_active: Some(5),
            streams_capacity: Some(23),
            utilization: Some(45.5),
            throughput_fps: Some(120.0),
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
