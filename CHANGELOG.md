# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] — unreleased

### Security

Addresses [RUSTSEC-2026-0273][adv] (`crypto-failure`), reported in
[issue #3][i3] by [@ZephyrCodesStuff][z] and corroborated by [@djc][d].
**Versions 0.1.0 and 0.2.0 are yanked.**

Those versions returned fabricated values from operations documented as real
hardware work. In `secure_enclave`:

- `create()` returned a fixed public key whose only variation was one byte set
  to a wrapping byte-sum of the key tag. No key was generated anywhere.
- `sign()` returned a DER-shaped value with `r` = 32 copies of a byte-sum of
  the message and `s` = 32 copies of a byte-sum of the tag — about 8 bits of
  entropy per half, and derivable by anyone.
- `verify()` re-derived that same value and compared bytes, so it accepted
  forgeries from anyone who knew the tag and had no cryptographic meaning.
- `delete()` returned `Ok(())` without deleting anything — fake success on a
  destructive security operation.
- `is_available()` returned `true` unconditionally on macOS, including x86_64
  hosts with no T2 chip. (The advisory records the affected platform as
  `aarch64`; it was in fact broader.)

The same pattern existed outside the module named in the advisory:

- `neural_engine::infer()` returned `Tensor::zeros(input.shape)` silently.
- `neural_engine::capabilities()` returned the M1 baseline (15.8 TOPS, 16
  cores) on every Apple Silicon chip, as though measured.
- `neural_engine::load()` reported a loaded model after checking only the
  filename; the file was never opened.
- `metal::dispatch()` returned `Ok(())` having dispatched nothing.
- `metal::compile_shader()` "compiled" by hashing the source string.
- `metal::allocate_buffer()` returned a handle holding no memory.
- `metal::fallback_device()` invented an "Apple GPU" whenever detection
  failed, including on non-macOS hosts.

### Fixed

- **Soundness (UB):** `UmaBuffer::new` allocated with `alloc()`, leaving the
  buffer uninitialized, while the safe methods `as_slice`/`as_mut_slice` built
  a `&[u8]` over it. Constructing a reference to uninitialized memory is
  undefined behaviour, and it was reachable from entirely safe code. Now
  allocates with `alloc_zeroed`.
  `[V]` `cargo test test_new_buffer_is_initialized_not_uninit`
- **Vulnerable dependency:** updated `crossbeam-epoch` 0.9.18 → 0.9.20 for
  [RUSTSEC-2026-0204][c] (invalid pointer dereference).
  `[V]` `cargo deny check advisories`
- `make miri` ended in `2>/dev/null || echo`, so it could never fail — it was
  the sole basis for the README's "MIRI-verified" claim. `make deny` had the
  same shape, converting real `cargo-deny` violations into a "not configured"
  message. Both now fail properly.
  `[V]` `make miri` with miri absent exits non-zero
- MSRV violation: a lint attribute used `reason = `, which requires Rust 1.81
  while this crate declares `rust-version = "1.75"`.
  `[V]` `cargo +1.75 check`

### Changed

- Operations that cannot reach real hardware now return the new
  `Error::Unimplemented` instead of a fabricated value. An operation that
  cannot do the real thing must fail loudly; a plausible-looking result is more
  dangerous, because a caller cannot distinguish it from a genuine one.
  `[V]` refusal tests asserting `err.is_unimplemented()` per operation
### Removed

- **`secure_enclave` is deleted, along with `src/ffi/security.rs` and the
  `secure-enclave` feature.** manzana ships no cryptography. Use
  [`security-framework`][sf].

  Deleting rather than delegating was the right call and the earlier plan to
  defer it to 0.4.0 was wrong: the deferral rested on "a live disclosure must
  not be churned", but nothing had been published, so there was no live release
  to churn. Wrapping `security-framework` (349M downloads) would have added
  zero capability while permanently attaching RUSTSEC-2026-0273 to this crate.

  798 lines of cryptographic surface removed, including a hand-rolled DER
  parser, a `PublicKey` type that accepted points not on the P-256 curve, and a
  test-only constructor that existed solely to make unreachable methods
  reachable. The advisory's scope is now closed by absence.
  `[V]` `grep -r secure_enclave src/` returns nothing
- Security-critical tests are no longer `#[cfg(target_os = "macos")]`-gated.
  That gating left the default Linux CI lane with an *empty* test set for the
  cryptographic surface — and zero tests passing is indistinguishable from all
  tests passing. The replacements assert refusal and run everywhere.
  `[V]` zero `cfg(target_os)` attributes on security tests
- Tests that asserted the fabricated behaviour were correct — including a
  property test whose own comment called it a "deterministic stub" — have been
  inverted to assert refusal.
- README leads with a security notice and an honest capability matrix
  separating what is implemented from what is not.
- Corrected false documentation: unsafe code is *not* confined to `src/ffi/`
  (`src/unified_memory.rs` has its own `allow`), the crate root uses
  `deny(unsafe_code)` rather than the documented `forbid`, and
  `src/ffi/security.rs` contains no FFI at all despite its name.
- `QA_REPORT_2026-01-06.md` marked superseded and excluded from the published
  package; it certified "120/120 PASS" over the fabricating build and shipped
  inside the 0.2.0 tarball.

### Known limitations

- Metal compute, CoreML inference, and ANE capability querying remain
  unimplemented and return `Error::Unimplemented`.
- `UmaBuffer` is a page-aligned **host** allocation. It is not a `MTLBuffer`
  and is not GPU-visible.

## [0.2.0] — 2026-01-10 [YANKED]

Yanked. See 0.3.0.

## [0.1.0] — 2026-01-06 [YANKED]

Yanked. See 0.3.0.

[adv]: https://rustsec.org/advisories/RUSTSEC-2026-0273.html
[c]: https://rustsec.org/advisories/RUSTSEC-2026-0204.html
[i3]: https://github.com/paiml/manzana/issues/3
[z]: https://github.com/ZephyrCodesStuff
[d]: https://github.com/djc
[sf]: https://crates.io/crates/security-framework
