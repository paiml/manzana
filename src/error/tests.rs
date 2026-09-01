//! Tests for the `error` module.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::*;

// F081: All errors implement std::error::Error
#[test]
fn test_error_implements_std_error() {
    fn assert_std_error<T: std::error::Error>() {}
    assert_std_error::<Error>();
}

// F082: Error messages are human-readable
#[test]
fn test_error_messages_are_readable() {
    let err = Error::not_available(Subsystem::Afterburner);
    let msg = err.to_string();
    assert!(msg.contains("Afterburner"));
    assert!(msg.contains("not available"));
}

// F083: IOKit errors include kern_return_t
#[test]
fn test_iokit_error_includes_code() {
    let err = Error::iokit(-536_870_206, "service not found");
    let msg = err.to_string();
    assert!(msg.contains("-536870206"));
    assert!(msg.contains("service not found"));
}

// F089: Error Display impl useful
#[test]
fn test_display_impl_not_generic() {
    let errors = vec![
        Error::not_available(Subsystem::Metal),
        Error::iokit(0, "test"),
        Error::metal("test"),
        Error::coreml("test"),
        Error::invalid_input("test"),
        Error::timeout(1000),
        Error::permission_denied("test"),
        Error::not_found("test"),
        Error::internal("test"),
    ];

    for err in errors {
        let msg = err.to_string();
        // No generic "Error" only messages
        assert!(msg.len() > 10, "Message too short: {msg}");
        assert!(!msg.eq_ignore_ascii_case("error"), "Generic message: {msg}");
    }
}

#[test]
fn test_subsystem_display() {
    assert_eq!(Subsystem::Afterburner.to_string(), "Afterburner FPGA");
    assert_eq!(Subsystem::NeuralEngine.to_string(), "Neural Engine");
    assert_eq!(Subsystem::Metal.to_string(), "Metal GPU");
    assert_eq!(Subsystem::UnifiedMemory.to_string(), "Unified Memory");
}

#[test]
fn test_error_constructors() {
    let _ = Error::not_available(Subsystem::Afterburner);
    let _ = Error::iokit(0, "msg");
    let _ = Error::metal("msg");
    let _ = Error::coreml("msg");
    let _ = Error::invalid_input("msg");
    let _ = Error::timeout(100);
    let _ = Error::permission_denied("op");
    let _ = Error::not_found("res");
    let _ = Error::internal("details");
}

#[test]
fn test_error_predicates() {
    assert!(Error::not_available(Subsystem::Metal).is_not_available());
    assert!(!Error::timeout(100).is_not_available());

    assert!(Error::timeout(100).is_timeout());
    assert!(!Error::not_available(Subsystem::Metal).is_timeout());

    assert!(Error::permission_denied("op").is_permission_denied());
    assert!(!Error::timeout(100).is_permission_denied());
}

#[test]
fn test_error_code_extraction() {
    assert_eq!(Error::iokit(42, "test").error_code(), Some(42));
    assert_eq!(Error::metal("test").error_code(), None);
}

#[test]
fn test_error_equality() {
    let e1 = Error::not_available(Subsystem::Afterburner);
    let e2 = Error::not_available(Subsystem::Afterburner);
    let e3 = Error::not_available(Subsystem::Metal);

    assert_eq!(e1, e2);
    assert_ne!(e1, e3);
}

#[test]
fn test_error_clone() {
    let e1 = Error::iokit(42, "test message");
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

#[test]
fn test_error_debug() {
    let err = Error::not_available(Subsystem::Afterburner);
    let debug = format!("{err:?}");
    assert!(debug.contains("NotAvailable"));
    assert!(debug.contains("Afterburner"));
}
