// RED fixture: a MULTI-LINE /* */ comment must not satisfy either limb.
//
// The stripper used the single-line C-comment regex, which needs the closing
// `*/` on the same record. awk has no cross-line state of its own, so a block
// comment spanning two lines survived intact into the body that both limbs
// text-match against. A fabricating fn whose comment merely MENTIONED
// Error::unimplemented, or a boundary symbol, was certified.
//
// Both functions below fabricate. Both must be RED.

/// Refusal named only inside a multi-line block comment.
pub fn read_die_temperature() -> Result<f32> {
    /* TODO(0.4.0): this must become
       Err(Error::unimplemented("no SMC binding yet"))
       once the SMC binding lands. */
    Ok(42.0)
}

/// Boundary named only inside a multi-line block comment.
pub fn read_gpu_core_count() -> Result<u32> {
    /* We would like to shell out to
       Command::new("system_profiler") here, but not yet. */
    Ok(10)
}
