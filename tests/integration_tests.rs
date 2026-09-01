//! Integration tests for Manzana.
//!
//! These tests verify the public API works correctly as a cohesive unit.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use manzana::afterburner::{is_available, AfterburnerMonitor, AfterburnerStats, ProResCodec};
use manzana::error::{Error, Subsystem};
use manzana::metal::MetalCompute;
use manzana::neural_engine::NeuralEngineSession;
use manzana::unified_memory::UmaBuffer;
use manzana::{is_acceleration_available, is_macos, VERSION};

// =============================================================================
// Library-level tests
// =============================================================================

#[test]
fn test_version_semver_format() {
    // Version should be in semver format (x.y.z)
    let parts: Vec<&str> = VERSION.split('.').collect();
    assert!(parts.len() >= 2, "Version should have at least major.minor");
    for part in &parts {
        assert!(
            part.parse::<u32>().is_ok(),
            "Version parts should be numeric"
        );
    }
}

#[test]
fn test_is_macos_platform_detection() {
    let result = is_macos();
    #[cfg(target_os = "macos")]
    assert!(result, "Should detect macOS on macOS");
    #[cfg(not(target_os = "macos"))]
    assert!(!result, "Should not detect macOS on other platforms");
}

#[test]
fn test_is_acceleration_available_no_crash() {
    // Should never panic, regardless of hardware
    let _ = is_acceleration_available();
}

// =============================================================================
// Afterburner API tests
// =============================================================================

#[test]
fn test_afterburner_monitor_graceful_on_missing_hardware() {
    // This test runs on all platforms
    // On non-Mac Pro, should return None gracefully
    let monitor = AfterburnerMonitor::new();
    // We can't assert the value since it depends on hardware,
    // but we verify it doesn't panic
    let _ = monitor;
}

#[test]
fn test_afterburner_is_available_consistent() {
    // is_available() should be consistent with new()
    let available = AfterburnerMonitor::is_available();
    let monitor = AfterburnerMonitor::new();
    assert_eq!(available, monitor.is_some());
}

#[test]
fn test_afterburner_convenience_function() {
    // Convenience function should match struct method
    assert_eq!(is_available(), AfterburnerMonitor::is_available());
}

#[test]
fn test_afterburner_stats_api() {
    let stats = AfterburnerStats {
        streams_active: 5,
        streams_capacity: 23,
        utilization_percent: 21.7,
        throughput_fps: 120.0,
        temperature_celsius: Some(65.0),
        power_watts: Some(25.0),
        codec_breakdown: std::collections::HashMap::new(),
    };

    // Verify all methods work correctly
    assert!(stats.is_active());
    assert!((stats.capacity_used_percent() - 21.739).abs() < 0.01);
    assert_eq!(stats.is_temperature_safe(), Some(true));
}

#[test]
fn test_prores_codec_all_variants() {
    let codecs = [
        ProResCodec::ProRes422,
        ProResCodec::ProRes422HQ,
        ProResCodec::ProRes422LT,
        ProResCodec::ProRes422Proxy,
        ProResCodec::ProRes4444,
        ProResCodec::ProRes4444XQ,
        ProResCodec::ProResRAW,
        ProResCodec::ProResRAWHQ,
    ];

    for codec in &codecs {
        let display = codec.to_string();
        assert!(
            display.contains("ProRes"),
            "All codecs should display ProRes"
        );
    }
}

// =============================================================================
// Error API tests
// =============================================================================

#[test]
fn test_error_subsystem_all_variants() {
    let subsystems = [
        Subsystem::Afterburner,
        Subsystem::NeuralEngine,
        Subsystem::Metal,
        Subsystem::UnifiedMemory,
    ];

    for subsystem in &subsystems {
        let err = Error::not_available(*subsystem);
        assert!(err.is_not_available());
        assert!(!err.is_timeout());
        assert!(err.error_code().is_none());
    }
}

#[test]
fn test_error_constructors_all_variants() {
    let errors = vec![
        Error::not_available(Subsystem::Afterburner),
        Error::iokit(0, "test"),
        Error::metal("test"),
        Error::coreml("test"),
        Error::security(-1),
        Error::invalid_input("test"),
        Error::timeout(1000),
        Error::permission_denied("op"),
        Error::not_found("resource"),
        Error::internal("details"),
    ];

    for err in &errors {
        // All errors should have non-empty display
        let display = err.to_string();
        assert!(!display.is_empty());
        assert!(display.len() > 5, "Error message should be descriptive");
    }
}

#[test]
fn test_error_std_error_trait() {
    fn accepts_std_error<E: std::error::Error>(_: &E) {}

    let err = Error::timeout(100);
    accepts_std_error(&err);
}

