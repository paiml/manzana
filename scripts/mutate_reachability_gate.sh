#!/usr/bin/env bash
# MZNQ-003 -- mutation set for the hardware-reachability gate.
#
# The gate is the thing every reachability verdict rests on. A gate that cannot
# fail is worth exactly nothing, and "the fixtures pass" does not establish that
# it can -- a gate hardcoded to exit 0 also passes every GREEN fixture.
#
# METHOD
#   Mechanically break one validation branch at a time, then re-run the full
#   fixture suite against the mutant. A mutant is KILLED if the suite notices.
#   A SURVIVING mutant names a branch no fixture constrains.
#
#   Target: 100% kill. This is a one, not a ratchet (spec section 6).
#
# Exit 0 = all mutants killed. Exit 1 = a mutant survived. Exit 2 = harness fault.
set -euo pipefail

GATE="scripts/check_hardware_reachability.sh"
FIX="tests/fixtures/quorum"

die() { printf 'harness failure: %s\n' "$*" >&2; exit 2; }
[ -f "$GATE" ] || die "gate not found at $GATE"
[ -d "$FIX" ]  || die "fixtures not found at $FIX"

WORK="$(mktemp -d)" || die "cannot create work directory"
cleanup() {
  [ -n "${WORK:-}" ] || return 0
  [ -d "$WORK" ] || return 0
  [ -e "$WORK/mutant.sh" ] && rm -f -- "$WORK/mutant.sh"
  rmdir -- "$WORK" 2>/dev/null || true
}
trap cleanup EXIT

# The fixture suite, as an oracle. Returns 0 when EVERY expectation holds.
# A mutant is killed when this returns non-zero.
suite() {
  local g="$1" e
  # fixture 1 -- the shipped 0.2.0 sign() body -- must be RED (exit 1)
  set +e; FIXTURE_DIR="$FIX/red_fabricating" bash "$g" >/dev/null 2>&1; e=$?; set -e
  [ "$e" -eq 1 ] || return 1
  # fixture 13 -- delegating to security-framework -- must be GREEN
  set +e; FIXTURE_DIR="$FIX/green_delegating" bash "$g" >/dev/null 2>&1; e=$?; set -e
  [ "$e" -eq 0 ] || return 1
  # fixture 14 -- returning Err(Unimplemented) -- must be GREEN
  set +e; FIXTURE_DIR="$FIX/green_refusing" bash "$g" >/dev/null 2>&1; e=$?; set -e
  [ "$e" -eq 0 ] || return 1
  # fixture 2 -- a capability asserted as a constant -- must be RED
  set +e; FIXTURE_DIR="$FIX/red_capability_lie" bash "$g" >/dev/null 2>&1; e=$?; set -e
  [ "$e" -eq 1 ] || return 1
  # Positive controls. Each asserts the exit code AND the specific diagnostic.
  #
  # Checking only the exit code made these equivalent mutants: the guards are
  # layered, so deleting one lets the next produce exit 2 with a DIFFERENT
  # message, which an exit-code-only oracle cannot tell apart. The guards do
  # differ in what they tell an operator, so the oracle asserts that.
  local empty out; empty="$(mktemp -d)"
  set +e; out="$(FIXTURE_DIR="$empty" bash "$g" 2>&1)"; e=$?; set -e
  rmdir -- "$empty" 2>/dev/null || true
  [ "$e" -eq 2 ] || return 1
  case "$out" in *"no .rs files under"*) : ;; *) return 1 ;; esac

  # .rs present but NO functions -- isolates the extraction guard.
  set +e; out="$(FIXTURE_DIR="$FIX/degenerate_no_functions" bash "$g" 2>&1)"; e=$?; set -e
  [ "$e" -eq 2 ] || return 1
  case "$out" in *"extracted 0 functions"*) : ;; *) return 1 ;; esac

  # Functions present but NONE public -- isolates the vacuous-pass guard.
  set +e; out="$(FIXTURE_DIR="$FIX/degenerate_only_private" bash "$g" 2>&1)"; e=$?; set -e
  [ "$e" -eq 2 ] || return 1
  case "$out" in *"examined 0 functions"*) : ;; *) return 1 ;; esac

  # Missing charter -- isolates the charter-exists guard.
  set +e; out="$(CHARTER=/nonexistent/charter.toml bash "$g" 2>&1)"; e=$?; set -e
  [ "$e" -eq 2 ] || return 1
  case "$out" in *"charter not found"*) : ;; *) return 1 ;; esac

  # Charter parses but declares an EMPTY denominator -- isolates that guard.
  set +e; out="$(CHARTER="$FIX/empty-denominator-charter.toml" bash "$g" 2>&1)"; e=$?; set -e
  [ "$e" -eq 2 ] || return 1
  case "$out" in *"denominator is empty"*) : ;; *) return 1 ;; esac
  # the real crate must be clean
  set +e; bash "$g" >/dev/null 2>&1; e=$?; set -e
  [ "$e" -eq 0 ] || return 1

  # MZNQ-007 -- an unmapped module must be RED. The charter is H1's
  # denominator; a module nothing maps is invisible to the gate (F8).
  printf '//! transient probe\npub const fn probe() -> bool { false }\n' > src/zz_mutation_probe.rs
  set +e; out="$(bash "$g" 2>&1)"; e=$?; set -e
  rm -f src/zz_mutation_probe.rs
  [ "$e" -eq 1 ] || return 1
  case "$out" in *"UNMAPPED"*) : ;; *) return 1 ;; esac

  return 0
}

