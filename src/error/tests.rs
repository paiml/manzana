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

/// Every constructor must build the variant it names, and carry its argument.
///
/// Was nine `let _ = ...` lines. It passed if every constructor were replaced
/// by a single constant -- `Error::timeout(0)` for all nine would have been
/// green. An error type whose constructors are untested is a poor foundation
/// for a crate whose central claim is that it returns the right error.
#[test]
fn test_error_constructors_build_what_they_name() {
    let e = Error::not_available(Subsystem::Afterburner);
    assert!(e.is_not_available());
    assert!(e.to_string().contains("Afterburner"));

    let e = Error::iokit(42, "msg");
    assert_eq!(e.error_code(), Some(42), "the kern_return_t must survive");
    assert!(e.to_string().contains("msg"));

    for (e, needle) in [
        (Error::metal("mmm"), "mmm"),
        (Error::coreml("ccc"), "ccc"),
        (Error::invalid_input("iii"), "iii"),
        (Error::permission_denied("ppp"), "ppp"),
        (Error::not_found("nnn"), "nnn"),
        (Error::internal("ddd"), "ddd"),
    ] {
        assert!(
            e.to_string().contains(needle),
            "constructor dropped its argument: {e}"
        );
    }

    let e = Error::timeout(100);
    assert!(
        e.to_string().contains("100"),
        "the timeout duration must reach the message: {e}"
    );

    // The variants must be DISTINGUISHABLE -- a single constant would satisfy
    // every assertion above if they all rendered the same.
    let rendered = [
        Error::not_available(Subsystem::Metal).to_string(),
        Error::metal("x").to_string(),
        Error::coreml("x").to_string(),
        Error::invalid_input("x").to_string(),
        Error::timeout(1).to_string(),
        Error::permission_denied("x").to_string(),
        Error::not_found("x").to_string(),
        Error::internal("x").to_string(),
    ];
    let mut unique: Vec<&String> = rendered.iter().collect();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        rendered.len(),
        "two constructors render identically, so a caller cannot tell them \
         apart: {rendered:?}"
    );
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
