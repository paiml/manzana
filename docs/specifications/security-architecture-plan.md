# Security Architecture Plan — manzana

| | |
|---|---|
| **Status** | Draft for review |
| **Advisory** | [RUSTSEC-2026-0273][adv] — `crypto-failure`, filed 2026-04-07 |
| **Issue** | [paiml/manzana#3][i3] |
| **Affected** | 0.1.0, 0.2.0 — **both yanked** |
| **Target** | 0.3.0 |

Sources: a 12-expert security quorum with adversarial cross-examination, a
parallel claims-vs-reality audit (108 findings, 13 critical), `pmat` static
analysis, and empirical probes on an Apple M4. Every claim below was verified
against the source or measured; findings that survived only as assertion are
marked as such.

---

## 1. Executive summary

manzana 0.1.0 and 0.2.0 advertised hardware-backed P-256 ECDSA signing via the
Apple Secure Enclave. No cryptography was performed. `sign()` returned a
public function of the message and the key tag; `verify()` recomputed that same
function and compared it to itself. Neither involved a key, a secret, or the
Secure Enclave.

The same pattern — **fabricate a plausible result rather than report
failure** — recurred in `neural_engine`, `metal`, and `unified_memory`. It was
not a single bad function. It was a house style.

0.3.0 removes every fabricating code path. Operations that cannot reach real
hardware return `Error::Unimplemented`. The `secure_enclave` module delegates
to [`security-framework`][sf] rather than binding `Security.framework` itself,
which makes fabrication *structurally* impossible rather than merely absent.

---

## 2. Incident analysis

### 2.1 The signature was a public 16-bit checksum in a DER costume

From `src/secure_enclave.rs:422-448` at `89e3183`:

```rust
let r_seed = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
sig_bytes.extend_from_slice(&[r_seed; 32]);          // r
let s_seed = self.tag.bytes().fold(0u8, u8::wrapping_add);
sig_bytes.extend_from_slice(&[s_seed; 32]);          // s
```

- `r` = 32 copies of the wrapping byte-sum of the message. **8 bits**, and a
  *linear* function of the message.
- `s` = 32 copies of the wrapping byte-sum of the tag. **8 bits**, and
  **constant for the lifetime of the key** — independent of the message.
- Both inputs are public. There is no secret anywhere in the computation.

**Forgery cost: universal forgery, zero signing queries**, given only the tag
string — one allocation plus `|m|` byte additions. Without the tag, a single
observed signature discloses `s` directly, since `s` never varies.

A concrete collision, useful for the disclosure: appending **eight space
characters** to any signed document preserves its signature exactly
(`0x20 × 8 = 256 ≡ 0 mod 256`). So does transposing any two bytes.

### 2.2 `verify()` was a tautology

```rust
let expected = self.sign(data)?;
Ok(expected.as_bytes() == signature.as_bytes())
```

A verifier that recomputes its own subject has **zero falsification power by
construction**. This generalises past this bug: *any* oracle sharing an
implementation with the thing it checks can only ever agree with itself.

### 2.3 Fake success on a destructive operation

`delete()` returned `Ok(())` without deleting. This is a worse class than a
fake signature: a fake signature is falsifiable by anyone who hands it to a
real verifier, whereas fake deletion is **unfalsifiable by the operator** —
there is nothing to inspect. An operator told "the key is destroyed" may
decommission hardware or publish a revocation on that basis.

The signature `delete(self) -> Result<()>` is itself deficient: "a key existed
and I removed it" and "there was nothing there" are both `Ok(())`, though
`SecItemDelete` distinguishes them.

### 2.4 `load()` was not the honest stub it appeared to be

It returned `Err(NotFound)`. `NotFound` is a *domain answer* — it asserts a
fact about the keychain ("I looked; it is absent") that was never established.
`Unimplemented` is the only truthful response.

### 2.5 Capability detection asserted hardware it never queried

`is_available()` was a `const fn` returning `true` on all macOS. A `const fn`
**cannot** have consulted the machine it runs on; `target_arch` is a property
of the build, not the host. On x86_64 it returned `true` with the comment
*"assume available on macOS x86_64 (conservative)"*. For a capability
predicate, conservative is `false`. Assuming a security capability is present
is the maximally unsafe direction.

> The advisory records `arch = ["aarch64"]`. That understates it: x86_64 macOS
> was equally affected, including pre-T2 Intel Macs with no Secure Enclave at
> all.

### 2.6 The same pattern elsewhere

| Location | Fabrication |
|---|---|
| `neural_engine::infer` | `Tensor::zeros(input.shape)`, silently |
| `neural_engine::capabilities` | M1 baseline (15.8 TOPS/16 cores) on any chip |
| `neural_engine::load` | Session returned after checking only the filename |
| `metal::dispatch` | `Ok(())`, nothing dispatched |
| `metal::compile_shader` | "Compiled" by hashing the source string |
| `metal::allocate_buffer` | Handle holding no memory |
| `metal::fallback_device` | Invented "Apple GPU", including on non-macOS |

### 2.7 A separate soundness defect (UB)

`UmaBuffer::new` allocated with `alloc()` — uninitialized — while the **safe**
methods `as_slice`/`as_mut_slice` construct a `&[u8]` over it. A reference to
uninitialized memory is undefined behaviour, reachable from entirely safe code:

```rust
let b = UmaBuffer::new(1024)?;  // safe
let _ = b.as_slice()[0];        // safe -> UB
```

Unrelated to the advisory, but shipped in the same artifacts. Fixed via
`alloc_zeroed`, with a regression test that reads the buffer back.

---

## 3. Threat model

**Assets**, by blast radius:

- **A1 — the caller's trust decision.** What downstream code does with
  `Ok(true)` from `verify()`. The top asset, because it is the only one the
  crate can destroy without touching key material.
- **A2 — signature artifacts** already produced and distributed.
- **A3 — operator belief about key destruction** (`delete`).
- **A4 — capacity/telemetry decisions** made from fabricated hardware specs.

**Trust boundaries:**

- **TB0 — crates.io artifact ↔ git repo.** *Already breached*: the published
  0.2.0 has no reproducible source (§6.2).
- **TB1 — caller ↔ manzana API.** The boundary this incident crosses. manzana
  runs inside the caller's trust domain and returns unearned assurances.
- **TB2 — process ↔ securityd/SEP.** Never actually reached.

**STRIDE, restricted to what was real:**

| | |
|---|---|
| **Spoofing** | Any party forges any identity bound to a key; `is_available()` spoofed hardware presence |
| **Tampering** | Documents modifiable while preserving the signature (§2.1) |
| **Repudiation** | Signatures bind nothing, so no signer can be held to one |
| **Info disclosure** | Not applicable — no secret existed to leak |
| **DoS** | Not applicable |
| **Elevation** | `verify() -> Ok(true)` grants whatever the caller gates on it |

**The governing threat is not weak cryptography. It is a fabricated assurance
oracle** — a dependency that returns `Ok(true)` for a security predicate it
never evaluated. Weak crypto degrades a guarantee; a fabricated oracle
manufactures one that never existed.

---

## 4. Root cause

Not "someone forgot." Four independent mechanisms each had to fail:

**4.1 Nothing in the type system prevented it.** `Signature::from_bytes` was
`pub` and validated only `64 <= len <= 72`, so `vec![0x30; 70]` was a valid
`manzana::Signature`. `PublicKey::from_bytes` checked length and a `0x04`
prefix with no on-curve test, so it accepted the shipped fake key
`04 || 00×32 || 01×32` — **which is not a point on P-256**.

> Verified independently and exhaustively for this document. The fabricated key
> had `X = tag_hash · 2^248` and `Y = 0x0101…01`, so only **256 distinct keys**
> were reachable, one per `tag_hash` byte. Checking
> `y² ≡ x³ − 3x + b (mod p)` for all 256: **zero are on the curve.** The
> fabrication was refutable by arithmetic alone, on any machine, with no Apple
> hardware and no access to a Secure Enclave — had anything ever checked.

**4.2 The tests certified the fabrication.** `test_f065_f066_signature_roundtrip`
asserted the round-trip *succeeded* — which a self-consistent fake passes
perfectly. A property test asserted signature stability, its own comment
calling it a *"deterministic stub"*. The length assertion in
`test_sign_and_verify` was **entailed by the constructor's own validation** —
a tautology, unfalsifiable by any reachable input.

**4.3 The CI gate structurally could not see it.** 26 of 163 `#[test]` fns
carried item-level `#[cfg(target_os = "macos")]`. On Linux these are not
skipped — they **do not exist**, so `cargo test` reports `0 ignored`. Zero
tests passing is indistinguishable from all tests passing. The `tests-174
passing` badge was a hardcoded string linking to the repo rather than any CI
run; the true Linux figure was 141.

**4.4 The quality tooling validated form, not substance.**

| Signal | Reported | Reality |
|---|---|---|
| `pmat analyze satd` | **0 debt** | Comments literally read `// generates a fake public key` — the scanner matched `TODO`, not `Stub:` |
| `pmat comply` *falsification* | **10/10 pass** | Falsification claims asserted against fakes |
| `make miri` | pass | Ended in `2>/dev/null \|\| echo` — **could not fail**; sole basis for "MIRI-verified" |
| `make deny` | pass | Same shape; masked a real advisory |
| Popper falsifiability | 45/100 (F) | Correctly poor, and ignored |

> **Where the honesty was.** The truthful statements — `"Stub implementation -
> generates a fake public key"` — were in `//` line comments, invisible in
> rendered rustdoc. The `///` doc comments directly above them described real
> hardware cryptography. The code told the truth only where no user would look.

---

## 5. Security architecture

### 5.1 The governing rule

> **An operation that cannot reach the hardware it claims to use must fail
> loudly. It must never return a value a caller could mistake for a genuine
> result.**

Encoded as `Error::Unimplemented` and asserted by ungated refutation tests.

### 5.2 Delegate rather than bind

`secure_enclave` delegates to `security-framework` (349M downloads). This is
the primary structural control: `SecKey::new` returns a `Result` which is
propagated, so **there is no code path in manzana that can invent a key or a
signature.** This property holds independently of whether the success path has
been exercised — which matters, because it currently cannot be (§5.4).

Hand-writing the FFI was rejected: unverifiable hand-rolled crypto FFI is a
larger risk than the one being fixed.

### 5.3 Make fabrication unrepresentable, not merely absent

Runtime `Unimplemented` guards are a day-one measure, not the architecture.
The quorum's strongest recommendation, endorsed here:

- **`Signature` must not be constructible from arbitrary bytes.** *Partially
  done*: `from_bytes` now parses the DER `ECDSA-Sig-Value` structure —
  tag, lengths, minimal-form integers, sign bits. Note honestly that **this
  does not exclude the original forgery**, which was well-formed DER. Structure
  checking was never going to catch fabrication; only removing `sign()` does.
  The remaining step is a sealed hardware witness so a `Signature` can only
  originate from a backend that reached the SEP.
- **`PublicKey` must reject non-points.** *Not done* — requires P-256 field
  arithmetic; see §7.

### 5.4 Honest capability reporting

`is_available()` reports **capability**, not hardware presence: "can this
library sign for me right now?" It should be de-`const`ed before republish so
that adding a real runtime probe later is not a breaking change.

---

## 6. Distribution and disclosure

### 6.1 Yank semantics — do not present the yank as the remediation

Yank removes the versions from the candidate set for **new** resolution only.
It deletes nothing; the tarballs remain on `static.crates.io` permanently.
Existing `Cargo.lock` files resolve against yanked versions without complaint,
and `cargo audit` does not flag yanked crates by default. **The yank does not
reach the 3,824 existing downloads.** The advisory does.

Crate deletion is unavailable (requires <500 downloads, 0 reverse deps, <72h
since publication), so yank is the registry-side ceiling.

### 6.2 Provenance

`.cargo_vcs_info.json` in the published 0.2.0 names sha
`4a76402d47d6421911e27d2fbab01b76c99fb560`, which **does not exist in the
repository**. `git tag -l` is empty; no release was ever tagged. A quorum
member empirically demonstrated that `cargo package --allow-dirty --no-verify`
still writes a `.cargo_vcs_info.json`, so the recorded sha is *not* evidence
the tree was clean. **The published artifact is not reproducible from source.**

Actions: preserve the object if it exists on any machine; tag every future
release; publish only from CI on a tagged, clean tree.

### 6.3 RUSTSEC-2026-0273

Currently `patched = []`. Its own text sets the bar: *"no versions… contain
either a real implementation, **or a warning about the stubbed
cryptography**."* 0.3.0 clears the second. Proposed PR: set
`patched = [">= 0.3.0"]`, drop the over-narrow `arch = ["aarch64"]`, append a
resolution note. Whether 0.3.0 counts as patched for a missing-functionality
advisory is the RustSec maintainers' call, to be **presented, not assumed**.

---

## 7. Remediation plan

Acceptance criteria are falsifiable — each names something that can fail.

### Done in 0.3.0

| # | Item | Acceptance |
|---|---|---|
| 1 | Remove every fabricating body | `grep` finds no synthetic key/signature/tensor construction in `src/` |
| 2 | `Error::Unimplemented` wired | Ungated tests assert `err.is_unimplemented()` for each op |
| 3 | Ungate security tests | No item-level `cfg(target_os)` on any security test; they run on Linux |
| 4 | Invert fake-certifying tests | Round-trip and determinism tests assert refusal |
| 5 | `make miri` / `make deny` can fail | Both exit non-zero on failure; no `\|\| echo` |
| 6 | Fix `UmaBuffer` UB | `alloc_zeroed`; regression test reads the buffer |
| 7 | RUSTSEC-2026-0204 | `cargo deny check advisories` exits 0 |
| 8 | MSRV violation (`reason =` needs 1.81) | Builds on 1.75 |
| 9 | `Signature` parses DER | `vec![0x30; 70]` rejected |
| 10 | Correct false docs | No "MIRI-verified"; `deny` not `forbid`; unsafe not confined to `src/ffi/` |
| 11 | Honest README matrix + CHANGELOG + credit reporters | Present |
| 12 | Quarantine stale QA report | Marked superseded; excluded from package |

### Before republish (P1)

| # | Item | Acceptance | Verifiable on Linux |
|---|---|---|---|
| 13 | Real `security-framework` backend | `create` returns a genuine `OSStatus`, never a synthetic key | Partly — error path only |
| 14 | De-`const` `is_available()` | Signature is non-`const` | Yes |
| 15 | Tag the release | `git tag -l` non-empty | Yes |
| 16 | Ban item-level `cfg(target_os)` on `#[test]` | CI gate fails if reintroduced | Yes |
| 17 | Test-census gate | CI asserts a minimum test count, so 0-run cannot read as pass | Yes |

### Roadmap (P2)

| # | Item |
|---|---|
| 18 | Sealed hardware witness making `Signature` unrepresentable without SEP |
| 19 | On-curve validation in `PublicKey::from_bytes` (needs P-256 arithmetic — dependency decision) |
| 20 | Apple Developer cert → signed harness → prove the SE success path (§5.4) |
| 21 | Wire feature flags — no `#[cfg(feature)]` exists in `src/` today, so flags gate nothing |
| 22 | Contract machinery: `manzana-tensor-v1.yaml` does not exist; the `#[contract]` attribute proves nothing |
| 23 | `delete` signature distinguishing "removed" from "absent" |

---

## 8. Test and CI architecture

1. **No item-level `#[cfg(target_os)]` on `#[test]`.** Use a runtime guard so
   the test exists, is counted, and is visibly skipped.
2. **Census, not just exit code.** CI asserts a minimum test count. Zero tests
   passing must not read as success.
3. **Refutation over confirmation.** Tests attempt to catch the code lying.
   Never assert a round-trip that a self-consistent fake satisfies.
4. **Known-answer vectors** from an independent implementation, once a real
   backend exists. A round-trip against oneself proves nothing (§2.2).
5. **No unfalsifiable badges.** Counts must come from a CI run or not appear.

---

## 9. Non-goals — what manzana does not claim

- Not a cryptography library. Use [`security-framework`][sf].
- No remote attestation. **Apple provides no key-attestation API for
  general-purpose `SecKey`s on macOS** — there is no analogue of Android Key
  Attestation or TPM 2.0 quoting. An SE signature attests only that a process
  on this machine held a keychain handle whose access-control policy `securityd`
  considered satisfied. It is **not** transferable evidence to a remote
  verifier, and "model attestation" must not be claimed on it.
- No CoreML inference, Metal compute, or GPU-shared memory.
- `UmaBuffer` is a host allocation, not a `MTLBuffer`.

---

## 10. Open questions

1. Does the `4a76402` object survive on any machine? It scopes §6.2.
2. Apple Developer account for a signed test harness (§7 item 20)?
3. Accept a P-256 dependency for on-curve validation (§7 item 19)?
4. Should the single crates.io reverse dependency be contacted directly, since
   the yank will not reach them?
5. Should the API token that published 0.1.0/0.2.0 be rotated?

[adv]: https://rustsec.org/advisories/RUSTSEC-2026-0273.html
[i3]: https://github.com/paiml/manzana/issues/3
[sf]: https://crates.io/crates/security-framework
