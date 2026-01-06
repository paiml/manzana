//! Property-based tests for Manzana.
//!
//! Uses proptest to generate random inputs and verify invariants hold.
//! This implements Popperian falsification - tests attempt to disprove claims.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use manzana::afterburner::{AfterburnerStats, ProResCodec};
use manzana::error::{Error, Subsystem};
use manzana::secure_enclave::{AccessControl, Algorithm, KeyConfig, PublicKey, Signature};
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
        Just(Subsystem::SecureEnclave),
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

// Strategy for generating AccessControl values
fn access_control_strategy() -> impl Strategy<Value = AccessControl> {
    prop_oneof![
        Just(AccessControl::None),
        Just(AccessControl::DevicePasscode),
        Just(AccessControl::Biometric),
        Just(AccessControl::BiometricOrPasscode),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    // Property: KeyConfig builder produces valid config
    #[test]
    fn prop_key_config_tag_preserved(tag in "[a-z.]{1,50}") {
        let config = KeyConfig::new(tag.clone());
        prop_assert_eq!(config.tag, tag);
        prop_assert_eq!(config.algorithm, Algorithm::P256);
    }

    // Property: KeyConfig access control is correctly set
    #[test]
    fn prop_key_config_access_control(
        tag in "[a-z.]{1,30}",
        ac in access_control_strategy()
    ) {
        let config = KeyConfig::new(tag).with_access_control(ac);
        prop_assert_eq!(config.access_control, ac);
    }

    // Property: UMA buffer allocation preserves length
    #[test]
    fn prop_uma_buffer_length_preserved(len in 1usize..100_000) {
        if let Ok(buffer) = UmaBuffer::new(len) {
            prop_assert_eq!(buffer.len(), len);
            prop_assert!(!buffer.is_empty());
        }
    }

    // Property: UMA buffer is always page-aligned
    #[test]
    fn prop_uma_buffer_alignment(len in 1usize..100_000) {
        if let Ok(buffer) = UmaBuffer::new(len) {
            prop_assert!(buffer.is_aligned());
            prop_assert!(buffer.allocated_size() >= 4096);
        }
    }

    // Property: UMA allocated size >= requested size
    #[test]
    fn prop_uma_allocated_ge_requested(len in 1usize..100_000) {
        if let Ok(buffer) = UmaBuffer::new(len) {
            prop_assert!(buffer.allocated_size() >= len);
        }
    }

    // Property: Valid signature length (64-72 for P-256 DER)
    #[test]
    fn prop_signature_valid_length(len in 64usize..73) {
        let bytes = vec![0x30; len]; // DER SEQUENCE marker
        let sig = Signature::from_bytes(bytes);
        prop_assert!(sig.is_ok());
        if let Ok(s) = sig {
            prop_assert_eq!(s.len(), len);
        }
    }

    // Property: Invalid signature lengths rejected
    #[test]
    fn prop_signature_invalid_length_short(len in 1usize..64) {
        let bytes = vec![0x30; len];
        let sig = Signature::from_bytes(bytes);
        prop_assert!(sig.is_err());
    }

    #[test]
    fn prop_signature_invalid_length_long(len in 73usize..200) {
        let bytes = vec![0x30; len];
        let sig = Signature::from_bytes(bytes);
        prop_assert!(sig.is_err());
    }

    // Property: P-256 public key must be 65 bytes
    #[test]
    fn prop_public_key_wrong_length_rejected(len in 1usize..200) {
        if len != 65 {
            let mut bytes = vec![0x04; len];
            if !bytes.is_empty() {
                bytes[0] = 0x04;
            }
            let pk = PublicKey::from_bytes(bytes);
            prop_assert!(pk.is_err());
        }
    }

    // Property: AccessControl Display not empty
    #[test]
    fn prop_access_control_display_not_empty(ac in access_control_strategy()) {
        let display = ac.to_string();
        prop_assert!(!display.is_empty());
    }

    // Property: Algorithm Display contains P-256
    #[test]
    fn prop_algorithm_display(_x in 0..10) {
        let alg = Algorithm::P256;
        let display = alg.to_string();
        prop_assert!(display.contains("P-256"));
    }
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

    // F099: Secure Enclave determinism
    #[test]
    #[cfg(target_os = "macos")]
    fn test_secure_enclave_deterministic() {
        use manzana::secure_enclave::SecureEnclaveSigner;

        let config = KeyConfig::new("com.manzana.proptest.determinism");
        if let Ok(signer) = SecureEnclaveSigner::create(config) {
            let data = b"Test data for determinism";

            // Sign same data multiple times
            let sig1 = signer.sign(data).unwrap();
            let sig2 = signer.sign(data).unwrap();

            // Should produce same signature (deterministic stub)
            assert_eq!(sig1.as_bytes(), sig2.as_bytes());
        }
    }

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
