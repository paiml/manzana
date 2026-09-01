<div align="center">

<img src="docs/hero.svg" alt="Manzana - Apple Hardware for Sovereign AI" width="600">

<h1 align="center">Manzana</h1>

<p align="center">
  <strong>Safe Rust interfaces to Apple hardware for Sovereign AI</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/manzana"><img src="https://img.shields.io/crates/v/manzana.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/manzana"><img src="https://docs.rs/manzana/badge.svg" alt="Documentation"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
</p>

</div>

---

## ⚠️ Security Notice — read before depending on this crate

**Manzana does not implement cryptography. Do not use it for signing, verification, key management, or attestation.**

Versions **0.1.0 and 0.2.0 have been yanked** from crates.io. Their
`secure_enclave` module advertised hardware-backed P-256 ECDSA signing but was
entirely stubbed:

- `create()` returned a fixed public key derived from a byte-sum of the key tag. No key was ever generated.
- `sign()` returned a DER-shaped value whose `r` was 32 copies of a byte-sum of the message and whose `s` was 32 copies of a byte-sum of the tag — roughly 8 bits of entropy per half.
- `verify()` recomputed that same value and compared bytes, so it accepted forgeries from anyone who knew the tag.
- `delete()` reported success without deleting anything.

The same pattern appeared elsewhere: `neural_engine::infer()` returned a
correctly-shaped all-zero tensor, and `metal::dispatch()` returned `Ok(())`
having dispatched nothing.

**As of 0.3.0 the `secure_enclave` module is DELETED, not fixed.** manzana
ships no cryptography at all. Wrapping `security-framework` would have added
zero capability while permanently attaching this advisory to the crate, so the
module is gone and the cryptographic attack surface is zero.

The remaining fabricating operations (`neural_engine`, `metal`) now return
`Error::Unimplemented`. Nothing in this crate will hand you a result that looks
real but is not.

