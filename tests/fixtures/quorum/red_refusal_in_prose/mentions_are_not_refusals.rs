// RED fixture: the refusal limb is a TEXT match, so a function that merely
// MENTIONS a refusal symbol -- in a block comment, or inside a string literal
// -- must not be certified as refusing.
//
// `//` line comments were already stripped. `/* */` and string literals were
// not, so both of these passed the gate while fabricating.

/// Fabricates. The refusal symbol appears only inside a string literal.
pub fn fabricates_with_symbol_in_a_string() -> Result<u32> {
    let _note = "this would be Error::unimplemented if we had a backend";
    Ok(23)
}

/// Fabricates. The refusal symbol appears only inside a block comment.
pub fn fabricates_with_symbol_in_a_block_comment() -> Result<u32> {
    /* one day this returns Error::unimplemented instead */
    Ok(23)
}
