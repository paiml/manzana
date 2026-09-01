#!/usr/bin/env bash
# MZNQ-004 / G3 -- test census gate.
#
# WHY THIS EXISTS
#   26 of 163 tests carried an item-level #[cfg(target_os = "macos")]. On Linux
#   those tests do not exist -- they are not skipped, so the runner reports
#   `0 ignored`. Zero tests passing is indistinguishable from all tests
#   passing, and that is how a fabricating crate held a green CI lane.
#
#   A "minimum test count" does not fix it: the threshold is invented, and a
#   count survives delete-one/add-one. This gate asserts four things instead:
#
#     1. ignored == 0          -- nothing silently skipped
#     2. per-module count > 0  -- no security module has an empty denominator
#     3. total is a RATCHET    -- monotone non-decreasing against a baseline
#     4. NAME-SET diff         -- a removed test is named, not absorbed
#
#   (4) is what a count cannot do. Deleting a refusal test and adding a
#   trivial one keeps the total identical.
#
# Exit 0 = clean, 1 = census violation, 2 = harness failure.
set -euo pipefail

# NOT under .pmat/: .gitignore excludes `**/.pmat/`, so a baseline there is
# untracked and CI fails with "no baseline" on a clean checkout. The baseline
# is the ratchet's reference point and must be committed.
BASELINE="${BASELINE:-.quorum/test-census.txt}"
UPDATE="${UPDATE:-0}"

die() { printf 'harness failure: %s\n' "$*" >&2; exit 2; }

# Modules whose test denominator must never be zero. These are the surfaces
# where a silent skip previously read as a pass.
# secure_enclave was removed in 0.3.0 along with all cryptography. The census
# correctly flagged its empty denominator when the module vanished -- that is
# the gate working. Dropping it here is a deliberate, recorded decision, not a
# silent accommodation.
REQUIRED_MODULES="neural_engine metal unified_memory error afterburner"

WORK="$(mktemp -d)" || die "cannot create work directory"
cleanup() {
  [ -n "${WORK:-}" ] || return 0
  [ -d "$WORK" ] || return 0
  for f in names.txt out.txt; do [ -e "$WORK/$f" ] && rm -f -- "$WORK/$f"; done
  rmdir -- "$WORK" 2>/dev/null || true
}
trap cleanup EXIT

# Enumerate the tests the harness ACTUALLY has, from the harness itself rather
# than by reading source. A test the runner does not list does not exist.
cargo test --all-features -- --list > "$WORK/out.txt" 2>/dev/null \
  || die "cargo test --list failed; the census cannot be taken"

grep ': test$' "$WORK/out.txt" | sed 's/: test$//' | sort -u > "$WORK/names.txt" \
  || die "no tests parsed from the harness listing"

total=$(wc -l < "$WORK/names.txt" | tr -d ' ')
[ "$total" -gt 0 ] || die "census counted 0 tests -- refusing to report that as a pass"

# 1. Nothing ignored.
ignored=$(grep -c ': test$' "$WORK/out.txt" >/dev/null 2>&1; cargo test --all-features 2>/dev/null | grep -oP '\d+(?= ignored)' | paste -sd+ | bc)
ignored="${ignored:-0}"

# 2. Per-module denominators.
missing=""
for m in $REQUIRED_MODULES; do
  n=$(grep -c "^${m}::" "$WORK/names.txt" || true)
  [ "$n" -gt 0 ] || missing="$missing $m"
done

printf 'test census\n'
printf '  total tests   : %d\n' "$total"
printf '  ignored       : %s\n' "$ignored"
for m in $REQUIRED_MODULES; do
  printf '  %-14s: %s\n' "$m" "$(grep -c "^${m}::" "$WORK/names.txt" || true)"
done

fail=0

if [ "$ignored" != "0" ]; then
  printf '\nVIOLATION: %s test(s) ignored. A skip must be a decision, not a default.\n' "$ignored" >&2
  fail=1
fi

if [ -n "$missing" ]; then
  printf '\nVIOLATION: module(s) with an EMPTY test denominator:%s\n' "$missing" >&2
  printf '           zero tests passing reads exactly like all tests passing.\n' >&2
  fail=1
fi

# 3 + 4. Ratchet and name-set diff.
if [ "$UPDATE" = "1" ]; then
  mkdir -p "$(dirname "$BASELINE")"
  cp "$WORK/names.txt" "$BASELINE"
  printf '\nbaseline written: %s (%d tests)\n' "$BASELINE" "$total"
  exit 0
fi

if [ ! -f "$BASELINE" ]; then
  printf '\nno baseline at %s -- create it with UPDATE=1 %s\n' "$BASELINE" "$0" >&2
  exit 2
fi

base_total=$(wc -l < "$BASELINE" | tr -d ' ')
if [ "$total" -lt "$base_total" ]; then
  printf '\nVIOLATION: total fell %d -> %d. The count is a ratchet.\n' "$base_total" "$total" >&2
  fail=1
fi

removed=$(comm -23 "$BASELINE" "$WORK/names.txt" || true)
if [ -n "$removed" ]; then
  printf '\nVIOLATION: test(s) present in the baseline and now absent:\n' >&2
  printf '%s\n' "$removed" | sed 's/^/  - /' >&2
  printf '           A count cannot see this: delete one test, add another,\n' >&2
  printf '           and the total is unchanged. Removing a test is a decision;\n' >&2
  printf '           record it by updating the baseline deliberately.\n' >&2
  fail=1
fi

added=$(comm -13 "$BASELINE" "$WORK/names.txt" || true)
[ -n "$added" ] && printf '\n  new tests: %d\n' "$(printf '%s\n' "$added" | grep -c . || true)"

[ "$fail" -eq 0 ] || exit 1
printf '\n  PASS -- census intact, nothing ignored, no empty denominators\n'