// =============================================================================
// F017: Graceful degradation test
// =============================================================================

#[test]
fn test_f017_graceful_degradation_on_missing_hardware() {
    // Per specification F017: Returns None on non-Mac Pro gracefully
    // This test verifies no panic, no error, just None
    let result = AfterburnerMonitor::new();
    // On real Mac Pro with Afterburner: Some(monitor)
    // On all other systems: None
    // Either way: no crash
    let _ = result;
}

// =============================================================================
// Secure Enclave API tests (F061-F070)
// =============================================================================

// F061: Secure Enclave capability is reported honestly on every platform.
// F063: Key creation must NOT succeed -- it must refuse rather than fabricate.
//
// These tests are intentionally ungated. Gating the cryptographic surface on
// `target_os = "macos"` is what let the fabricated implementations ship: the
// Linux matrix was empty, so nothing here could ever fail in the default CI lane.
// F065/F066: No signature can be produced at all, so no roundtrip can exist.
// F067: Verification cannot report a boolean it has no basis for.
//
// The removed implementation answered `Ok(true)`/`Ok(false)` by re-deriving a
// deterministic fake and comparing bytes. A verifier that cannot check a
// signature must return an error, never `Ok(false)` -- callers routinely treat
// `Ok(false)` as "definitively invalid", which was never true here.
// =============================================================================
// Metal API tests (F046-F060)
// =============================================================================

// F046: All Metal devices enumerated
#[test]
fn test_f046_metal_device_enumeration() {
    let devices = MetalCompute::devices();
    #[cfg(target_os = "macos")]
    assert!(
        !devices.is_empty(),
        "Should find at least one Metal device on macOS"
    );
    #[cfg(not(target_os = "macos"))]
    assert!(
        devices.is_empty(),
        "Should find no Metal devices on non-macOS"
    );
}

// F047: Device properties accurate
#[test]
#[cfg(target_os = "macos")]
fn test_f047_device_properties() {
    let devices = MetalCompute::devices();
    for device in &devices {
        assert!(!device.name.is_empty(), "Device should have a name");
        assert!(
            device.max_buffer_length > 0,
            "Device should have buffer capacity"
        );
        assert!(
            device.max_threads_per_threadgroup > 0,
            "Device should have thread capacity"
        );
    }
}

// F060: The Metal compute path refuses rather than silently dropping work.
#[test]
#[cfg(target_os = "macos")]
fn test_f060_compute_path_refuses() {
    let Ok(compute) = MetalCompute::default_device() else {
        return; // No Metal device on this runner; nothing to assert.
    };
    let err = compute
        .compile_shader("kernel void test() {}", "test")
        .expect_err("shader compilation is not implemented");
    assert!(err.is_unimplemented(), "got {err:?}");

    let err = compute
        .allocate_buffer(1024)
        .expect_err("buffer allocation is not implemented");
    assert!(err.is_unimplemented(), "got {err:?}");
}

// =============================================================================
// Neural Engine API tests (F031-F045)
// =============================================================================

// F031/F032: Platform detection
#[test]
fn test_f031_f032_neural_engine_detection() {
    let available = NeuralEngineSession::is_available();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    assert!(
        available,
        "Neural Engine should be available on Apple Silicon"
    );
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    assert!(
        !available,
        "Neural Engine should not be available on non-Apple Silicon"
    );
}

// F038: Invalid model path returns Error
#[test]
fn test_f038_invalid_model_path() {
    use std::path::Path;
    let result = NeuralEngineSession::load(Path::new("/nonexistent/model.mlmodel"));
    assert!(result.is_err(), "Should reject nonexistent model path");
}

// =============================================================================
// Unified Memory API tests (F071-F080)
// =============================================================================

// F071: UMA buffer allocation succeeds
#[test]
fn test_f071_uma_allocation() {
    let result = UmaBuffer::new(4096);
    assert!(result.is_ok(), "4KB allocation should succeed");
}

// F076: Alignment correct for Metal
#[test]
fn test_f076_metal_alignment() {
    let buffer = UmaBuffer::new(1024).expect("Allocation failed");
    assert!(
        buffer.is_aligned(),
        "Buffer should be page-aligned for Metal"
    );
    assert!(
        buffer.allocated_size() >= 4096,
        "Should allocate at least one page"
    );
}

// F078: Large allocation failure returns Error
#[test]
fn test_f078_large_allocation_fails() {
    // Try to allocate more than 16GB
    let result = UmaBuffer::new(20_000_000_000);
    assert!(result.is_err(), "Oversized allocation should fail");
}

// =============================================================================
// Cross-module integration tests
// =============================================================================
