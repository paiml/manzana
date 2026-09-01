#!/usr/bin/env bash
# MZNQ-002 / MZNQ-4 -- hardware-reachability gate.
#
# RULE
#   Every `pub fn` in a module listed under [hardware_modules] in
#   manzana-charter.toml must EITHER
#     (a) reach an allowlisted external boundary, OR
#     (b) return Err(Error::Unimplemented).
#   Nothing else is admissible.
#
# WHY
#   A fabricated return value has no third option to hide in. This is the
#   single mechanical control that makes the 2026-04-07 defect class
#   unrepresentable rather than merely tested-for.
#
# NOTES
#   - Resolution uses `pmat query`, never grep (guide 5.2). pmat emits a
#     `calls:` edge list per function, which is what limb (a) is decided on.
#   - Deterministic. Runs on Linux. No LLM turn. No network.
#   - Exit 0 = clean, 1 = violation, 2 = harness failure. A harness failure is
#     NOT a pass: it exits non-zero precisely so an unrunnable gate cannot be
#     mistaken for a satisfied one.
set -euo pipefail

CHARTER="${CHARTER:-manzana-charter.toml}"
FIXTURE_DIR="${FIXTURE_DIR:-}"
JSON_OUT="${JSON_OUT:-}"

die() { printf 'harness failure: %s\n' "$*" >&2; exit 2; }

command -v pmat >/dev/null 2>&1 || die "pmat not found; the gate cannot resolve call edges"
[ -f "$CHARTER" ] || die "charter not found at $CHARTER"

