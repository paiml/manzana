//! Value types: the ProRes codecs Afterburner accelerates, and a statistics
//! snapshot read from the IOKit registry.

use std::collections::HashMap;
use std::fmt;

/// A ProRes codec variant.
///
/// This enum exists to key [`AfterburnerStats::codec_breakdown`]. Because
/// nothing in this crate populates that map, no manzana API ever hands you a
/// `ProResCodec`; the type is useful only if you build an `AfterburnerStats`
/// yourself. The variants name codecs, they do not report what a particular
/// card supports.
///
/// # Example
///
/// ```
/// use manzana::afterburner::ProResCodec;
///
/// assert_eq!(ProResCodec::ProRes4444XQ.to_string(), "ProRes 4444 XQ");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProResCodec {
    /// ProRes 422.
    ProRes422,
    /// ProRes 422 HQ.
    ProRes422HQ,
    /// ProRes 422 LT.
    ProRes422LT,
    /// ProRes 422 Proxy.
    ProRes422Proxy,
    /// ProRes 4444.
    ProRes4444,
    /// ProRes 4444 XQ.
    ProRes4444XQ,
    /// ProRes RAW.
    ProResRAW,
    /// ProRes RAW HQ.
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

/// A snapshot of Afterburner FPGA statistics.
///
/// Returned by [`AfterburnerMonitor::stats`], which builds it from a single
/// read of the card's IOKit registry entry. The value is a plain owned struct;
/// it does not update itself, and holding it does not hold the IOKit service
/// open. Call `stats()` again for a fresh reading.
///
/// # How each field is produced
///
/// Every field comes from one IOKit registry property, and **a property that
/// is absent, or is not a `CFNumber`, is silently replaced by the default
/// shown below**. There is no way to tell a defaulted field from a genuine
/// reading, so on an untested registry layout the whole struct reads as an
/// idle card. Values that are present are then range-checked, and a reading
/// outside the plausible range is discarded rather than reported.
///
/// | Field | Default when absent | Range check applied |
/// |-------|--------------------|---------------------|
/// | `streams_active` | `0` | none |
/// | `streams_capacity` | `23` (hardcoded, never read from the device in this case) | none |
/// | `utilization_percent` | `0.0` | clamped into `0.0..=100.0` |
/// | `throughput_fps` | `0.0` | negative values raised to `0.0` |
/// | `temperature_celsius` | `None` | outside `0.0..150.0` becomes `None` |
/// | `power_watts` | `None` | outside `0.0..500.0` becomes `None` |
/// | `codec_breakdown` | always empty | — |
///
/// # Example
///
/// ```
/// use manzana::afterburner::AfterburnerStats;
///
/// let stats = AfterburnerStats {
///     streams_active: 10,
///     streams_capacity: 23,
///     ..Default::default()
/// };
/// assert!(stats.is_active());
/// assert!((stats.capacity_used_percent() - 43.478).abs() < 0.01);
/// ```
#[derive(Debug, Clone)]
pub struct AfterburnerStats {
    /// Number of decode streams the card reported as active.
    ///
    /// `0` if the registry did not report a stream count.
    pub streams_active: u32,
    /// Maximum concurrent stream capacity the card reported.
    ///
    /// `23` if the registry did not report a capacity. That fallback is a
    /// constant in this crate, not a measurement, and it is indistinguishable
    /// here from a genuine reading of 23.
    pub streams_capacity: u32,
    /// FPGA utilization, clamped into `0.0..=100.0`.
    ///
    /// `0.0` if the registry did not report utilization.
    pub utilization_percent: f64,
    /// Decode throughput in frames per second, floored at `0.0`.
    ///
    /// `0.0` if the registry did not report throughput.
    pub throughput_fps: f64,
    /// FPGA temperature in Celsius.
    ///
    /// `None` if the registry did not report a temperature **or** reported one
    /// outside `0.0..150.0`. The two cases are not distinguished.
    pub temperature_celsius: Option<f64>,
    /// Power draw in watts.
    ///
    /// `None` if the registry did not report power **or** reported a value
    /// outside `0.0..500.0`. The two cases are not distinguished.
    pub power_watts: Option<f64>,
    /// Active streams broken down by codec.
    ///
    /// Always empty in a value produced by [`AfterburnerMonitor::stats`]:
    /// manzana does not read per-codec properties from the registry. The field
    /// is public so you can populate it yourself; do not read an empty map as
    /// "the card is decoding nothing".
    pub codec_breakdown: HashMap<ProResCodec, u32>,
}

