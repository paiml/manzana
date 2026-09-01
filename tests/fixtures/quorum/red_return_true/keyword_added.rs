// RED fixture: `return true;` is the same capability lie as `true`.
//
// The capability-without-probe patterns are textual and matched `{ true }` /
// `{ true` / `true }`. A body of `{ return true; }` matches none of them --
// it is "{ return", and "true; }" -- so RUSTSEC-2026-0273's headline defect,
// a capability predicate asserting a constant with no runtime probe, passed
// this gate with one keyword added.

/// Asserts a capability as a constant. Probes nothing.
pub fn is_smc_available() -> bool {
    return true;
}
