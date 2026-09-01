//! Read-only monitoring of the Apple Afterburner FPGA, via IOKit.
//!
//! The Afterburner is an accelerator card for the Mac Pro (2019) that offloads
//! ProRes and ProRes RAW decode from the CPU. This module reports whether such
//! a card is present and reads a statistics snapshot from it.
//!
//! Both paths go through real IOKit calls in the crate's FFI layer:
//! `IOServiceGetMatchingService` for presence, `IORegistryEntryCreateCFProperties`
//! for statistics. Nothing here is simulated, and nothing here returns
//! [`Error::Unimplemented`] — this is the one
//! hardware subsystem in manzana whose advertised operations are implemented.
//!
//! The module is observe-only. There is no API to configure the card, submit
//! work to it, or decode a frame with it.
//!
//! # Availability
//!
//! [`is_available`] is `true` exactly when an IOKit service matching one of
//! three Afterburner class names can be found. It is a live registry lookup,
//! not a cached flag and not a build-target check, so it is `false` on every
//! non-macOS target (there is no IOKit) and on any Mac without the card.
//!
//! # What the statistics do and do not tell you
//!
//! [`AfterburnerStats`] is assembled from IOKit registry properties, and a
//! property that is not in the registry is an ERROR: [`AfterburnerMonitor::stats`]
//! returns `Err` rather than a snapshot with a stand-in value in it. A card
//! whose registry uses different property names than this crate looks up
//! therefore reports a failure to read, not an idle card. See
//! [`AfterburnerStats`] for the exact per-field behaviour.
//!
//! Until 0.3.0 the missing property was silently defaulted and such a card
//! "read as an idle card: zero streams, zero utilization, no temperature" --
//! indistinguishable from a genuine idle reading.
//!
//! [`AfterburnerStats::codec_breakdown`] is always empty. Nothing in this
//! crate populates it.
//!
//! # Verification status
//!
//! No machine with an Afterburner installed appears in this repository's test
//! evidence, so the code paths that parse a populated registry have not been
//! exercised against the hardware. What is covered by tests: the absence path
//! (off macOS, `is_available()` is `false` and `new()` is `None`), and the
//! clamping and filtering applied to raw values before they reach a caller.
//! The specification tracks the hardware cases as F016 and F029; neither is
//! discharged here.
//!
//! # Tracing
//!
//! [`AfterburnerMonitor::new`] and [`AfterburnerMonitor::stats`] are
//! instrumented with `tracing` at the `debug` level.
//!
//! # Example
//!
//! ```
//! use manzana::afterburner::AfterburnerMonitor;
//!
//! match AfterburnerMonitor::new() {
//!     Some(monitor) => {
//!         let stats = monitor.stats()?;
//!         println!(
//!             "{} of {} streams active, {:.1}% utilization",
//!             stats.streams_active, stats.streams_capacity, stats.utilization_percent
//!         );
//!     }
//!     // The usual outcome: no card, on anything but a Mac Pro (2019) with one
//!     // installed.
//!     None => println!("Afterburner not present"),
//! }
//! # Ok::<(), manzana::Error>(())
//! ```

use crate::error::{Error, Result};
use crate::ffi::iokit::{find_afterburner_service, AfterburnerRawStats, AfterburnerService};
use std::collections::HashMap;
use tracing::{debug, instrument, warn};

mod stats;

pub use stats::{AfterburnerStats, ProResCodec};

/// An open handle to the Afterburner FPGA's IOKit service.
///
/// Holding one means the card was found: `AfterburnerMonitor` can only be
/// constructed by [`new`](Self::new), which returns `None` when the lookup
/// fails. The handle owns an IOKit object and releases it on drop.
///
/// The monitor is read-only. It exposes the card's statistics and nothing
/// else; there is no method here that changes the card's state.
///
/// # Thread safety
///
/// On macOS this type is `!Send` and `!Sync`, because the underlying IOKit
/// service handle is. Construct a monitor on the thread that will use it.
///
/// (On other targets the underlying handle is an uninhabited-in-practice unit
/// struct that happens to be `Send + Sync`, so the monitor is too. This is not
/// a usable difference: [`new`](Self::new) never returns `Some` off macOS.)
///
/// # Cost
///
/// [`new`](Self::new) and [`is_available`](Self::is_available) each perform a
/// full IOKit registry search, trying up to three service class names. Neither
/// result is cached. [`stats`](Self::stats) copies the service's entire
/// property dictionary out of the registry on every call.
pub struct AfterburnerMonitor {
    service: AfterburnerService,
}

