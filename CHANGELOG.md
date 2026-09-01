# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] — 2026-09-01

### Fixed

Documentation only. No code behaviour changes, no API changes. Every item
here is a doc that described behaviour the code does not have — the same
defect class 0.3.0 was released to remove, found by continuing to review the
published artifact after it shipped.

- `AfterburnerService::get_stats`'s `# Errors` still said a registry missing
  every property "parses into `AfterburnerRawStats` fallbacks rather than an
  error". That was the **seventh** copy of the defaulting claim; 0.3.0
  corrected six and missed this one. Absent properties are `None` here, and
  `crate::afterburner` turns a `None` in a required field into `Err`.
- `MetalDevice`'s type doc said "the other six fields" when the struct has
  **nine**, and classified `reported_vram_bytes` — the field 0.3.0 added
  precisely to record provenance — as though it carried none.
- `MetalDevice::name` was documented as the report line "with the trailing `:`
  removed". That describes the blacklist parser 0.3.0 deleted, the one that
  invented devices called `Software`. It is the value of the `Chipset Model:`
  line, trimmed.
- The `neural_engine` falsification claim F032 read "Returns None on Intel
  Mac", implying `capabilities()` returns figures somewhere. It returns `None`
  on every platform, Apple Silicon included.
- `Tensor::zeros`'s comment claimed saturation "turns a panic into an
  allocation failure". In Rust a failed allocation of that size aborts or
  panics too, so that sentence distinguished nothing. What saturating actually
  avoids is a **wrapped** length: a `Vec` shorter than the shape claims, which
  is a silently wrong tensor rather than a loud failure. The process still
  dies; it just dies instead of lying.
- The README's `Error` table listed only two `IoKit` cases and omitted the
  missing-property error `stats()` began returning in 0.3.0.

## [0.3.0] — 2026-09-01

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

- **The removed fabrications came back through `Default`.** Both of the
  constants deleted from the hardware paths were still reachable from safe
  public API, and by an idiomatic line of Rust rather than an obscure one.
  Found in the pre-publish artifact review, demonstrated on x86_64 Linux with
  no Apple hardware present:
  - `NeuralEngineSession::capabilities()` returns `Option`, so
    `capabilities().unwrap_or_default()` yielded `AneCapabilities::default()` —
    the M1's published 15.8 TOPS and 16 cores — on any machine at all. That is
    the exact figure the advisory names, restored by a trait impl after being
    removed from `capabilities()` itself. `impl Default for AneCapabilities` is
    gone. The figures first moved behind a named constructor,
    `AneCapabilities::m1_baseline()`; then the review pointed out that 15.8 TOPS
    is the **M2's** published figure, not the M1's, so the constructor's own
    justification was false and it was deleted too. manzana now states no TOPS
    and no core count for any Apple part: it cannot measure one and declines to
    repeat one. `capabilities().unwrap_or_default()` no longer compiles (E0277).
  - `AfterburnerMonitor::stats()` returns `Result`, so
    `stats().unwrap_or_default()` yielded `streams_capacity: 23` — Apple's
    marketed "23 streams of 4K ProRes" — plus zero streams and zero
    utilisation: a complete, plausible idle-card reading on a machine with no
    card. `AfterburnerStats::default()` is now all-zero, and `0` capacity is
    not a card anyone can mistake for a measurement.
  `[V]` `cargo test test_default_does_not_reconstitute_the_marketed_capacity`
  — RED against the previous impl (`left: 23, right: 0`)
- **`AfterburnerStats`'s rustdoc still described the fabricating behaviour as
  current.** It stated that an absent registry property "is silently replaced
  by the default shown below", tabulated `23` as that default, and told the
  reader "there is no way to tell a defaulted field from a genuine reading" —
  all false since `convert_raw_stats` began returning `Err`. The doc survived
  the fix it documents. Corrected, with the old behaviour kept and labelled as
  history. Same for `is_active()`'s note about defaulted snapshots.
- **A vacuous test.** `metal::tests::test_detect_gpu_vram` wrapped its whole
  body in `if !devices.is_empty()`, so replacing `MetalCompute::devices()` with
  `Vec::new()` left it passing green having asserted nothing.

  Removing the guard exposed a second defect rather than fixing the first: the
  replacement asserted "at least one Metal device on macOS", which is a
  hardware assumption wearing a platform assumption's clothes, and it failed on
  GitHub's headless macos-latest runner. It now asks `system_profiler` what
  this host actually reported and asserts agreement — falsifiable both on a
  machine with a GPU (fails against an empty list) and on one without (fails
  against a fabricated fallback device). Three sibling tests and one in
  `tests/integration_tests.rs` had the same defect and got the same treatment.
- **Unit labels.** `17_179_869_184` is 16 GiB, not 16 GB, and was documented as
  "GB" in five places while `vram_gb()` divides by 2^30. `system_profiler`
  prints "GB" meaning GiB, and the crate now says so where it parses it.
- **Doctest identities in the census gate** were keyed by source line number,
  so any edit above a doctest reported it as removed-and-re-added. One
  docs-only pass produced 7 spurious removals and 20 spurious additions — and a
  genuinely deleted doctest would have been invisible in that noise, defeating
  the gate's name-set assertion exactly when it was needed. Keys are now
  line-independent.
  `[V]` inserting a line above a doctest produces no census churn; converting a
  doctest to ```` ```text ```` is still reported as a removal
- The published crate shipped `contracts/.pv/cache/lint/*.json` — cached `pv`
  verdicts computed on the author's machine, travelling inside the artifact for
  an auditor to be served instead of recomputing. Excluded.

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
  `[V]` `! grep -rq "secure_enclave::\|mod secure_enclave" src/ && ! test -e src/secure_enclave && ! test -e src/ffi/security.rs`

  (The earlier receipt read `grep -r secure_enclave src/ returns nothing` and
  was FALSE when executed: one doc comment in `neural_engine` cites the deleted
  module to explain why a test constructor exists. A falsifiable receipt that
  fails when run is worse than no receipt, and this one sat in the changelog
  entry about false claims. The command above is the claim actually meant --
  no code references the module, and neither it nor `ffi/security.rs` exists --
  and it passes.)
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
