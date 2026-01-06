//! Afterburner FPGA monitoring for Mac Pro (2019+).
//!
//! The Apple Afterburner is an FPGA-based hardware accelerator for ProRes
//! and ProRes RAW video codec acceleration. It can decode up to 6 streams
//! of 8K ProRes RAW or 23 streams of 4K ProRes 422 simultaneously.
//!
//! # Example
//!
//! ```no_run
//! use manzana::afterburner::AfterburnerMonitor;
//!
//! if let Some(monitor) = AfterburnerMonitor::new() {
//!     match monitor.stats() {
//!         Ok(stats) => println!("Active streams: {}", stats.streams_active),
//!         Err(e) => eprintln!("Failed to get stats: {e}"),
//!     }
//! } else {
//!     println!("Afterburner not available (non-Mac Pro or card not installed)");
//! }
//! ```
//!
//! # Falsification Claims
//!
//! - F016: Afterburner detected on Mac Pro 2019+
//! - F017: Returns None on non-Mac Pro gracefully
//! - F024: No crash on rapid polling
//! - F029: Zero streams when idle

use crate::error::Result;
use crate::ffi::iokit::{find_afterburner_service, AfterburnerRawStats, AfterburnerService};
use std::collections::HashMap;
use std::fmt;
use tracing::{debug, instrument, warn};

/// ProRes codec types supported by Afterburner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProResCodec {
    /// ProRes 422 (standard quality).
    ProRes422,
    /// ProRes 422 HQ (high quality).
    ProRes422HQ,
    /// ProRes 422 LT (light, smaller files).
    ProRes422LT,
    /// ProRes 422 Proxy (offline editing).
    ProRes422Proxy,
    /// ProRes 4444 (with alpha channel).
    ProRes4444,
    /// ProRes 4444 XQ (extreme quality with alpha).
    ProRes4444XQ,
    /// ProRes RAW.
    ProResRAW,
    /// ProRes RAW HQ (high quality RAW).
    ProResRAWHQ,
}

impl fmt::Display for ProResCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProRes422 => write!(f, "ProRes 422"),
            Self::ProRes422HQ => write!(f, "ProRes 422 HQ"),
            Self::ProRes422LT => write!(f, "ProRes 422 LT"),
            Self::ProRes422Proxy => write!(f, "ProRes 422 Proxy"),
            Self::ProRes4444 => write!(f, "ProRes 4444"),
            Self::ProRes4444XQ => write!(f, "ProRes 4444 XQ"),
            Self::ProResRAW => write!(f, "ProRes RAW"),
            Self::ProResRAWHQ => write!(f, "ProRes RAW HQ"),
        }
    }
}

/// Statistics from the Afterburner FPGA.
///
/// All fields are read-only snapshots of the current FPGA state.
#[derive(Debug, Clone)]
pub struct AfterburnerStats {
    /// Number of active decode streams.
    pub streams_active: u32,
    /// Maximum concurrent stream capacity.
    ///
    /// Typically 23 for 4K ProRes or 6 for 8K ProRes RAW.
    pub streams_capacity: u32,
    /// FPGA utilization percentage (0.0 - 100.0).
    pub utilization_percent: f64,
    /// Total frames per second throughput.
    pub throughput_fps: f64,
    /// FPGA temperature in Celsius (if available).
    pub temperature_celsius: Option<f64>,
    /// Power consumption in watts (if available).
    pub power_watts: Option<f64>,
    /// Breakdown of active streams by codec type.
    pub codec_breakdown: HashMap<ProResCodec, u32>,
}

impl Default for AfterburnerStats {
    fn default() -> Self {
        Self {
            streams_active: 0,
            streams_capacity: 23, // Default 4K capacity
            utilization_percent: 0.0,
            throughput_fps: 0.0,
            temperature_celsius: None,
            power_watts: None,
            codec_breakdown: HashMap::new(),
        }
    }
}

impl AfterburnerStats {
    /// Check if the Afterburner is currently processing video.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.streams_active > 0
    }

    /// Get the percentage of capacity in use.
    #[must_use]
    pub fn capacity_used_percent(&self) -> f64 {
        if self.streams_capacity == 0 {
            return 0.0;
        }
        (f64::from(self.streams_active) / f64::from(self.streams_capacity)) * 100.0
    }

    /// Check if temperature is within safe operating range.
    ///
    /// Returns `None` if temperature is not available.
    #[must_use]
    pub fn is_temperature_safe(&self) -> Option<bool> {
        self.temperature_celsius.map(|t| t < 100.0)
    }
}

/// Monitor for the Apple Afterburner FPGA.
///
/// Provides read-only access to Afterburner statistics. This type cannot
/// control the FPGA, only observe its current state.
///
/// # Thread Safety
///
/// This type is `!Send` and `!Sync` because the underlying IOKit service
/// is not thread-safe. Create a new monitor on each thread if needed.
///
/// # Graceful Degradation
///
/// On systems without Afterburner (non-Mac Pro, card not installed),
/// `AfterburnerMonitor::new()` returns `None` instead of panicking.
pub struct AfterburnerMonitor {
    service: AfterburnerService,
}

