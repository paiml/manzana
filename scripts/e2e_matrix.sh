#!/usr/bin/env bash
# End-to-end verification across the real hardware matrix.
#
# manzana's whole subject is Apple hardware, and the incident it is recovering
# from was hidden by a single-platform lane: 26 tests were
# #[cfg(target_os = "macos")]-gated, so on Linux they did not exist and
# `0 ignored` read as a pass. One host cannot establish this crate's claims.
#
# HOSTS (from ~/.ssh/config)
#   intel  Linux x86_64   -- hostname is "mac-server" but it runs Linux, NOT
#                            macOS. It covers the non-Apple path on x86_64.
#   mini   macOS arm64    -- Apple M4. Covers the real Apple Silicon path.
#
# NOT COVERED, stated rather than implied: Intel macOS. The original
# is_available() defect was specifically an x86_64-macOS lie, and no host here
# can exercise that combination.
#
# Exit 0 = every host passed. 1 = a host failed. 2 = harness failure.
set -uo pipefail

HOSTS="${HOSTS:-intel mini}"
REMOTE_DIR="${REMOTE_DIR:-/tmp/manzana-e2e}"
RECEIPTS="${RECEIPTS:-evidence/e2e}"
LOCAL="$(cd "$(dirname "$0")/.." && pwd)"

die() { printf 'harness failure: %s\n' "$*" >&2; exit 2; }
command -v rsync >/dev/null 2>&1 || die "rsync not found"

