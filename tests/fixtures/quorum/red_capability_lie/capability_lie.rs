//! FIXTURE 2 -- must produce RED (`capability-without-probe`).
//!
//! The 0.1.0/0.2.0 shape: a compile-time constant asserting hardware presence.
//! A `const fn` cannot have consulted the machine it runs on, and `target_arch`
//! describes the BUILD, not the host.

pub const fn is_available() -> bool {
    true
}
