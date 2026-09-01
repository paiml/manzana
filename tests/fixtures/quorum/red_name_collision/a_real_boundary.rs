//! Half of the NAME-COLLISION fixture (must be RED together with b_*.rs).
//!
//! A genuine boundary-reaching function. Its only job is to share a name with
//! the fabricator next door.

pub fn is_available() -> bool {
    std::process::Command::new("system_profiler")
        .arg("SPDisplaysDataType")
        .output()
        .is_ok()
}