impl AfterburnerMonitor {
    /// Construct a monitor for tests only, bypassing service discovery.
    ///
    /// `new()` requires IOKit, which exists only on macOS. Note this is NOT
    /// the same as requiring the card: the project's Linux host is a
    /// MacPro7,1 with an Afterburner fitted (`lspci` shows Apple 106b:0205 at
    /// 0f:00.0), and manzana still cannot read it there. Presence and
    /// reachability are different, and conflating them is how a doc comment
    /// states a false reason. Without this, `stats()` and `is_active()` were unreachable from
    /// any test -- including the error path added when the fabricated
    /// `unwrap_or(23)` default was removed. An unreachable method is not a
    /// tested one.
    #[cfg(all(test, not(target_os = "macos")))]
    pub(crate) const fn for_tests() -> Self {
        Self {
            service: crate::ffi::iokit::AfterburnerService,
        }
    }

    /// Looks up the Afterburner IOKit service and takes a handle to it.
    ///
    /// Returns `None` when no matching service is found — which is the normal
    /// result on any machine without the card, and the only result off macOS.
    /// `None` means "not found"; it does not distinguish a machine that cannot
    /// have the card from one whose card failed to enumerate.
    ///
    /// This is a fresh registry search on every call; nothing is cached.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerMonitor;
    ///
    /// match AfterburnerMonitor::new() {
    ///     Some(_monitor) => println!("Afterburner found"),
    ///     None => println!("no Afterburner on this machine"),
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

    /// Reads a fresh statistics snapshot from the card's IOKit registry entry.
    ///
    /// Each call copies the service's property dictionary out of the registry
    /// and converts it. An absent property is an error, not a default;
    /// out-of-range readings on the optional fields are discarded — see
    /// [`AfterburnerStats`] for the per-field table.
    ///
    /// # Errors
    ///
    /// - [`Error::IoKit`] if
    ///   `IORegistryEntryCreateCFProperties` returns anything other than
    ///   `KERN_SUCCESS`. [`error_code`](crate::Error::error_code) carries the
    ///   `kern_return_t`.
    /// - [`Error::IoKit`] with code `0` if that call
    ///   succeeds but hands back a null dictionary.
    ///
    /// - [`Error::IoKit`] with code `0` if the dictionary is readable but does
    ///   not carry one of `StreamsActive`, `StreamsCapacity`, `Utilization` or
    ///   `ThroughputFPS`. The message names the missing key.
    ///
    /// That last case is new in 0.3.0, and this section previously said the
    /// opposite of it in as many words: "a registry that contains none of the
    /// properties manzana looks for is not an error: it yields a defaulted
    /// snapshot." It now errors, and a `# Errors` section that denies the
    /// error its function returns is worse than none.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerMonitor;
    ///
    /// if let Some(monitor) = AfterburnerMonitor::new() {
    ///     let stats = monitor.stats()?;
    ///     println!("{:.1}% utilized", stats.utilization_percent);
    ///     if let Some(celsius) = stats.temperature_celsius {
    ///         println!("FPGA at {celsius:.1} C");
    ///     }
    /// }
    /// # Ok::<(), manzana::Error>(())
    /// ```
    #[instrument(level = "debug", skip(self))]
    pub fn stats(&self) -> Result<AfterburnerStats> {
        let raw = self.service.get_stats()?;
        convert_raw_stats(&raw)
    }

