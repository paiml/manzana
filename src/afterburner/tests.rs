//! Tests for the `afterburner` module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;

// F017: Returns None on non-Mac Pro gracefully
#[test]
fn test_new_graceful_on_missing_hardware() {
    // Should not panic even if Afterburner is not present
    let result = AfterburnerMonitor::new();
    // We can't assert the result since it depends on hardware,
    // but we verify it doesn't panic
    let _ = result;
}

// F029: Zero streams when idle (simulated via default)
#[test]
fn test_default_stats_zero_streams() {
    let stats = AfterburnerStats::default();
    assert_eq!(stats.streams_active, 0);
    assert!(!stats.is_active());
}

/// `Default` must not reconstitute the fabricated capacity.
///
/// `AfterburnerMonitor::stats()` returns a `Result`, so
/// `monitor.stats().unwrap_or_default()` is an ordinary thing to write. While
/// `Default` set `streams_capacity: 23`, that line handed the caller a
/// plausible idle-Afterburner reading -- capacity 23, zero streams, zero
/// utilisation -- on a machine with no card in it. `23` is the figure Apple
/// markets for the card ("up to 23 streams of 4K ProRes"), which is exactly
/// what made it convincing.
///
/// The constant was removed from the IOKit path in 0.3.0 and came straight
/// back through this impl. RED against that impl: it asserted 0, got 23.
#[test]
fn test_default_does_not_reconstitute_the_marketed_capacity() {
    let stats = AfterburnerStats::default();
    assert_eq!(
        stats.streams_capacity, 0,
        "Default must not carry a figure describing a real card; 23 is Apple's \
         marketed capacity and reads as a genuine measurement"
    );
    assert!(stats.utilization_percent.abs() < f64::EPSILON);
    assert!(stats.throughput_fps.abs() < f64::EPSILON);
    assert!(stats.temperature_celsius.is_none());
    assert!(stats.power_watts.is_none());
    assert!(stats.codec_breakdown.is_empty());
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
        streams_active: Some(5),
        streams_capacity: Some(23),
        utilization: Some(150.0), // Invalid, should clamp
        throughput_fps: Some(100.0),
        temperature: Some(65.0),
        power: Some(25.0),
    };
    let stats = convert_raw_stats(&raw).expect("all keys present");
    assert!((stats.utilization_percent - 100.0).abs() < 0.01);
}

#[test]
fn test_convert_raw_stats_clamps_negative_utilization() {
    let raw = AfterburnerRawStats {
        streams_active: Some(0),
        streams_capacity: Some(23),
        utilization: Some(-10.0), // Invalid, should clamp
        throughput_fps: Some(0.0),
        temperature: None,
        power: None,
    };
    let stats = convert_raw_stats(&raw).expect("all keys present");
    assert!((stats.utilization_percent - 0.0).abs() < 0.01);
}

/// Raw stats with every REQUIRED key present, so a test about optional
/// fields is not accidentally testing the missing-key path.
fn raw_with_required() -> AfterburnerRawStats {
    AfterburnerRawStats {
        streams_active: Some(0),
        streams_capacity: Some(23),
        utilization: Some(0.0),
        throughput_fps: Some(0.0),
        temperature: None,
        power: None,
    }
}

#[test]
fn test_each_required_key_is_individually_required() {
    // Each of the four required properties has its own error path, and each
    // error names the key it could not read. Testing only the all-absent case
    // left three of the four `ok_or_else` branches unexercised -- coverage
    // reported 74%, which is how this was noticed.
    let full = raw_with_required();

    let cases: [(&str, AfterburnerRawStats); 4] = [
        (
            "StreamsActive",
            AfterburnerRawStats {
                streams_active: None,
                ..full.clone()
            },
        ),
        (
            "StreamsCapacity",
            AfterburnerRawStats {
                streams_capacity: None,
                ..full.clone()
            },
        ),
        (
            "Utilization",
            AfterburnerRawStats {
                utilization: None,
                ..full.clone()
            },
        ),
        (
            "ThroughputFPS",
            AfterburnerRawStats {
                throughput_fps: None,
                ..full
            },
        ),
    ];

    for (key, raw) in cases {
        let err =
            convert_raw_stats(&raw).expect_err("a missing required key must not yield a snapshot");
        assert!(
            err.to_string().contains(key),
            "error for a missing {key} should name it; got: {err}"
        );
    }

    // And with all four present it succeeds, so the test is not passing
    // merely because convert_raw_stats always fails.
    assert!(convert_raw_stats(&full).is_ok());
}

#[test]
fn test_convert_raw_stats_requires_reported_keys() {
    // The whole point of the change: an absent registry key is a failure
    // to read the card, not a reading of zero, and never the hardcoded 23.
    let err = convert_raw_stats(&AfterburnerRawStats::default())
        .expect_err("absent keys must not produce a fabricated snapshot");
    assert!(err.to_string().contains("cannot be read"), "got {err}");
}

#[test]
fn test_convert_raw_stats_filters_invalid_temperature() {
    let raw = AfterburnerRawStats {
        temperature: Some(-10.0), // Invalid
        ..raw_with_required()
    };
    let stats = convert_raw_stats(&raw).expect("all keys present");
    assert!(stats.temperature_celsius.is_none());

    let raw2 = AfterburnerRawStats {
        temperature: Some(200.0), // Invalid (too hot)
        ..raw_with_required()
    };
    let stats2 = convert_raw_stats(&raw2).expect("required keys present");
    assert!(stats2.temperature_celsius.is_none());
}

#[test]
fn test_convert_raw_stats_filters_invalid_power() {
    let raw = AfterburnerRawStats {
        power: Some(-5.0), // Invalid
        ..raw_with_required()
    };
    let stats = convert_raw_stats(&raw).expect("all keys present");
    assert!(stats.power_watts.is_none());

    let raw2 = AfterburnerRawStats {
        power: Some(600.0), // Invalid (too high)
        ..raw_with_required()
    };
    let stats2 = convert_raw_stats(&raw2).expect("required keys present");
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

// ---------------------------------------------------------------------------
// stats() and is_active() reached directly.
//
// They require a real Afterburner card through the public API, so on every
// host in this matrix they were uncovered -- including the error path added
// when the fabricated `unwrap_or(23)` default was removed.
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
#[test]
fn test_stats_reports_absence_off_macos() {
    let mon = AfterburnerMonitor::for_tests();
    let err = mon
        .stats()
        .expect_err("there is no IOKit here, so no snapshot exists");
    assert!(err.is_not_available(), "got {err:?}");
    assert!(
        !err.is_unimplemented(),
        "absent hardware is not an unimplemented operation"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn test_is_active_propagates_the_read_failure() {
    let mon = AfterburnerMonitor::for_tests();
    let err = mon
        .is_active()
        .expect_err("is_active must not answer when the card cannot be read");
    assert!(err.is_not_available(), "got {err:?}");
}