# Sanity: the UNMUTATED gate must satisfy the suite. If it does not, every
# mutant would read as "killed" and the score would be a meaningless 100%.
if ! suite "$GATE"; then
  die "the unmutated gate fails its own fixture suite; mutation scores would be meaningless"
fi

# ---------------------------------------------------------------------------
# The mutation set. Each entry: label <US> sed program.
# Every one removes or inverts a validation the gate's verdict depends on.
# ---------------------------------------------------------------------------
declare -a MUTANTS=(
  "always-clean|violations=0"
  "never-count|checked=0"
  "drop-vacuous-pass-guard|s/die \"gate examined 0 functions -- vacuous pass refused\"/: /"
  "drop-empty-fixture-guard|s/die \"fixture mode: no .rs files under \$FIXTURE_DIR\"/: /"
  "drop-charter-guard|s|\\[ -f \"\$CHARTER\" \\] .. die \"charter not found at \$CHARTER\"|: |"
  "drop-empty-denominator-guard|s/die \"charter declares no \\[hardware_modules\\].paths.*\$/: /"
  "always-refuses|refuses=1"
  "always-reaches|reaches=1"
  "ignore-result-discriminator|rr-zero"
  "drop-capability-check|cap_lie=0"
  "exit-zero-always|s/^\\[ \"\$violations\" -eq 0 \\] .. exit 1\$/: /"
  "no-extraction-guard|s/die \"extracted 0 functions from the module set -- vacuous pass refused\"/: /"
  "drop-charter-mapping|charter-mapping"
)

killed=0
survived=0
declare -a SURVIVORS=()

printf 'MZNQ-003 -- mutation set for %s\n' "$GATE"
printf '%s\n' "-------------------------------------------------------------"

for entry in "${MUTANTS[@]}"; do
  label="${entry%%|*}"
  prog="${entry#*|}"

  cp "$GATE" "$WORK/mutant.sh"

  case "$prog" in
    violations=0)
      # Force the tally to zero right before the report.
      sed -i 's/^printf .\\n.$/violations=0\nprintf "\\n"/' "$WORK/mutant.sh" ;;
    checked=0)
      sed -i 's/  checked=\$((checked + 1))/  checked=$((checked + 0))/' "$WORK/mutant.sh" ;;
    refuses=1)
      sed -i 's/^  refuses=0$/  refuses=1/' "$WORK/mutant.sh" ;;
    reaches=1)
      sed -i 's|^  reaches=\$(awk.*$|  reaches=1|' "$WORK/mutant.sh" ;;
    rr-zero)
      sed -i 's/    if \[ "\$rr" = "1" \]; then/    if [ "$rr" = "NEVER" ]; then/' "$WORK/mutant.sh" ;;
    charter-mapping)
      sed -i 's/^  if \[ "\$unmapped" -ne 0 \]; then$/  if [ "$unmapped" = "NEVER" ]; then/' "$WORK/mutant.sh" ;;
    cap_lie=0)
      sed -i 's/^  cap_lie=0$/  cap_lie=0; skip_cap=1/; s/if \[ "\$cap_lie" = "1" \] \&\& \[ "\$reaches" != "1" \]; then/if [ "$cap_lie" = "NEVER" ]; then/' "$WORK/mutant.sh" ;;
    *)
      sed -i "$prog" "$WORK/mutant.sh" ;;
  esac

  if cmp -s "$GATE" "$WORK/mutant.sh"; then
    printf '  %-32s \033[1;33mNOT APPLIED\033[0m (mutation was a no-op)\n' "$label"
    SURVIVORS+=("$label (mutation did not apply)")
    survived=$((survived + 1))
    continue
  fi

  if suite "$WORK/mutant.sh"; then
    printf '  %-32s \033[0;31mSURVIVED\033[0m\n' "$label"
    SURVIVORS+=("$label")
    survived=$((survived + 1))
  else
    printf '  %-32s \033[0;32mkilled\033[0m\n' "$label"
    killed=$((killed + 1))
  fi
done

total=$((killed + survived))
[ "$total" -gt 0 ] || die "no mutants were generated -- a 100%% score here would be vacuous"
score=$((killed * 100 / total))

printf '%s\n' "-------------------------------------------------------------"
printf 'mutants: %d   killed: %d   survived: %d   score: %d%%\n' "$total" "$killed" "$survived" "$score"

if [ "$survived" -ne 0 ]; then
  printf '\nsurviving mutants name branches no fixture constrains:\n'
  for s in "${SURVIVORS[@]}"; do printf '  - %s\n' "$s"; done
  printf '\nTarget is 100%%, unconditional. Add a fixture per survivor.\n'
  exit 1
fi

printf '\033[0;32m100%% kill\033[0m -- every validation branch is constrained by a fixture\n'
