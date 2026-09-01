#!/usr/bin/env bats
#
# The pmat-dependent half of the quorum suite, split out of quorum.bats.
#
# WHY THE SPLIT, stated rather than buried: `pmat` is not installable on public
# GitHub runners, and these two tests invoke it directly. Left in quorum.bats
# they exited 127 and failed the whole `Anti-fabrication gates` job -- which
# runs the gates that must never be skipped -- for a reason that had nothing to
# do with those gates.
#
# This file runs in the `contracts` job instead, behind the same `if:` guard as
# the other pmat and pv steps, and that job already emits a ::warning:: naming
# every check that did NOT run. That is the honest place for a check whose tool
# may be absent: reported as unrun, never reported as passed.
#
# It is NOT a relaxation. Run locally, or anywhere pmat is installed, these two
# tests are exactly as binding as before, and `make quorum` runs both files.

setup() {
  cd "$BATS_TEST_DIRNAME/.." || exit 1
  command -v pmat >/dev/null 2>&1 || {
    echo "pmat is not installed; these tests cannot run and are NOT a pass" >&2
    exit 127
  }
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

