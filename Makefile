# Manzana Makefile
# Three-Tiered Certeza Testing Methodology (Iron Lotus Framework)
#
# Tier 1: ON-SAVE (< 3s) - Rapid feedback for flow state
# Tier 2: ON-COMMIT (1-5 min) - Comprehensive pre-commit validation
# Tier 3: ON-MERGE (hours) - Exhaustive quality assurance

.PHONY: all tier1 tier2 tier3 test test-fast coverage mutation miri bench fmt lint audit clean help

# Default target
all: tier2

# =============================================================================
# TIER 1: ON-SAVE (Sub-3-second feedback)
# =============================================================================
tier1: check lint-fast test-unit
	@echo "✅ Tier 1 passed (on-save feedback)"

check:
	@echo "🔍 Running cargo check..."
	cargo check --all-targets

lint-fast:
	@echo "🔍 Running fast clippy..."
	cargo clippy --lib -- -D warnings

test-unit:
	@echo "🧪 Running unit tests..."
	cargo test --lib -- --test-threads=4

# =============================================================================
# TIER 2: ON-COMMIT (1-5 minutes)
# =============================================================================
tier2: fmt-check lint test coverage-check coverage-gate audit deny quorum
	@echo "✅ Tier 2 passed (on-commit validation)"

fmt-check:
	@echo "📝 Checking formatting..."
	cargo fmt --all --check

lint:
	@echo "🔍 Running full clippy..."
	cargo clippy --all-targets -- -D warnings -D clippy::pedantic -D clippy::nursery \
		-A clippy::module_name_repetitions \
		-A clippy::must_use_candidate

test-fast:
	@echo "🧪 Running fast tests..."
	cargo test --lib -- --test-threads=4

test:
	@echo "🧪 Running all tests..."
	cargo test --all-targets

COVERAGE_FLOOR ?= 95.0

coverage-gate:
	@echo "📊 Coverage floor ($(COVERAGE_FLOOR)% lines)..."
	@# The floor is passed into awk, so the printed message cannot drift from
	@# the comparison. A gate whose message and check disagree is how a false
	@# claim survives review.
	@cargo llvm-cov --all-features --summary-only 2>/dev/null | \
		awk -v floor=$(COVERAGE_FLOOR) '/^TOTAL/ { \
			seen=1; pct=$$10; gsub(/%/,"",pct); \
			printf "   lines: %s%% (floor %s%%)\n", pct, floor; \
			if (pct+0 < floor+0) { \
				printf "\033[0;31m   FAIL: %s%% is below the %s%% floor\033[0m\n", pct, floor; \
				exit 1 \
			} \
			printf "\033[0;32m   PASS\033[0m\n" \
		} END { if (!seen) { \
			printf "\033[0;31m   FAIL: no TOTAL row parsed -- coverage was not measured\033[0m\n"; \
			exit 1 } }'

e2e:
	@echo "🔌 End-to-end matrix (real hardware)..."
	./scripts/e2e_matrix.sh

coverage-check:
	@echo "📊 Checking coverage (target: 95%)..."
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "Installing cargo-llvm-cov..."; cargo install cargo-llvm-cov; }
	cargo llvm-cov --lib --fail-under 90

