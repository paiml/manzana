//! Only private functions. Isolates the vacuous-pass guard: functions ARE
//! extracted, so the extraction guard cannot fire, yet zero are public. A gate
//! that examined zero public fns must refuse, not pass.
fn helper(a: usize) -> usize {
    a + 1
}