For real Secure Enclave and Keychain access, use
[`security-framework`](https://crates.io/crates/security-framework).

Tracked as [**RUSTSEC-2026-0273**](https://rustsec.org/advisories/RUSTSEC-2026-0273.html)
(category: `crypto-failure`), reported in
[#3](https://github.com/paiml/manzana/issues/3). Full analysis and remediation
plan: [`docs/specifications/security-architecture-plan.md`](docs/specifications/security-architecture-plan.md).

Note that the advisory records the affected platform as `aarch64` macOS. The
defect was in fact broader: `is_available()` returned `true` unconditionally on
**x86_64** macOS as well, including pre-T2 Intel Macs with no Secure Enclave at
all.

---

## Table of Contents

- [Overview](#overview)
- [What actually works](#what-actually-works)
- [Installation](#installation)
- [Usage](#usage)
- [Examples](#examples)
- [Safety Architecture](#safety-architecture)
- [Contributing](#contributing)
- [License](#license)

## Overview

Manzana provides safe Rust interfaces to Apple hardware subsystems on macOS.
Today it is primarily a **hardware discovery** library: it can tell you what
accelerators are present. The compute and cryptographic paths are not
implemented.

## What actually works

Every row is the current state, not a roadmap. "Not implemented" means the
function returns `Error::Unimplemented` — it does not return a guess.

| Capability | Module | Status |
|---|---|---|
| Metal GPU enumeration (name, VRAM, limits) | `metal` | **Implemented** — via `system_profiler` |
| Afterburner presence + stats | `afterburner` | **Implemented** — via IOKit |
| Apple Silicon / ANE presence | `neural_engine` | **Implemented** — compile-time target check |
| Page-aligned host buffer allocation | `unified_memory` | **Implemented** — real allocation, RAII |
| Metal shader compilation, buffer allocation, dispatch | `metal` | **Not implemented** |
| CoreML model loading and inference | `neural_engine` | **Not implemented** |
| ANE capability querying (TOPS, cores) | `neural_engine` | **Not implemented** |
| GPU-shared / zero-copy buffers | `unified_memory` | **Not implemented** |

Notes on the implemented rows:

- `unified_memory::UmaBuffer` is a page-aligned **host** allocation. It is not
  a `MTLBuffer`, is not wrapped with `newBufferWithBytesNoCopy:`, and is not
  visible to a GPU. Page alignment is a prerequisite for that wrap, not the
  wrap itself.
- Metal enumeration shells out to `system_profiler`. If detection fails it
  returns an empty list rather than inventing a device.

## Supported Hardware

Detection only — see the table above for what can be done with each.

| Accelerator | Module | Mac Pro | Apple Silicon | Intel Mac |
|-------------|--------|---------|---------------|-----------|
| Afterburner FPGA | `afterburner` | ✓ | - | - |
| Neural Engine | `neural_engine` | - | ✓ | - |
| Metal GPU | `metal` | ✓ | ✓ | ✓ |
| Unified Memory | `unified_memory` | - | ✓ | - |

## Installation

```toml
[target.'cfg(target_os = "macos")'.dependencies]
manzana = "0.3"
```

### Feature Flags

```toml
[features]
default = []
afterburner = []      # Mac Pro Afterburner support
neural-engine = []    # Apple Silicon Neural Engine
metal = []            # Metal GPU compute
full = ["afterburner", "neural-engine", "metal"]
```

> **Note:** these flags are currently declared but gate nothing — no `#[cfg(feature = ...)]`
> exists in `src/`, so every module is compiled regardless of which features you
> enable. Wiring them up is tracked in the security architecture plan.

## Usage

### Hardware discovery

```rust
use manzana::{
    afterburner::AfterburnerMonitor,
    metal::MetalCompute,
    neural_engine::NeuralEngineSession,
};

fn main() {
    println!("Afterburner:   {}", AfterburnerMonitor::is_available());
    println!("Neural Engine: {}", NeuralEngineSession::is_available());
    println!("Metal GPU:     {}", MetalCompute::is_available());

    for device in MetalCompute::devices() {
        println!("GPU: {} ({:.1} GB VRAM)", device.name, device.vram_gb());
    }
}
```

### Page-aligned host buffers

```rust
use manzana::unified_memory::UmaBuffer;

fn buffers() -> manzana::Result<()> {
    let mut buffer = UmaBuffer::new(1024 * 1024)?;
    buffer.as_mut_slice()[0] = 42;
    assert!(buffer.is_aligned());
    Ok(())
}
```

### Handling unimplemented operations

Operations that cannot reach real hardware fail rather than guess. Callers can
detect this specifically:

```rust
use manzana::neural_engine::NeuralEngineSession;
use std::path::Path;

let err = NeuralEngineSession::load(Path::new("model.mlmodelc"))
    .expect_err("CoreML model loading is not implemented");
assert!(err.is_unimplemented());
```

## Examples

```bash
cargo run --example hardware_discovery   # Discover available Apple hardware
cargo run --example metal_compute        # Enumerate Metal devices
```

## Safety Architecture

The public API is safe Rust. `#![deny(unsafe_code)]` is set at the crate root,
with overrides in two places:

```
src/ffi/iokit.rs        Real FFI. extern "C" bindings to IOServiceMatching,
                        IORegistryEntryCreateCFProperties and friends, each
                        unsafe block carrying a // SAFETY: justification.
                        This is what backs Afterburner detection and stats.
src/unified_memory.rs   Carries its own #![allow(unsafe_code)] for the
                        page-aligned allocation and its RAII Drop.
```

Unsafe code is therefore **not** confined to `src/ffi/`, contrary to earlier
versions of this document. Earlier versions also described the FFI layer as
"MIRI-verified"; the `make miri` target behind that claim suppressed its own
failures and could not fail. Both the target and the claim have been corrected.

## Quality

| Metric | Value |
|--------|-------|
| Tests | 126 on Linux; more on macOS, where platform-gated tests also run |
| Clippy | 0 warnings (pedantic + nursery, `--all-targets`) |
| Unsafe code | `src/ffi/iokit.rs` (real IOKit FFI) and `src/unified_memory.rs` |

Test counts are platform-dependent because some tests are gated on
`target_os`. The security-critical assertions are deliberately **ungated** —
gating the entire cryptographic surface behind `target_os = "macos"` is what
allowed the stubbed implementations to ship with a green Linux CI lane.

## Part of the Sovereign AI Stack

Manzana is part of the Sovereign AI stack orchestrated by
[aprender](https://github.com/paiml/aprender). Orchestration formerly lived in
the separate `batuta` repository, which has been archived and merged into the
aprender monorepo as `crates/aprender-orchestrate`.

## Contributing

Contributions are welcome. Please open an issue or pull request on
[GitHub](https://github.com/paiml/manzana).

1. Fork the repository
2. Create your feature branch
3. Run `make tier2` to validate
4. Submit a pull request

**A function must never return a fabricated value.** If an operation cannot
reach the hardware it claims to use, it returns `Error::Unimplemented`. A
plausible-looking placeholder is worse than an error, because a caller cannot
tell it apart from a real result.

## Security

To report a security issue, open an issue on
[GitHub](https://github.com/paiml/manzana/issues) or contact the maintainers.

### Acknowledgements

The stubbed-cryptography defect was reported by
[@ZephyrCodesStuff](https://github.com/ZephyrCodesStuff) in
[issue #3](https://github.com/paiml/manzana/issues/3), and corroborated by
[@djc](https://github.com/djc). Both were right, and the crate is better for
it. RUSTSEC-2026-0273 was filed on the strength of that report.

See [CHANGELOG.md](CHANGELOG.md) for the full record of what was wrong and
what changed.

## License

MIT License - see [LICENSE](LICENSE) for details.

## References

- [Apple Afterburner](https://support.apple.com/en-us/HT210918)
- [Apple Neural Engine](https://machinelearning.apple.com/research/neural-engine-transformers)
- [Metal Framework](https://developer.apple.com/metal/)
- [Secure Enclave](https://support.apple.com/guide/security/secure-enclave-sec59b0b31ff/web)
- [`security-framework` crate](https://crates.io/crates/security-framework) — for real Secure Enclave access
