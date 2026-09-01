// RED fixture: a Result-returning fn that reaches NOTHING must not be
// certified as reaching a boundary because its body contains a call whose
// BARE NAME matches some other function that does.
//
// `AfterburnerMonitor::new` genuinely reaches IOKit, so the gate marks the
// name `new` as boundary-reaching. Call-edge resolution matched the bare text
// `new(` anywhere in a body, so `Vec::new()`, `String::new()` and
// `HashMap::new()` all counted as calling it -- laundering a boundary verdict
// onto code that touches no hardware.
//
// `read_stats` below is the 0.2.0 shape exactly: it promises fallible hardware
// work in its signature, does none, and returns a plausible constant. The gate
// must report a VIOLATION for it.

pub struct Monitor;

impl Monitor {
    /// Reaches a real boundary. Named `new`, as the real constructor is.
    pub fn new() -> Option<Self> {
        let handle = unsafe { IOServiceGetMatchingService(0, 0) };
        if handle == 0 { None } else { Some(Self) }
    }
}

/// Promises fallible hardware work. Does none. Returns a fabricated reading.
/// Its only claim to the boundary is the text `new(` from `Vec::new()`.
pub fn read_stats() -> Result<Vec<u32>> {
    let mut samples = Vec::new();
    samples.push(23);
    Ok(samples)
}
