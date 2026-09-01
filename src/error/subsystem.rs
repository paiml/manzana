//! The hardware subsystems manzana can report on.

use std::fmt;

/// The hardware subsystem an [`enum@Error`] refers to.
///
/// Carried by [`Error::NotAvailable`] and [`Error::Unimplemented`] so a caller
/// can tell which accelerator a refusal concerns.
///
/// ```
/// use manzana::error::Subsystem;
///
/// assert_eq!(Subsystem::Metal.to_string(), "Metal GPU");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subsystem {
    /// Apple Afterburner FPGA (Mac Pro 2019+). Displays as "Afterburner FPGA".
    Afterburner,
    /// Apple Neural Engine (Apple Silicon). Displays as "Neural Engine".
    NeuralEngine,
    /// Metal GPU compute. Displays as "Metal GPU".
    Metal,
    /// Unified memory. Displays as "Unified Memory".
    ///
    /// No error produced by manzana 0.3.0 carries this subsystem;
    /// `unified_memory` reports its failures as [`Error::InvalidInput`] or
    /// [`Error::Internal`].
    UnifiedMemory,
}

impl fmt::Display for Subsystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Afterburner => write!(f, "Afterburner FPGA"),
            Self::NeuralEngine => write!(f, "Neural Engine"),
            Self::Metal => write!(f, "Metal GPU"),
            Self::UnifiedMemory => write!(f, "Unified Memory"),
        }
    }
}
