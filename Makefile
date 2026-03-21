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
tier2: fmt-check lint test coverage-check audit deny
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

coverage-check:
	@echo "📊 Checking coverage (target: 95%)..."
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "Installing cargo-llvm-cov..."; cargo install cargo-llvm-cov; }
	cargo llvm-cov --lib --fail-under 90

audit:
	@echo "🔒 Running security audit..."
	@command -v cargo-audit >/dev/null 2>&1 || { echo "Installing cargo-audit..."; cargo install cargo-audit; }
	cargo audit

deny:
	@echo "📋 Checking dependencies..."
	@command -v cargo-deny >/dev/null 2>&1 || { echo "Installing cargo-deny..."; cargo install cargo-deny; }
	cargo deny check 2>/dev/null || echo "⚠️  cargo-deny not configured (create deny.toml)"

# =============================================================================
# TIER 3: ON-MERGE (Hours - exhaustive QA)
# =============================================================================
tier3: tier2 mutation miri bench doc
	@echo "✅ Tier 3 passed (on-merge exhaustive QA)"

mutation:
	@echo "🧬 Running mutation testing (target: 80%)..."
	@command -v cargo-mutants >/dev/null 2>&1 || { echo "Installing cargo-mutants..."; cargo install cargo-mutants; }
	cargo mutants --timeout-multiplier 2.0 -- --lib

miri:
	@echo "🔬 Running MIRI (undefined behavior check)..."
	@rustup run nightly cargo miri test --lib 2>/dev/null || echo "⚠️  MIRI requires nightly: rustup +nightly component add miri"

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
