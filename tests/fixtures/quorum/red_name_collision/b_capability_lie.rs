//! Half of the NAME-COLLISION fixture (must be RED).
//!
//! The RUSTSEC-2026-0273 capability-without-probe shape, under a name that
//! collides with the boundary-reaching function in a_real_boundary.rs.
//!
//! THE GATE SHIPPED GREEN ON THIS. The reach table was keyed on the bare
//! function NAME, so this inherited the other function's verdict. Every other
//! fixture directory holds exactly ONE file, and that isolation is precisely
//! what hid the bug -- a fixture cannot exercise a collision it cannot have.

pub const fn is_available() -> bool {
    true
}