    /// Reads a snapshot and reports whether any decode stream was active.
    ///
    /// Equivalent to `self.stats()?.is_active()`, and it costs a full registry
    /// read. If you want other fields as well, call [`stats`](Self::stats)
    /// once and inspect the snapshot.
    ///
    /// `Ok(false)` now means the card reported zero active streams. It no
    /// longer means "the registry key was missing and the field defaulted to
    /// zero": since 0.3.0 a missing `StreamsActive` makes [`stats`](Self::stats)
    /// return `Err`, so there is no defaulted snapshot for this to read.
    ///
    /// # Errors
    ///
    /// The same [`Error::IoKit`] cases as
    /// [`stats`](Self::stats).
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerMonitor;
    ///
    /// if let Some(monitor) = AfterburnerMonitor::new() {
    ///     println!("decoding: {}", monitor.is_active()?);
    /// }
    /// # Ok::<(), manzana::Error>(())
    /// ```
    pub fn is_active(&self) -> Result<bool> {
        Ok(self.stats()?.is_active())
    }

    /// Returns `true` if the Afterburner IOKit service can be found.
    ///
    /// An associated function, so it needs no monitor. It performs the same
    /// registry search as [`new`](Self::new) and immediately releases the
    /// handle, so it is neither free nor cached, and it is a time-of-check
    /// value: a later `new()` searches again and may disagree.
    ///
    /// If you intend to use the card, call [`new`](Self::new) and match on the
    /// `Option` instead of checking first.
    ///
    /// # Example
    ///
    /// ```
    /// use manzana::afterburner::AfterburnerMonitor;
    ///
    /// // False on every non-macOS target, and on any Mac without the card.
    /// let present = AfterburnerMonitor::is_available();
    /// println!("Afterburner present: {present}");
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_available() -> bool {
        find_afterburner_service().is_some()
    }
}

/// Converts a raw IOKit reading into the public snapshot type.
///
/// This is where range checking happens, and it is deliberately lossy: an
/// implausible reading is replaced (utilization, throughput) or dropped
/// (temperature, power) rather than passed to a caller who has no way to tell
/// it apart from a sound one. `codec_breakdown` is left empty because nothing
/// reads per-codec properties from the registry.
fn convert_raw_stats(raw: &AfterburnerRawStats) -> Result<AfterburnerStats> {
    // A property IOKit did not report is a FAILURE TO READ, not a reading of
    // zero. `streams_capacity` previously defaulted to 23 -- the figure Apple
    // markets for Afterburner ("23x 4K streams") -- so a missing registry key
    // produced a plausible marketing number presented as a hardware
    // measurement, in the crate's most genuinely-implemented path.
    //
    // `stats()` already returns Result, so the honest answer was available all
    // along: say the card could not be read.
    let missing = |key: &str| {
        Error::iokit(
            0,
            format!("Afterburner registry entry carries no {key}; stats cannot be read"),
        )
    };

    Ok(AfterburnerStats {
        streams_active: raw.streams_active.ok_or_else(|| missing("StreamsActive"))?,
        streams_capacity: raw
            .streams_capacity
            .ok_or_else(|| missing("StreamsCapacity"))?,
        utilization_percent: raw
            .utilization
            .ok_or_else(|| missing("Utilization"))?
            .clamp(0.0, 100.0),
        throughput_fps: raw
            .throughput_fps
            .ok_or_else(|| missing("ThroughputFPS"))?
            .max(0.0),
        // These two are genuinely optional on the hardware, not merely absent
        // from this parse: Option here means "the card does not report it".
        temperature_celsius: raw.temperature.filter(|&t| (0.0..150.0).contains(&t)),
        power_watts: raw.power.filter(|&p| (0.0..500.0).contains(&p)),
        codec_breakdown: HashMap::new(),
    })
}

/// Returns `true` if the Afterburner IOKit service can be found.
///
/// A free-function alias for [`AfterburnerMonitor::is_available`], with the
/// same cost and the same time-of-check caveat.
///
/// # Example
///
/// ```
/// // False on every non-macOS target, and on any Mac without the card.
/// println!("Afterburner present: {}", manzana::afterburner::is_available());
/// ```
#[must_use]
#[allow(clippy::missing_const_for_fn)]
pub fn is_available() -> bool {
    AfterburnerMonitor::is_available()
}

#[cfg(test)]
mod tests;