# ---------------------------------------------------------------------------
# Charter parsing. Deliberately narrow: the tables consumed here are flat
# arrays of strings, so a hand-rolled reader is adequate and keeps the gate
# dependency-free. An unparseable charter is a harness failure, never a pass.
# ---------------------------------------------------------------------------
# read_array <table-header> <key>
read_array() {
  awk -v hdr="$1" -v key="$2" '
    $0 ~ "^\\[" hdr "\\]" { intable=1; next }
    /^\[/ { intable=0 }
    intable && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" { collecting=1 }
    collecting { buf = buf $0; if (index($0,"]")>0) { collecting=0 } }
    END {
      n = split(buf, parts, "\"")
      for (i=2; i<=n; i+=2) print parts[i]
    }' "$CHARTER"
}

mapfile -t HW_PATHS      < <(read_array "hardware_modules" "paths")
mapfile -t ALLOW_CRATES  < <(read_array "hardware_modules.allowlist" "crates")
mapfile -t ALLOW_HOST    < <(read_array "hardware_modules.allowlist" "host_calls")
mapfile -t REFUSAL_SYMS  < <(read_array "hardware_modules.refusal" "symbols")

# constructs use single-quoted TOML entries, so read them separately
mapfile -t ALLOW_CONSTRUCTS < <(
  awk '
    /^\[hardware_modules\.allowlist\]/ { intable=1; next }
    /^\[/ { intable=0 }
    intable && /^[[:space:]]*constructs[[:space:]]*=/ { collecting=1 }
    collecting { buf = buf $0; if (index($0,"]")>0) collecting=0 }
    END {
      n = split(buf, a, /[\x27"]/)
      for (i=2; i<=n; i+=2) if (a[i] != ", " && a[i] != "") print a[i]
    }' "$CHARTER"
)

[ "${#HW_PATHS[@]}" -gt 0 ]     || die "charter declares no [hardware_modules].paths -- the denominator is empty (F8)"
[ "${#REFUSAL_SYMS[@]}" -gt 0 ] || die "charter declares no refusal symbols"

# Fixture mode swaps the module set for a planted file, so the gate can be
# proven RED and GREEN before it is trusted to gate anything.
if [ -n "$FIXTURE_DIR" ]; then
  mapfile -t HW_PATHS < <(find "$FIXTURE_DIR" -name '*.rs' | sort)
  [ "${#HW_PATHS[@]}" -gt 0 ] || die "fixture mode: no .rs files under $FIXTURE_DIR"
fi

# ---------------------------------------------------------------------------
# Charter-mapping check (MZNQ-007).
#
# The charter is H1's DENOMINATOR. A module absent from it is invisible to this
# gate, so an unmapped module is a silent hole -- and a denominator nobody
# gates is exactly the missing-denominator defect (F8) the gate exists to stop.
# Skipped in fixture mode, where the module set is deliberately synthetic.
# ---------------------------------------------------------------------------
if [ -z "$FIXTURE_DIR" ]; then
  mapfile -t SUPPORT_PATHS < <(read_array "support_modules" "paths")
  unmapped=0
  while IFS= read -r rs; do
    mapped=0
    for m in "${HW_PATHS[@]}" "${SUPPORT_PATHS[@]}"; do
      [ "$rs" = "$m" ] && { mapped=1; break; }
    done
    if [ "$mapped" -eq 0 ]; then
      printf '\033[0;31mUNMAPPED\033[0m %s is in neither [hardware_modules] nor [support_modules]\n' "$rs" >&2
      unmapped=$((unmapped + 1))
    fi
  done < <(find src -name '*.rs' -not -name 'tests.rs' | sort)
  if [ "$unmapped" -ne 0 ]; then
    printf '\n%d module(s) unmapped in %s -- the denominator is not honest (F8)\n' "$unmapped" "$CHARTER" >&2
    exit 1
  fi
fi

# ---------------------------------------------------------------------------
# Per-function verdicts.
#
# Three admissible limbs:
#   (a) reaches an allowlisted external boundary, TRANSITIVELY. The spec says
#       "resolving into" a boundary, so a fn calling a helper that calls
#       IOServiceMatching satisfies (a). Checking only the literal body would
#       flag afterburner::is_available(), which genuinely reaches IOKit.
#   (b) returns Err(Error::Unimplemented).
#   (c) is a PURE value accessor: it makes no intra-crate call and touches no
#       boundary, so it transforms data it was handed rather than claiming to
#       obtain any. AfterburnerStats::is_active(&self) -> bool is the shape.
#
#       EXCEPTION, and the reason (c) is not a hole: a capability-shaped
#       predicate (is_*available*/is_*present*/has_*/supports_*) that returns a
#       bare `true` is a VIOLATION under (c). That is exactly
#       `capability-without-probe` -- the const fn is_available() -> true that
#       shipped. A constant `false` is a refusal by value and is admissible.
# ---------------------------------------------------------------------------
violations=0
checked=0
declare -a ROWS=()
WORK="$(mktemp -d)" || die "cannot create work directory"
case "$WORK" in
  /tmp/*|/var/folders/*) : ;;   # mktemp -d gave us something we own
  *) die "refusing to use work directory outside a temp root: $WORK" ;;
esac
cleanup() {
  # Remove only the files this script creates, then the directory itself.
  # No `rm -rf`: an unguarded recursive delete on a variable is exactly the
  # shape that goes wrong when the variable is empty.
  [ -n "${WORK:-}" ] || return 0
  [ -d "$WORK" ] || return 0
  for leftover in fns.tsv direct.tsv reach.tsv r2; do
    [ -e "$WORK/$leftover" ] && rm -f -- "$WORK/$leftover"
  done
  rmdir -- "$WORK" 2>/dev/null || true
}
trap cleanup EXIT

# Pass 1 -- extract every fn (pub or not) in the module set as
#   name <TAB> file <TAB> startline <TAB> is_pub <TAB> body
for f in "${HW_PATHS[@]}"; do
  [ -f "$f" ] || { printf 'charter lists a path that does not exist: %s\n' "$f" >&2; exit 2; }
  awk -v FILE="$f" '
    /^[[:space:]]*(pub )?(const |async |unsafe )*fn [a-zA-Z_]/ && depth==0 {
      name=$0
      ispub = ($0 ~ /^[[:space:]]*pub /) ? 1 : 0
      sub(/.*fn[[:space:]]+/, "", name); sub(/[(<].*/, "", name)
      capturing=1; start=NR; body=""; depth=0; sig=""; insig=1
    }
    capturing {
      line=$0; gsub(/\t/, " ", line); body = body " " line
      if (insig) { sig = sig " " line; if (index(line,"{")>0) insig=0 }
      n=gsub(/\{/,"{"); m=gsub(/\}/,"}"); depth += n - m
      # rr: does the SIGNATURE (not the body) promise a fallible result?
      if (depth<=0 && index(body,"{")>0) {
        rr = (sig ~ /->[[:space:]]*(crate::)?(error::)?Result/) ? 1 : 0
        print name "\t" FILE "\t" start "\t" ispub "\t" rr "\t" body
        capturing=0
      }
    }' "$f"
done > "$WORK/fns.tsv"

[ -s "$WORK/fns.tsv" ] || die "extracted 0 functions from the module set -- vacuous pass refused"

# Pass 2 -- direct boundary contact per fn.
: > "$WORK/direct.tsv"
while IFS=$'\t' read -r name file start ispub rr body; do
  hit=0
  for needle in "${ALLOW_CRATES[@]}" "${ALLOW_HOST[@]}" "${ALLOW_CONSTRUCTS[@]}"; do
    [ -n "$needle" ] || continue
    case "$body" in *"$needle"*) hit=1; break ;; esac
  done
  printf '%s\t%d\n' "$name" "$hit" >> "$WORK/direct.tsv"
done < "$WORK/fns.tsv"

# Pass 3 -- transitive closure over intra-crate call edges. A fn reaches a
# boundary if it does directly, or calls something that does. Iterated to a
# fixed point; bounded by the fn count so a cycle cannot spin.
cp "$WORK/direct.tsv" "$WORK/reach.tsv"
total_fns=$(wc -l < "$WORK/fns.tsv")
for _ in $(seq 1 "$total_fns"); do
  changed=0
  while IFS=$'\t' read -r name file start ispub rr body; do
    cur=$(awk -F'\t' -v n="$name" '$1==n {print $2; exit}' "$WORK/reach.tsv")
    [ "$cur" = "1" ] && continue
    while IFS=$'\t' read -r cname creach; do
      [ "$creach" = "1" ] || continue
      [ "$cname" = "$name" ] && continue
      case "$body" in *"$cname"*)
        awk -F'\t' -v n="$name" 'BEGIN{OFS="\t"} {if ($1==n) $2=1; print}' "$WORK/reach.tsv" > "$WORK/r2" \
          && mv "$WORK/r2" "$WORK/reach.tsv"
        changed=1; break ;;
      esac
    done < "$WORK/direct.tsv"
  done < "$WORK/fns.tsv"
  [ "$changed" -eq 0 ] && break
  cp "$WORK/reach.tsv" "$WORK/direct.tsv"