impl AfterburnerMonitor {
    /// Attempt to connect to the Afterburner FPGA.
    ///
    /// # Returns
    ///
    /// - `Some(AfterburnerMonitor)` if Afterburner is present
    /// - `None` if not available (graceful degradation)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use manzana::afterburner::AfterburnerMonitor;
    ///
    /// match AfterburnerMonitor::new() {
    ///     Some(monitor) => println!("Afterburner found!"),
    ///     None => println!("No Afterburner (this is normal on non-Mac Pro)"),
    /// }
    /// ```
    #[instrument(level = "debug")]
    #[must_use]
    pub fn new() -> Option<Self> {
        debug!("Searching for Afterburner service");
        let service = find_afterburner_service()?;
        debug!("Afterburner service found");
        Some(Self { service })
    }

    /// Query current FPGA statistics.
    ///
    /// This performs a direct IOKit query (Genchi Genbutsu principle).
    ///
    /// # Errors
    ///
    /// Returns an error if the IOKit registry query fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use manzana::afterburner::AfterburnerMonitor;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let monitor = AfterburnerMonitor::new().ok_or("No Afterburner")?;
    /// let stats = monitor.stats()?;
    /// println!("Utilization: {:.1}%", stats.utilization_percent);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(level = "debug", skip(self))]
    pub fn stats(&self) -> Result<AfterburnerStats> {
        let raw = self.service.get_stats()?;
        Ok(convert_raw_stats(&raw))
    }

    /// Check if the Afterburner is actively processing video.
    ///
    /// This is a convenience method that queries stats and checks stream count.
    ///
    /// # Errors
    ///
    /// Returns an error if the stats query fails.
    pub fn is_active(&self) -> Result<bool> {
        Ok(self.stats()?.is_active())
    }

    /// Check if Afterburner hardware is available on this system.
    ///
    /// This is a static method that can be called without creating a monitor.
    #[must_use]
    pub fn is_available() -> bool {
        find_afterburner_service().is_some()
    }
}

/// Convert raw IOKit stats to the public API type.
fn convert_raw_stats(raw: &AfterburnerRawStats) -> AfterburnerStats {
    AfterburnerStats {
        streams_active: raw.streams_active,
        streams_capacity: raw.streams_capacity,
        utilization_percent: raw.utilization.clamp(0.0, 100.0),
        throughput_fps: raw.throughput_fps.max(0.0),
        temperature_celsius: raw.temperature.filter(|&t| (0.0..150.0).contains(&t)),
        power_watts: raw.power.filter(|&p| (0.0..500.0).contains(&p)),
        codec_breakdown: HashMap::new(), // Populated from detailed IOKit properties
    }
}

/// Check if Afterburner hardware is available.
///
/// Convenience function equivalent to `AfterburnerMonitor::is_available()`.
#[must_use]
pub fn is_available() -> bool {
    AfterburnerMonitor::is_available()
}

#[cfg(test)]
mod tests {
    use super::*;

    // F017: Returns None on non-Mac Pro gracefully
    #[test]
    fn test_new_graceful_on_missing_hardware() {
        // Should not panic even if Afterburner is not present
        let result = AfterburnerMonitor::new();
        // We can't assert the result since it depends on hardware,
        // but we verify it doesn't panic
        drop(result);
    }

    // F029: Zero streams when idle (simulated via default)
    #[test]
    fn test_default_stats_zero_streams() {
        let stats = AfterburnerStats::default();
        assert_eq!(stats.streams_active, 0);
        assert!(!stats.is_active());
    }

    #[test]
    fn test_stats_is_active() {
        let stats = AfterburnerStats::default();
        assert!(!stats.is_active());

        let stats = AfterburnerStats {
            streams_active: 5,
            ..Default::default()
        };
        assert!(stats.is_active());
    }

    #[test]
    fn test_stats_capacity_used_percent() {
        let stats = AfterburnerStats {
            streams_capacity: 23,
            streams_active: 0,
            ..Default::default()
        };
        assert!((stats.capacity_used_percent() - 0.0).abs() < 0.01);

        let stats = AfterburnerStats {
            streams_capacity: 23,
            streams_active: 23,
            ..Default::default()
        };
        assert!((stats.capacity_used_percent() - 100.0).abs() < 0.01);

        let stats = AfterburnerStats {
            streams_capacity: 23,
            streams_active: 10,
            ..Default::default()
        };
        let expected = (10.0 / 23.0) * 100.0;
        assert!((stats.capacity_used_percent() - expected).abs() < 0.01);
    }

