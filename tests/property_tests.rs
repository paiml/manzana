//! Property-based tests for Manzana.
//!
//! Uses proptest to generate random inputs and verify invariants hold.
//! This implements Popperian falsification - tests attempt to disprove claims.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use manzana::afterburner::{AfterburnerStats, ProResCodec};
use manzana::error::{Error, Subsystem};
use manzana::neural_engine::Tensor;
use manzana::unified_memory::UmaBuffer;
use proptest::prelude::*;
use std::collections::HashMap;

// Strategy for generating ProResCodec values
fn prores_codec_strategy() -> impl Strategy<Value = ProResCodec> {
    prop_oneof![
        Just(ProResCodec::ProRes422),
        Just(ProResCodec::ProRes422HQ),
        Just(ProResCodec::ProRes422LT),
        Just(ProResCodec::ProRes422Proxy),
        Just(ProResCodec::ProRes4444),
        Just(ProResCodec::ProRes4444XQ),
        Just(ProResCodec::ProResRAW),
        Just(ProResCodec::ProResRAWHQ),
    ]
}

// Strategy for generating Subsystem values
fn subsystem_strategy() -> impl Strategy<Value = Subsystem> {
    prop_oneof![
        Just(Subsystem::Afterburner),
        Just(Subsystem::NeuralEngine),
        Just(Subsystem::Metal),
        Just(Subsystem::UnifiedMemory),
    ]
}

