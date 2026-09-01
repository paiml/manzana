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

@test "gate emits a machine-readable receipt" {
  out="$(mktemp)"
  run env JSON_OUT="$out" bash "$GATE"
  [ "$status" -eq 0 ]
  grep -q '"gate":"hardware_reachability"' "$out"
  grep -q '"violations":0' "$out"
}