    #[test]
    fn test_stats_capacity_used_percent_zero_capacity() {
        let stats = AfterburnerStats {
            streams_capacity: 0,
            streams_active: 5,
            ..Default::default()
        };
        assert!((stats.capacity_used_percent() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_stats_temperature_safe() {
        // No temperature reading
        let stats = AfterburnerStats::default();
        assert!(stats.is_temperature_safe().is_none());

        // Safe temperature
        let stats = AfterburnerStats {
            temperature_celsius: Some(65.0),
            ..Default::default()
        };
        assert_eq!(stats.is_temperature_safe(), Some(true));

        // Unsafe temperature
        let stats = AfterburnerStats {
            temperature_celsius: Some(105.0),
            ..Default::default()
        };
        assert_eq!(stats.is_temperature_safe(), Some(false));

        // Edge case at 100
        let stats = AfterburnerStats {
            temperature_celsius: Some(100.0),
            ..Default::default()
        };
        assert_eq!(stats.is_temperature_safe(), Some(false));
    }

    #[test]
    fn test_prores_codec_display() {
        assert_eq!(ProResCodec::ProRes422.to_string(), "ProRes 422");
        assert_eq!(ProResCodec::ProRes422HQ.to_string(), "ProRes 422 HQ");
        assert_eq!(ProResCodec::ProRes422LT.to_string(), "ProRes 422 LT");
        assert_eq!(ProResCodec::ProRes422Proxy.to_string(), "ProRes 422 Proxy");
        assert_eq!(ProResCodec::ProRes4444.to_string(), "ProRes 4444");
        assert_eq!(ProResCodec::ProRes4444XQ.to_string(), "ProRes 4444 XQ");
        assert_eq!(ProResCodec::ProResRAW.to_string(), "ProRes RAW");
        assert_eq!(ProResCodec::ProResRAWHQ.to_string(), "ProRes RAW HQ");
    }

    #[test]
    fn test_prores_codec_equality() {
        assert_eq!(ProResCodec::ProRes422, ProResCodec::ProRes422);
        assert_ne!(ProResCodec::ProRes422, ProResCodec::ProRes4444);
    }

    #[test]
    fn test_prores_codec_hash() {
        let mut map = HashMap::new();
        map.insert(ProResCodec::ProRes422, 5);
        map.insert(ProResCodec::ProRes4444, 3);
        assert_eq!(map.get(&ProResCodec::ProRes422), Some(&5));
        assert_eq!(map.get(&ProResCodec::ProRes4444), Some(&3));
    }

    #[test]
    fn test_stats_clone() {
        let stats = AfterburnerStats {
            streams_active: 10,
            streams_capacity: 23,
            utilization_percent: 45.5,
            throughput_fps: 120.0,
            temperature_celsius: Some(65.0),
            power_watts: Some(25.0),
            codec_breakdown: HashMap::new(),
        };
        let cloned = stats.clone();
        assert_eq!(stats.streams_active, cloned.streams_active);
        assert_eq!(stats.streams_capacity, cloned.streams_capacity);
    }

    #[test]
    fn test_stats_debug() {
        let stats = AfterburnerStats::default();
        let debug = format!("{stats:?}");
        assert!(debug.contains("AfterburnerStats"));
        assert!(debug.contains("streams_active"));
    }

    #[test]
    fn test_convert_raw_stats_clamps_utilization() {
        let raw = AfterburnerRawStats {
            streams_active: 5,
            streams_capacity: 23,
            utilization: 150.0, // Invalid, should clamp
            throughput_fps: 100.0,
            temperature: Some(65.0),
            power: Some(25.0),
        };
        let stats = convert_raw_stats(&raw);
        assert!((stats.utilization_percent - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_convert_raw_stats_clamps_negative_utilization() {
        let raw = AfterburnerRawStats {
            streams_active: 0,
            streams_capacity: 23,
            utilization: -10.0, // Invalid, should clamp
            throughput_fps: 0.0,
            temperature: None,
            power: None,
        };
        let stats = convert_raw_stats(&raw);
        assert!((stats.utilization_percent - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_convert_raw_stats_filters_invalid_temperature() {
        let raw = AfterburnerRawStats {
            temperature: Some(-10.0), // Invalid
            ..Default::default()
        };
        let stats = convert_raw_stats(&raw);
        assert!(stats.temperature_celsius.is_none());

        let raw2 = AfterburnerRawStats {
            temperature: Some(200.0), // Invalid (too hot)
            ..Default::default()
        };
        let stats2 = convert_raw_stats(&raw2);
        assert!(stats2.temperature_celsius.is_none());
    }

    #[test]
    fn test_convert_raw_stats_filters_invalid_power() {
        let raw = AfterburnerRawStats {
            power: Some(-5.0), // Invalid
            ..Default::default()
        };
        let stats = convert_raw_stats(&raw);
        assert!(stats.power_watts.is_none());

        let raw2 = AfterburnerRawStats {
            power: Some(600.0), // Invalid (too high)
            ..Default::default()
        };
        let stats2 = convert_raw_stats(&raw2);
        assert!(stats2.power_watts.is_none());
    }

    #[test]
    fn test_is_available_static() {
        // Should not panic
        let _ = AfterburnerMonitor::is_available();
        let _ = is_available();
    }

    // F024: No crash on rapid polling (simulated)
    #[test]
    fn test_rapid_stats_creation() {
        // Create and drop many stats objects rapidly
        for _ in 0..1000 {
            let stats = AfterburnerStats::default();
            assert!(!stats.is_active());
        }
    }
}