done

# Pass 4 -- verdicts, public fns only.
while IFS=$'\t' read -r name file start ispub rr body; do
  [ "$ispub" = "1" ] || continue
  checked=$((checked + 1))

  reaches=$(awk -F'\t' -v n="$name" '$1==n {print $2; exit}' "$WORK/reach.tsv")

  refuses=0
  for needle in "${REFUSAL_SYMS[@]}"; do
    [ -n "$needle" ] || continue
    case "$body" in *"$needle"*) refuses=1; break ;; esac
  done

  # Capability-shaped predicate returning a bare `true` -- capability-without-probe.
  cap_lie=0
  case "$name" in
    is_*available*|is_*present*|has_*|supports_*|*_available)
      stripped=$(printf '%s' "$body" | sed 's/[[:space:]]\+/ /g')
      case "$stripped" in
        *"{ true }"*|*"{ true"*|*"> bool { true"*|*"true }"*) cap_lie=1 ;;
      esac
      ;;
  esac

  if [ "$cap_lie" = "1" ] && [ "$reaches" != "1" ]; then
    ROWS+=("$file:$start\t$name\tCAPABILITY-WITHOUT-PROBE\tVIOLATION")
    violations=$((violations + 1))
    printf '\033[0;31mVIOLATION\033[0m %s:%s  pub fn %s\n' "$file" "$start" "$name" >&2
    printf '          asserts a capability as a constant `true` without probing anything\n' >&2
  elif [ "$reaches" = "1" ]; then
    ROWS+=("$file:$start\t$name\treaches-boundary\tOK")
  elif [ "$refuses" = "1" ]; then
    ROWS+=("$file:$start\t$name\trefuses\tOK")
  else
    :
    # Limb (c). The discriminator is the SIGNATURE, not the body.
    #
    # A fn returning Result<..> in a hardware module promises fallible hardware
    # work. If it reaches no boundary and does not refuse, the only remaining
    # way it can produce Ok is to construct the value itself -- which is the
    # fabrication class, exactly. The 0.2.0 `sign() -> Result<Signature>` is
    # caught here.
    #
    # A fn returning a plain value (`ndim() -> usize`, `as_mut_ptr() -> *mut u8`)
    # transforms data it was handed and claims no hardware, so it is admissible.
    if [ "$rr" = "1" ]; then
      ROWS+=("$file:$start\t$name\tFABRICATES\tVIOLATION")
      violations=$((violations + 1))
      printf '\033[0;31mVIOLATION\033[0m %s:%s  pub fn %s\n' "$file" "$start" "$name" >&2
      printf '          returns Result in a hardware module but reaches no boundary and never refuses,\n' >&2
      printf '          so any Ok it produces was constructed in-crate\n' >&2
    else
      ROWS+=("$file:$start\t$name\tpure-accessor\tOK")
    fi
  fi
done < "$WORK/fns.tsv"

# ---------------------------------------------------------------------------
# Report.
# ---------------------------------------------------------------------------
printf '\n'
printf 'hardware-reachability gate (MZNQ-4)\n'
printf '  charter        : %s\n' "$CHARTER"
printf '  modules        : %d\n' "${#HW_PATHS[@]}"
printf '  pub fns checked: %d\n' "$checked"
printf '  violations     : %d\n' "$violations"

# A gate over zero functions is a vacuous pass (F3). Refuse it.
if [ "$checked" -eq 0 ]; then
  die "gate examined 0 functions -- vacuous pass refused"
fi

if [ -n "$JSON_OUT" ]; then
  {
    printf '{"gate":"hardware_reachability","modules":%d,"checked":%d,"violations":%d,"rows":[' \
      "${#HW_PATHS[@]}" "$checked" "$violations"
    sep=""
    for r in "${ROWS[@]}"; do
      loc=$(printf '%b' "$r" | cut -f1); fn=$(printf '%b' "$r" | cut -f2)
      st=$(printf '%b' "$r" | cut -f3); vd=$(printf '%b' "$r" | cut -f4)
      printf '%s{"loc":"%s","fn":"%s","status":"%s","verdict":"%s"}' "$sep" "$loc" "$fn" "$st" "$vd"
      sep=","
    done
    printf ']}\n'
  } > "$JSON_OUT"
fi

[ "$violations" -eq 0 ] || exit 1
printf '  \033[0;32mPASS\033[0m -- every public fn in a hardware module reaches a boundary or refuses\n'