mkdir -p "$RECEIPTS" || die "cannot create $RECEIPTS"
# Clear receipts from previous runs. A host that fails early `continue`s before
# writing one, and the summary globs *.json -- so a stale receipt from an
# earlier green run would be reported as this run's result.
rm -f "$RECEIPTS"/*.json "$RECEIPTS"/*.census "$RECEIPTS"/*.census.err 2>/dev/null || true

overall=0
ran=0

for host in $HOSTS; do
  printf '\n\033[1m=== %s ===\033[0m\n' "$host"

  if ! timeout 20 ssh -o BatchMode=yes -o ConnectTimeout=10 "$host" true 2>/dev/null; then
    printf '  \033[0;31mUNREACHABLE\033[0m -- host did not answer\n' >&2
    printf '  A host that cannot be reached is NOT a pass. Recording failure.\n' >&2
    overall=1
    continue
  fi

  # Identify the target honestly: what we assert depends on what this is.
  info=$(ssh -o BatchMode=yes "$host" 'uname -s; uname -m; (sw_vers -productVersion 2>/dev/null || echo "-"); (cargo --version 2>/dev/null || echo "no-cargo")')
  os=$(printf '%s' "$info"   | sed -n 1p)
  arch=$(printf '%s' "$info" | sed -n 2p)
  osver=$(printf '%s' "$info"| sed -n 3p)
  cargov=$(printf '%s' "$info"| sed -n 4p)
  printf '  target : %s %s (%s)\n' "$os" "$arch" "$osver"
  printf '  cargo  : %s\n' "$cargov"

  case "$cargov" in
    no-cargo) printf '  \033[0;31mno cargo on host\033[0m\n' >&2; overall=1; continue ;;
    *) : ;;
  esac

  # Ship the source. Exclude build output and local state so the remote build
  # is from source, not from a copied target dir.
  # `.cargo/` is EXCLUDED and then removed on the remote. `cargo llvm-cov`
  # writes a transient .cargo/config.toml pinning target-dir to a local
  # coverage path (the file itself says "DO NOT COMMIT"). Shipping it to
  # another host points cargo at a directory that does not exist there --
  # observed as "failed to create directory /mnt/nvme-raid0/coverage:
  # Read-only file system" on mini, which `2>/dev/null` then hid, leaving an
  # empty census that looked like a result. Host-specific cargo config must
  # not travel.
  ssh -o BatchMode=yes "$host" "rm -rf $REMOTE_DIR/.cargo" 2>/dev/null || true
  rsync -az --delete \
    --exclude 'target/' --exclude '.git/' --exclude 'mutants.out/' \
    --exclude '.pmat/' --exclude '.pv/' --exclude 'evidence/' --exclude '.cargo/' \
    "$LOCAL/" "$host:$REMOTE_DIR/" >/dev/null 2>&1 \
    || { printf '  \033[0;31mrsync failed\033[0m\n' >&2; overall=1; continue; }

  log="$RECEIPTS/$host.log"
  # --all-features so nothing hides behind a disabled flag.
  ssh -o BatchMode=yes "$host" \
    "cd $REMOTE_DIR && cargo test --all-features --test e2e -- --nocapture 2>&1" \
    > "$log" 2>&1
  rc=$?
  ran=$((ran + 1))

  passed=$(grep -oE '[0-9]+ passed' "$log" | head -1 | grep -oE '[0-9]+' || echo 0)
  failed=$(grep -oE '[0-9]+ failed' "$log" | head -1 | grep -oE '[0-9]+' || echo 0)

  # --- H2 execution census (MZNQ-007) ------------------------------------
  # The direct instrument for the incident's root cause. 26 of 163 tests were
  # item-level cfg(target_os="macos")-gated, so on Linux they did NOT EXIST --
  # the security surface had a denominator of zero on the only lane that ran,
  # and `0 ignored` was indistinguishable from a full pass.
  #
  # A true line-coverage ratio would need llvm-cov on every host; `mini` is a
  # cowork box and that tooling is not installed there. Counting the tests the
  # harness LISTS per platform measures the same thing without it: a test that
  # does not exist on a platform cannot be counted there.
  census="$RECEIPTS/$host.census"
  ssh -o BatchMode=yes "$host" \
    "cd $REMOTE_DIR && cargo test --all-features -- --list 2>/dev/null | grep ': test$' | sed 's/: test$//'" \
    2>"$RECEIPTS/$host.census.err" | LC_ALL=C sort -u > "$census"
  # Sorted LOCALLY under LC_ALL=C, not on the remote host. `sort` collation is
  # locale-dependent, and the two hosts disagreed about where `_` orders --
  # which made `comm` report ~21 spurious "macOS-only" tests, including things
  # like test_version_not_empty that plainly exist on both. comm requires its
  # inputs sorted under the SAME collation or its output is meaningless.
  # `grep -c` prints a count AND exits 1 on zero matches, so `|| echo 0`
  # appended a SECOND line: total_tests became the string "0\n0", the
  # integer test below errored, and the empty-denominator guard never fired.
  total_tests=$( { grep -c . "$census" 2>/dev/null || true; } | head -1 )
  total_tests=${total_tests:-0}
  ignored=$(grep -oE '[0-9]+ ignored' "$log" | head -1 | grep -oE '[0-9]+' || echo 0)
  printf '  census : %s test(s) exist here, %s ignored\n' "$total_tests" "$ignored"

  if [ "${total_tests:-0}" -eq 0 ]; then
    printf '  \033[0;31m0 tests exist on this host\033[0m -- an empty denominator IS the defect\n' >&2
    # Show WHY rather than leaving an empty result to be interpreted.
    [ -s "$RECEIPTS/$host.census.err" ] && sed 's/^/    /' "$RECEIPTS/$host.census.err" | head -6 >&2
    overall=1
  fi
  for m in neural_engine metal unified_memory afterburner error; do
    n=$( { grep -c "^${m}::" "$census" 2>/dev/null || true; } | head -1 )
    n=${n:-0}
    if [ "${n:-0}" -eq 0 ]; then
      printf '  \033[0;31mEMPTY DENOMINATOR\033[0m %s has 0 tests on %s\n' "$m" "$host" >&2
      overall=1
    fi
  done


  if [ "$rc" -eq 0 ]; then
    printf '  e2e    : \033[0;32m%s passed\033[0m, %s failed\n' "$passed" "$failed"
  else
    printf '  e2e    : \033[0;31mFAILED\033[0m (rc=%s, %s passed, %s failed)\n' "$rc" "$passed" "$failed" >&2
    grep -E "^test .* FAILED|panicked at|assertion" "$log" | head -8 | sed 's/^/    /' >&2
    overall=1
  fi

  # A run reporting zero tests is not a pass -- that is the empty-denominator
  # failure this crate exists to stop.
  if [ "${passed:-0}" -eq 0 ]; then
    printf '  \033[0;31m0 tests ran\033[0m -- an empty run is not a pass\n' >&2
    overall=1
  fi

  {
    printf '{"host":"%s","os":"%s","arch":"%s","os_version":"%s",' "$host" "$os" "$arch" "$osver"
    printf '"cargo":"%s","rc":%d,"e2e_passed":%s,"e2e_failed":%s,' "$cargov" "$rc" "${passed:-0}" "${failed:-0}"
    printf '"tests_existing":%s,"ignored":%s}\n' "${total_tests:-0}" "${ignored:-0}"
  } > "$RECEIPTS/$host.json"
done

printf '\n%s\n' "-------------------------------------------------------------"
[ "$ran" -gt 0 ] || die "no host was exercised -- refusing to report that as a pass"

printf 'hosts exercised: %d\n' "$ran"
for j in "$RECEIPTS"/*.json; do [ -e "$j" ] && sed 's/^/  /' "$j"; done

if [ "$overall" -ne 0 ]; then
  printf '\n\033[0;31mE2E MATRIX FAILED\033[0m\n' >&2
  exit 1
fi
printf '\n\033[0;32mE2E MATRIX PASSED\033[0m on %d host(s)\n' "$ran"
# Cross-host census delta. A large gap means much of the surface is exercised
# on only one platform -- exactly the shape the incident had.
if [ -s "$RECEIPTS/intel.census" ] && [ -s "$RECEIPTS/mini.census" ]; then
  only_mac=$( { LC_ALL=C comm -13 "$RECEIPTS/intel.census" "$RECEIPTS/mini.census" | grep -c . || true; } | head -1 )
  only_lin=$( { LC_ALL=C comm -23 "$RECEIPTS/intel.census" "$RECEIPTS/mini.census" | grep -c . || true; } | head -1 )
  both=$(     { LC_ALL=C comm -12 "$RECEIPTS/intel.census" "$RECEIPTS/mini.census" | grep -c . || true; } | head -1 )
  printf '\ncensus delta: %s on both, %s macOS-only, %s Linux-only\n' "$both" "$only_mac" "$only_lin"
  if [ "${only_mac:-0}" -gt 0 ]; then
    printf 'macOS-only tests (absent from the Linux lane):\n'
    LC_ALL=C comm -13 "$RECEIPTS/intel.census" "$RECEIPTS/mini.census" | sed 's/^/  /'
  fi
fi

printf 'Not covered: Intel macOS (x86_64 + Darwin). No host in this matrix has it.\n'