quorum:
	@echo "📜 Provable contracts (pv)..."
	@# Per-file `pv validate`, never `pv lint <FILE>`: lint passes vacuously
	@# over zero contracts, so a typo'd path would read as success.
	@for c in contracts/*.yaml; do \
		echo "   validate $$c"; pv validate "$$c" >/dev/null || exit 1; \
		echo "   audit    $$c"; pv audit "$$c" >/dev/null || exit 1; \
	done
	@# Score against the binding registry. Without --binding, Bind scores 0.00
	@# and the registry is never consulted -- a silent 0 that looks like a
	@# measurement. The registry lives outside this repo, so it is optional
	@# here but reported when present.
	@if [ -f ../provable-contracts/contracts/manzana/binding.yaml ]; then \
		pv score contracts/ --binding ../provable-contracts/contracts/manzana/binding.yaml > .pv-score.txt \
			|| { echo "pv score failed"; rm -f .pv-score.txt; exit 1; }; \
		tail -6 .pv-score.txt; rm -f .pv-score.txt; \
	else \
		echo "   NOTE: binding registry absent; Bind score not measured"; \
	fi
	@echo "   binding liveness (contract attrs are bound, not decorative)"
	cargo test --all-features contract_binding -- --exact contract_binding::test_contract_binding_is_live
	@echo "🧾 SATD euphemism detection (MZNQ-005)..."
	@# `--extended` detects the euphemisms plain SATD misses: stub, placeholder,
	@# "for now". Default mode scored the fabricating 0.2.0 build at ZERO debt
	@# over comments reading "generates a fake public key". The detector already
	@# existed; this repo had simply never enabled it.
	pmat analyze satd --extended --fail-on-violation
	@echo "🔍 Hardware-reachability gate (MZNQ-4)..."
	./scripts/check_hardware_reachability.sh
	@echo "🧬 Gate mutation set (MZNQ-003, target 100%)..."
	./scripts/mutate_reachability_gate.sh
	@if command -v bats >/dev/null 2>&1; then \
		echo "🧪 Quorum fixtures..."; bats tests/quorum.bats; \
	else \
		echo "❌ bats not installed; the fixture suite cannot be reported as passing."; \
		echo "   Install: https://github.com/bats-core/bats-core"; \
		exit 1; \
	fi

audit:
	@echo "🔒 Running security audit..."
	@command -v cargo-audit >/dev/null 2>&1 || { echo "Installing cargo-audit..."; cargo install cargo-audit; }
	cargo audit

deny:
	@echo "📋 Checking dependencies..."
	@command -v cargo-deny >/dev/null 2>&1 || { echo "Installing cargo-deny..."; cargo install cargo-deny; }
	cargo deny check

# =============================================================================
# TIER 3: ON-MERGE (Hours - exhaustive QA)
# =============================================================================
tier3: tier2 mutation miri bench doc e2e
	@echo "✅ Tier 3 passed (on-merge exhaustive QA)"

mutation:
	@echo "🧬 Running mutation testing (target: 80%)..."
	@command -v cargo-mutants >/dev/null 2>&1 || { echo "Installing cargo-mutants..."; cargo install cargo-mutants; }
	cargo mutants --timeout-multiplier 2.0 -- --lib

miri:
	@echo "🔬 Running MIRI (undefined behavior check)..."
	@rustup run nightly cargo miri --version >/dev/null 2>&1 || { \
		echo "❌ MIRI is not installed. Install it with:"; \
		echo "     rustup +nightly component add miri"; \
		echo "   This target must not be reported as passing without it."; \
		exit 1; }
	rustup run nightly cargo miri test --lib

bench:
	@echo "⏱️  Running benchmarks..."
	cargo bench --no-run

doc:
	@echo "📚 Building documentation..."
	cargo doc --no-deps --document-private-items

# =============================================================================
# Individual Commands
# =============================================================================
coverage:
	@echo "📊 Generating coverage report..."
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "Installing cargo-llvm-cov..."; cargo install cargo-llvm-cov; }
	cargo llvm-cov --lib --html
	@echo "Coverage report: target/llvm-cov/html/index.html"

coverage-report:
	@echo "📊 Full coverage report..."
	cargo llvm-cov --lib --text

fmt:
	@echo "📝 Formatting code..."
	cargo fmt --all

# Property tests with more cases
proptest:
	@echo "🎲 Running property tests (extended)..."
	PROPTEST_CASES=1000 cargo test property_tests

# Chaos testing
chaos:
	@echo "🌪️  Running chaos tests..."
	PROPTEST_CASES=5000 cargo test property_tests

clean:
	@echo "🧹 Cleaning..."
	cargo clean
	rm -rf target/llvm-cov target/criterion

# =============================================================================
# CI Integration
# =============================================================================
ci-tier1:
	@echo "🚀 CI Tier 1..."
	$(MAKE) tier1

ci-tier2:
	@echo "🚀 CI Tier 2..."
	$(MAKE) tier2

ci-tier3:
	@echo "🚀 CI Tier 3..."
	$(MAKE) tier3

# =============================================================================
# Help
# =============================================================================
help:
	@echo "Manzana Build System (Iron Lotus Framework)"
	@echo ""
	@echo "Testing Tiers:"
	@echo "  make tier1      - ON-SAVE: Fast feedback (<3s)"
	@echo "  make tier2      - ON-COMMIT: Full validation (1-5min)"
	@echo "  make tier3      - ON-MERGE: Exhaustive QA (hours)"
	@echo ""
	@echo "Individual Commands:"
	@echo "  make check      - Type check"
	@echo "  make lint       - Run clippy"
	@echo "  make test       - Run all tests"
	@echo "  make coverage   - Generate coverage report"
	@echo "  make mutation   - Run mutation testing"
	@echo "  make miri       - Run MIRI (requires nightly)"
	@echo "  make bench      - Run benchmarks"
	@echo "  make fmt        - Format code"
	@echo "  make audit      - Security audit"
	@echo "  make clean      - Clean build artifacts"
	@echo ""
	@echo "CI:"
	@echo "  make ci-tier1   - CI tier 1 checks"
	@echo "  make ci-tier2   - CI tier 2 checks"
	@echo "  make ci-tier3   - CI tier 3 checks"