impl Default for AfterburnerStats {
    /// An idle-card snapshot, used as a starting value by callers building
    /// their own `AfterburnerStats`. It is NOT what an unreadable card
    /// produces: reading a card whose registry lacks the required keys is now
    /// an error, not a default.
    fn default() -> Self {
        Self {
            streams_active: 0,
            streams_capacity: 23,
            utilization_percent: 0.0,
            throughput_fps: 0.0,
            temperature_celsius: None,
            power_watts: None,
            codec_breakdown: HashMap::new(),
        }
    }
}

impl AfterburnerStats {
    /// Returns `true` if at least one decode stream was active.
    ///
    /// This is `streams_active > 0` and nothing more. It reads a field of this
    /// snapshot; it does not re-query the card.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerStats;
    ///
    /// assert!(!AfterburnerStats::default().is_active());
    ///
    /// let busy = AfterburnerStats { streams_active: 3, ..Default::default() };
    /// assert!(busy.is_active());
    /// ```
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.streams_active > 0
    }

    /// Returns `streams_active` as a percentage of `streams_capacity`.
    ///
    /// Returns `0.0` when `streams_capacity` is `0`, rather than dividing by
    /// zero. The result is not clamped: if you construct a snapshot whose
    /// active count exceeds its capacity, you get a value above 100.
    ///
    /// This is stream occupancy, which is a different measurement from
    /// [`utilization_percent`](Self::utilization_percent) — that one is read
    /// from the card.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerStats;
    ///
    /// let stats = AfterburnerStats {
    ///     streams_active: 10,
    ///     streams_capacity: 23,
    ///     ..Default::default()
    /// };
    /// assert!((stats.capacity_used_percent() - 43.478).abs() < 0.01);
    ///
    /// let unknown_capacity = AfterburnerStats {
    ///     streams_active: 5,
    ///     streams_capacity: 0,
    ///     ..Default::default()
    /// };
    /// assert_eq!(unknown_capacity.capacity_used_percent(), 0.0);
    /// ```
    #[must_use]
    pub fn capacity_used_percent(&self) -> f64 {
        if self.streams_capacity == 0 {
            return 0.0;
        }
        (f64::from(self.streams_active) / f64::from(self.streams_capacity)) * 100.0
    }

    /// Returns whether the recorded temperature is below 100 °C.
    ///
    /// Returns `None` when [`temperature_celsius`](Self::temperature_celsius)
    /// is `None`, which includes the case where the card reported a
    /// temperature that the range filter rejected.
    ///
    /// The 100 °C threshold is a constant in this crate. It is not a limit
    /// published by, or read from, the device, so treat `Some(false)` as "hot
    /// by manzana's rule of thumb", not as a thermal fault reported by the
    /// hardware.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerStats;
    ///
    /// assert_eq!(AfterburnerStats::default().is_temperature_safe(), None);
    ///
    /// let warm = AfterburnerStats { temperature_celsius: Some(65.0), ..Default::default() };
    /// assert_eq!(warm.is_temperature_safe(), Some(true));
    ///
    /// let hot = AfterburnerStats { temperature_celsius: Some(101.0), ..Default::default() };
    /// assert_eq!(hot.is_temperature_safe(), Some(false));
    /// ```
    #[must_use]
    pub fn is_temperature_safe(&self) -> Option<bool> {
        self.temperature_celsius.map(|t| t < 100.0)
    }
}