// Strategy for generating AfterburnerStats
fn afterburner_stats_strategy() -> impl Strategy<Value = AfterburnerStats> {
    (
        0u32..100,                            // streams_active
        1u32..50,                             // streams_capacity
        0.0f64..100.0,                        // utilization_percent
        0.0f64..1000.0,                       // throughput_fps
        proptest::option::of(20.0f64..120.0), // temperature
        proptest::option::of(0.0f64..100.0),  // power
    )
        .prop_map(
            |(
                streams_active,
                streams_capacity,
                utilization_percent,
                throughput_fps,
                temp,
                power,
            )| {
                AfterburnerStats {
                    streams_active,
                    streams_capacity,
                    utilization_percent,
                    throughput_fps,
                    temperature_celsius: temp,
                    power_watts: power,
                    codec_breakdown: HashMap::new(),
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    // Property: is_active() returns true iff streams_active > 0
    #[test]
    fn prop_is_active_iff_streams_positive(stats in afterburner_stats_strategy()) {
        prop_assert_eq!(stats.is_active(), stats.streams_active > 0);
    }

    // Property: capacity_used_percent is always in [0, infinity)
    // (can exceed 100% if streams_active > streams_capacity)
    #[test]
    fn prop_capacity_used_percent_non_negative(stats in afterburner_stats_strategy()) {
        prop_assert!(stats.capacity_used_percent() >= 0.0);
    }

    // Property: capacity_used_percent is 0 when capacity is 0
    #[test]
    fn prop_capacity_zero_means_zero_percent(
        streams_active in 0u32..100
    ) {
        let stats = AfterburnerStats {
            streams_active,
            streams_capacity: 0,
            ..Default::default()
        };
        prop_assert!((stats.capacity_used_percent() - 0.0).abs() < f64::EPSILON);
    }

    // Property: temperature safety check is consistent
    #[test]
    fn prop_temperature_safety_consistent(temp in 0.0f64..150.0) {
        let stats = AfterburnerStats {
            temperature_celsius: Some(temp),
            ..Default::default()
        };
        let is_safe = stats.is_temperature_safe();
        prop_assert!(is_safe.is_some());
        if let Some(safe) = is_safe {
            prop_assert_eq!(safe, temp < 100.0);
        }
    }

    // Property: temperature safety returns None when temp is None
    #[test]
    fn prop_temperature_none_means_none_safety(_x in 0..100) {
        let stats = AfterburnerStats::default();
        prop_assert!(stats.is_temperature_safe().is_none());
    }

    // Property: Error::is_not_available only true for NotAvailable variant
    #[test]
    fn prop_is_not_available_only_for_variant(subsystem in subsystem_strategy()) {
        let err = Error::not_available(subsystem);
        prop_assert!(err.is_not_available());

        let other_err = Error::timeout(100);
        prop_assert!(!other_err.is_not_available());
    }

    // Property: Error::is_timeout only true for Timeout variant
    #[test]
    fn prop_is_timeout_only_for_variant(duration in 0u64..10000) {
        let err = Error::timeout(duration);
        prop_assert!(err.is_timeout());

        let other_err = Error::not_available(Subsystem::Metal);
        prop_assert!(!other_err.is_timeout());
    }

    // Property: error_code returns Some for IoKit and Security, None otherwise
    #[test]
    fn prop_error_code_iokit(code in -1000i32..1000) {
        let err = Error::iokit(code, "test");
        prop_assert_eq!(err.error_code(), Some(code));
    }

    #[test]
    fn prop_error_code_security(code in -1000i32..1000) {
        let err = Error::security(code);
        prop_assert_eq!(err.error_code(), Some(code));
    }

    #[test]
    fn prop_error_code_none_for_others(subsystem in subsystem_strategy()) {
        let err = Error::not_available(subsystem);
        prop_assert!(err.error_code().is_none());
    }

    // Property: ProResCodec Display is not empty
    #[test]
    fn prop_prores_codec_display_not_empty(codec in prores_codec_strategy()) {
        let display = codec.to_string();
        prop_assert!(!display.is_empty());
        prop_assert!(display.contains("ProRes"));
    }

    // Property: Subsystem Display is not empty
    #[test]
    fn prop_subsystem_display_not_empty(subsystem in subsystem_strategy()) {
        let display = subsystem.to_string();
        prop_assert!(!display.is_empty());
    }

    // Property: Error Display is human-readable (> 10 chars)
    #[test]
    fn prop_error_display_readable(subsystem in subsystem_strategy()) {
        let err = Error::not_available(subsystem);
        let display = err.to_string();
        prop_assert!(display.len() > 10);
    }

    // Property: AfterburnerStats clone equals original
    #[test]
    fn prop_stats_clone_equals(stats in afterburner_stats_strategy()) {
        let cloned = stats.clone();
        prop_assert_eq!(stats.streams_active, cloned.streams_active);
        prop_assert_eq!(stats.streams_capacity, cloned.streams_capacity);
        prop_assert!((stats.utilization_percent - cloned.utilization_percent).abs() < f64::EPSILON);
    }

    // Property: Error clone equals original
    #[test]
    fn prop_error_clone_equals(code in -1000i32..1000) {
        let err = Error::iokit(code, "test message");
        let cloned = err.clone();
        prop_assert_eq!(err, cloned);
    }

    // Property: capacity_used_percent = (active/capacity) * 100 when capacity > 0
    #[test]
    fn prop_capacity_formula_correct(
        active in 0u32..50,
        capacity in 1u32..50
    ) {
        let stats = AfterburnerStats {
            streams_active: active,
            streams_capacity: capacity,
            ..Default::default()
        };
        let expected = (f64::from(active) / f64::from(capacity)) * 100.0;
        prop_assert!((stats.capacity_used_percent() - expected).abs() < 0.001);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // FALSIFY-TENSOR-003 (contract manzana-tensor-v1).
    //
    // Construction must be TOTAL: Ok or Err, never a panic. This crate sets
    // panic = "deny", and Tensor::new is safe and public, so a panicking
    // constructor is a denial of service reachable from safe code.
    //
    // This found a real defect: `shape.iter().product::<usize>()` panicked in
    // debug and wrapped in release for shapes like [usize::MAX, 2].
    #[test]
    fn prop_tensor_construction_is_total(
        shape in proptest::collection::vec(0usize..=usize::MAX, 0..4),
        n in 0usize..64,
    ) {
        let data = vec![0.0f32; n];
        let _ = Tensor::new(shape, data); // must not panic
        prop_assert!(true);
    }

    // The overflow case specifically, since random usize rarely hits it.
    #[test]
    fn prop_tensor_overflowing_shape_is_rejected_not_panicking(
        big in (usize::MAX / 2)..=usize::MAX,
        m in 2usize..8,
    ) {
        let r = Tensor::new(vec![big, m], vec![0.0; 4]);
        prop_assert!(r.is_err(), "an overflowing shape product must be rejected");
    }

    // Property: UMA buffer allocation preserves length
    #[test]
    fn prop_uma_buffer_length_preserved(len in 1usize..100_000) {
        // NOT `if let Ok(..)`: that made an implementation of UmaBuffer::new
        // returning Err pass all three properties across every generated case.
        // Allocation of a valid length must SUCCEED, so assert it.
        let buffer = UmaBuffer::new(len).expect("valid length must allocate");
        {
            prop_assert_eq!(buffer.len(), len);
            prop_assert!(!buffer.is_empty());
        }
    }

    // Property: UMA buffer is always page-aligned
    #[test]
    fn prop_uma_buffer_alignment(len in 1usize..100_000) {
        // NOT `if let Ok(..)`: that made an implementation of UmaBuffer::new
        // returning Err pass all three properties across every generated case.
        // Allocation of a valid length must SUCCEED, so assert it.
        let buffer = UmaBuffer::new(len).expect("valid length must allocate");
        {
            prop_assert!(buffer.is_aligned());
            prop_assert!(buffer.allocated_size() >= 4096);
        }
    }

    // Property: UMA allocated size >= requested size
    #[test]
    fn prop_uma_allocated_ge_requested(len in 1usize..100_000) {
        // NOT `if let Ok(..)`: that made an implementation of UmaBuffer::new
        // returning Err pass all three properties across every generated case.
        // Allocation of a valid length must SUCCEED, so assert it.
        let buffer = UmaBuffer::new(len).expect("valid length must allocate");
        {
            prop_assert!(buffer.allocated_size() >= len);
        }
    }

    // Property: Valid signature length (64-72 for P-256 DER)

    // Property: the DER parser must never panic, on ANY input.
    //
    // parse_der_ecdsa_sig does raw indexing and slicing, and this crate sets
    // panic/unwrap/expect = "deny". A parser reachable from a public
    // constructor that panics on hostile input is a denial-of-service bug, so
    // this throws arbitrary bytes at it and only requires that it returns.

    // Property: length-prefix fields are attacker-controlled, so drive them
    // directly rather than hoping random bytes hit the interesting paths.

    // Property: non-DER bytes of a plausible length are rejected.

    // Property: Invalid signature lengths rejected


}

#[cfg(test)]
mod determinism_tests {
    use super::*;

    // F099: Deterministic output for same input
    #[test]
    fn test_stats_methods_deterministic() {
        let stats = AfterburnerStats {
            streams_active: 10,
            streams_capacity: 23,
            utilization_percent: 45.0,
            throughput_fps: 120.0,
            temperature_celsius: Some(65.0),
            power_watts: Some(25.0),
            codec_breakdown: HashMap::new(),
        };

        // Run multiple times and verify same result
        for _ in 0..100 {
            assert!(stats.is_active());
            assert!((stats.capacity_used_percent() - 43.478).abs() < 0.01);
            assert_eq!(stats.is_temperature_safe(), Some(true));
        }
    }

    #[test]
    fn test_error_methods_deterministic() {
        let err = Error::iokit(42, "test");

        for _ in 0..100 {
            assert_eq!(err.error_code(), Some(42));
            assert!(!err.is_not_available());
            assert!(!err.is_timeout());
        }
    }

    // F099: Secure Enclave refuses consistently, for every input.
    //
    // The previous version of this test asserted that signing the same data
    // twice produced identical bytes, describing it in its own comment as a
    // "deterministic stub" -- it encoded the fake as the expected behaviour.

    // F099: UMA buffer operations deterministic
    #[test]
    fn test_uma_deterministic() {
        let buffer = UmaBuffer::new(4096).unwrap();

        for _ in 0..100 {
            assert_eq!(buffer.len(), 4096);
            assert!(buffer.is_aligned());
            assert!(buffer.allocated_size() >= 4096);
        }
    }
}
