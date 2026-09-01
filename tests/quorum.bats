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

@test "MZNQ-4: a call edge is not laundered by a bare name (Vec::new)" {
  # `AfterburnerMonitor::new` reaches IOKit, so the gate marks the NAME `new`
  # as boundary-reaching. Call edges were resolved by bare name, so every
  # `Vec::new()`, `String::new()` and `HashMap::new()` in the crate resolved to
  # it -- certifying functions that touch no hardware at all.
  #
  # Finding it also exposed a second hole it had been masking: the extractor
  # accepted only `pub ` and bare `fn `, so `pub(super) fn` was never in the
  # graph, and all of src/metal/detect.rs -- including the one function that
  # actually calls Command::new -- was invisible. Two holes cancelling out.
  run env FIXTURE_DIR=tests/fixtures/quorum/red_bare_name_call_edge bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"read_stats"* ]]
}

@test "MZNQ-4: mentioning a refusal in prose is not refusing" {
  # Both limbs are text matches. `//` comments were stripped; block comments
  # and STRING LITERALS were not, so a fabricating function that merely named
  # Error::unimplemented in either was certified as refusing.
  run env FIXTURE_DIR=tests/fixtures/quorum/red_refusal_in_prose bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"fabricates_with_symbol_in_a_string"* ]]
  [[ "$output" == *"fabricates_with_symbol_in_a_block_comment"* ]]
}

@test "MZNQ-4: a multi-line block comment satisfies neither limb" {
  # The stripper used the single-line C-comment regex, so a /* */ spanning two
  # lines survived into the body both limbs text-match against. A fabricating
  # fn that merely MENTIONED Error::unimplemented or Command::new passed.
  run env FIXTURE_DIR=tests/fixtures/quorum/red_multiline_comment bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"read_die_temperature"* ]]
  [[ "$output" == *"read_gpu_core_count"* ]]
}

@test "MZNQ-4: a return-true capability lie is caught like a bare true" {
  # The capability patterns are textual and matched "{ true }" but not
  # "{ return true; }" -- the advisory's headline defect with one keyword added.
  run env FIXTURE_DIR=tests/fixtures/quorum/red_return_true bash "$GATE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"is_smc_available"* ]]
  [[ "$output" == *"without probing anything"* ]]
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

@test "gate emits a machine-readable receipt" {
  out="$(mktemp)"
  run env JSON_OUT="$out" bash "$GATE"
  [ "$status" -eq 0 ]
  grep -q '"gate":"hardware_reachability"' "$out"
  grep -q '"violations":0' "$out"
}
