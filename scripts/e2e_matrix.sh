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
  rsync -az --delete \
    --exclude 'target/' --exclude '.git/' --exclude 'mutants.out/' \
    --exclude '.pmat/' --exclude '.pv/' --exclude 'evidence/' \
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
    printf '"cargo":"%s","rc":%d,"passed":%s,"failed":%s}\n' "$cargov" "$rc" "${passed:-0}" "${failed:-0}"
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
printf 'Not covered: Intel macOS (x86_64 + Darwin). No host in this matrix has it.\n'
