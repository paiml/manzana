#!/usr/bin/env bats
# MZNQ-004 -- fixture harness for the hardware-reachability gate.
#
# POSITIVE CONTROL FIRST. If a deliberately-broken input passes, the harness's
# GREEN is a file count, not a verdict.

GATE="scripts/check_hardware_reachability.sh"

@test "positive control: an empty module set is refused, not passed" {
  run env FIXTURE_DIR="$(mktemp -d)" bash "$GATE"
  [ "$status" -eq 2 ]                       # harness failure, NOT 0
  [[ "$output" == *"no .rs files"* ]]
}

@test "positive control: a missing charter is refused, not passed" {
  run env CHARTER=/nonexistent/charter.toml bash "$GATE"
  [ "$status" -eq 2 ]
}

@test "fixture 1: the shipped 0.2.0 sign() body is RED" {
  run env FIXTURE_DIR=tests/fixtures/quorum/red_fabricating bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"VIOLATION"* ]]
}

@test "fixture 13 (discrimination): delegating to security-framework is GREEN" {
  run env FIXTURE_DIR=tests/fixtures/quorum/green_delegating bash "$GATE"
  [ "$status" -eq 0 ]
}

@test "fixture 14 (discrimination): returning Err(Unimplemented) is GREEN" {
  run env FIXTURE_DIR=tests/fixtures/quorum/green_refusing bash "$GATE"
  [ "$status" -eq 0 ]
}

@test "the real crate is clean" {
  run bash "$GATE"
  [ "$status" -eq 0 ]
}

@test "NAME COLLISION: a fabricator sharing a name with a real fn is RED" {
  # The gate SHIPPED GREEN on this. Its reach table was keyed on the bare
  # function NAME, so a fabricator inherited the verdict of any same-named
  # function elsewhere in the module set. Every other fixture directory holds
  # exactly ONE file, and that isolation is what hid it -- a fixture cannot
  # exercise a collision it cannot have.
  run env FIXTURE_DIR=tests/fixtures/quorum/red_name_collision bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"b_capability_lie.rs"* ]]
}

@test "backtest: the corrected gate catches 0.2.0 sign, verify, delete AND is_available" {
  # The name-keyed gate reported 11 violations on the published 0.2.0 but
  # marked every is_available "reaches-boundary", missing the headline
  # RUSTSEC-2026-0273 capability lie.
  R=evidence/quorum/mznq-002-backtest/published-0.2.0.json
  [ -f "$R" ]
  run python3 tests/fixtures/quorum/assert_backtest.py "$R"
  [ "$status" -eq 0 ]
}

@test "fixture 2: a capability asserted as a constant is RED" {
  run env FIXTURE_DIR=tests/fixtures/quorum/red_capability_lie bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"capability"* ]]
}

@test "degenerate: .rs with no functions is refused with its own diagnostic" {
  run env FIXTURE_DIR=tests/fixtures/quorum/degenerate_no_functions bash "$GATE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"extracted 0 functions"* ]]
}

@test "degenerate: no PUBLIC functions is refused with its own diagnostic" {
  run env FIXTURE_DIR=tests/fixtures/quorum/degenerate_only_private bash "$GATE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"examined 0 functions"* ]]
}

@test "degenerate: an empty denominator is refused with its own diagnostic" {
  run env CHARTER=tests/fixtures/quorum/empty-denominator-charter.toml bash "$GATE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"denominator is empty"* ]]
}

@test "MZNQ-007: an unmapped module is RED (the denominator is gated)" {
  printf '//! transient probe\npub const fn probe() -> bool { false }\n' > src/zz_bats_probe.rs
  run bash "$GATE"
  rm -f src/zz_bats_probe.rs
  [ "$status" -eq 1 ]
  [[ "$output" == *"UNMAPPED"* ]]
}

@test "MZNQ-003: the gate scores 100% against its mutation set" {
  run bash scripts/mutate_reachability_gate.sh
  [ "$status" -eq 0 ]
  [[ "$output" == *"100% kill"* ]]
}

@test "MZNQ-005: extended SATD is clean on the current crate" {
  run pmat analyze satd --extended --fail-on-violation
  [ "$status" -eq 0 ]
}

@test "MZNQ-005 discrimination: extended SATD flags the 0.2.0 euphemisms" {
  # Default SATD scored this file at ZERO debt while it contained
  # "// Stub implementation - generates a fake public key".
  d="$(mktemp -d)"; mkdir -p "$d/src"
  cp tests/fixtures/quorum/satd_euphemism/secure_enclave_0_2_0.rs.fixture "$d/src/secure_enclave.rs"
  printf '[package]\nname = "satdfix"\nversion = "0.1.0"\nedition = "2021"\n' > "$d/Cargo.toml"
  run bash -c "cd '$d' && pmat analyze satd --extended 2>&1"
  rm -rf -- "$d"
  [[ "$output" == *"Found 9 SATD violations"* ]]
}

@test "gate emits a machine-readable receipt" {
  out="$(mktemp)"
  run env JSON_OUT="$out" bash "$GATE"
  [ "$status" -eq 0 ]
  grep -q '"gate":"hardware_reachability"' "$out"
  grep -q '"violations":0' "$out"
}
