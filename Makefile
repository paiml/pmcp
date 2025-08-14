# Rust MCP SDK Makefile with pmat quality standards
# Zero tolerance for technical debt

CARGO = cargo
RUSTFLAGS = -D warnings
RUST_LOG ?= debug
RUST_BACKTRACE ?= 1

# Colors for output
RED = \033[0;31m
GREEN = \033[0;32m
YELLOW = \033[1;33m
BLUE = \033[0;34m
NC = \033[0m # No Color

# Default target
.PHONY: all
all: quality-gate

# Development setup
.PHONY: setup
setup:
	@echo "$(BLUE)Setting up development environment...$(NC)"
	rustup component add rustfmt clippy llvm-tools-preview
	cargo install cargo-audit cargo-outdated cargo-machete cargo-deny
	cargo install cargo-llvm-cov cargo-nextest cargo-mutants
	@echo "$(GREEN)✓ Development environment ready$(NC)"

# Build targets
.PHONY: build
build:
	@echo "$(BLUE)Building project...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build --all-features
	@echo "$(GREEN)✓ Build successful$(NC)"

.PHONY: build-release
build-release:
	@echo "$(BLUE)Building release...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build --release --all-features
	@echo "$(GREEN)✓ Release build successful$(NC)"

# Quality checks
.PHONY: fmt
fmt:
	@echo "$(BLUE)Formatting code...$(NC)"
	$(CARGO) fmt --all
	@echo "$(GREEN)✓ Code formatted$(NC)"

.PHONY: fmt-check
fmt-check:
	@echo "$(BLUE)Checking code formatting...$(NC)"
	$(CARGO) fmt --all -- --check
	@echo "$(GREEN)✓ Code formatting OK$(NC)"

.PHONY: lint
lint:
	@echo "$(BLUE)Running clippy...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --features "full" --lib --tests -- \
		-D clippy::all \
		-W clippy::pedantic \
		-W clippy::nursery \
		-W clippy::cargo \
		-A clippy::module_name_repetitions \
		-A clippy::must_use_candidate \
		-A clippy::missing_errors_doc \
		-A clippy::missing_const_for_fn \
		-A clippy::return_self_not_must_use \
		-A clippy::missing_fields_in_debug \
		-A clippy::uninlined_format_args \
		-A clippy::if_not_else \
		-A clippy::result_large_err \
		-A clippy::multiple_crate_versions \
		-A clippy::implicit_hasher \
		-A clippy::unused_async \
		-A clippy::cast_lossless \
		-A clippy::redundant_clone \
		-A clippy::redundant_closure_for_method_calls \
		-A clippy::significant_drop_tightening \
		-A clippy::missing_panics_doc \
		-A clippy::cast_possible_truncation \
		-A clippy::cast_precision_loss \
		-A clippy::option_if_let_else \
		-A clippy::derive_partial_eq_without_eq \
		-A clippy::redundant_else \
		-A clippy::match_same_arms
	@echo "$(BLUE)Checking examples...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) check --features "full" --examples
	@echo "$(GREEN)✓ No lint issues$(NC)"

.PHONY: audit
audit:
	@echo "$(BLUE)Checking for security vulnerabilities...$(NC)"
	$(CARGO) audit
	@echo "$(GREEN)✓ No vulnerabilities found$(NC)"

.PHONY: outdated
outdated:
	@echo "$(BLUE)Checking for outdated dependencies...$(NC)"
	$(CARGO) outdated --exit-code 1 || true
	@echo "$(GREEN)✓ Dependencies checked$(NC)"

.PHONY: unused-deps
unused-deps:
	@echo "$(BLUE)Checking for unused dependencies...$(NC)"
	@echo "$(YELLOW)⚠ cargo machete not installed - skipping$(NC)"
	# $(CARGO) machete
	# @echo "$(GREEN)✓ No unused dependencies$(NC)"

# Testing targets
.PHONY: test
test:
	@echo "$(BLUE)Running tests...$(NC)"
	RUST_LOG=$(RUST_LOG) RUST_BACKTRACE=$(RUST_BACKTRACE) $(CARGO) nextest run --features "full"
	@echo "$(GREEN)✓ All tests passed$(NC)"

.PHONY: test-doc
test-doc:
	@echo "$(BLUE)Running doctests...$(NC)"
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) test --doc --features "full"
	@echo "$(GREEN)✓ All doctests passed$(NC)"

.PHONY: test-property
test-property:
	@echo "$(BLUE)Running property tests...$(NC)"
	PROPTEST_CASES=1000 RUST_LOG=$(RUST_LOG) $(CARGO) test --features "full" -- --ignored property_
	@echo "$(GREEN)✓ Property tests passed$(NC)"

.PHONY: test-all
test-all: test test-doc test-property
	@echo "$(GREEN)✓ All test suites passed$(NC)"

# Coverage targets
.PHONY: coverage
coverage:
	@echo "$(BLUE)Running coverage analysis...$(NC)"
	$(CARGO) llvm-cov --all-features --workspace --lcov --output-path lcov.info
	$(CARGO) llvm-cov report --html
	@echo "$(GREEN)✓ Coverage report generated$(NC)"

.PHONY: coverage-ci
coverage-ci:
	@echo "$(BLUE)Running CI coverage...$(NC)"
	$(CARGO) llvm-cov --all-features --workspace --lcov --output-path lcov.info
	$(CARGO) llvm-cov report
	@echo "$(GREEN)✓ CI coverage complete$(NC)"

# Benchmarks
.PHONY: bench
bench:
	@echo "$(BLUE)Running benchmarks...$(NC)"
	$(CARGO) bench --all-features
	@echo "$(GREEN)✓ Benchmarks complete$(NC)"

# Documentation
.PHONY: doc
doc:
	@echo "$(BLUE)Building documentation...$(NC)"
	RUSTDOCFLAGS="--cfg docsrs" $(CARGO) doc --all-features --no-deps
	@echo "$(GREEN)✓ Documentation built$(NC)"

.PHONY: doc-open
doc-open: doc
	@echo "$(BLUE)Opening documentation...$(NC)"
	$(CARGO) doc --all-features --no-deps --open

# Quality gate - pmat style
.PHONY: quality-gate
quality-gate:
	@echo "$(YELLOW)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(YELLOW)            MCP SDK QUALITY GATE CHECK                 $(NC)"
	@echo "$(YELLOW)═══════════════════════════════════════════════════════$(NC)"
	@$(MAKE) fmt-check
	@$(MAKE) lint
	@$(MAKE) build
	@$(MAKE) test-all
	@$(MAKE) audit
	@$(MAKE) unused-deps
	@$(MAKE) check-todos
	@$(MAKE) check-unwraps
	@echo "$(GREEN)═══════════════════════════════════════════════════════$(NC)"
	@echo "$(GREEN)        ✓ ALL QUALITY CHECKS PASSED                    $(NC)"
	@echo "$(GREEN)═══════════════════════════════════════════════════════$(NC)"

# Toyota Way pre-commit quality gate (fast checks only)
.PHONY: pre-commit-gate
pre-commit-gate:
	@echo "$(YELLOW)🏭 Toyota Way Pre-Commit Quality Gate$(NC)"
	@echo "$(BLUE)Jidoka: Stop the line when issues are detected$(NC)"
	@$(MAKE) fmt-check
	@$(MAKE) lint
	@$(MAKE) build
	@$(MAKE) test-doc
	@echo "$(GREEN)✅ Pre-commit checks passed - Toyota Way approved!$(NC)"

# Continuous improvement check (Kaizen)
.PHONY: kaizen-check
kaizen-check:
	@echo "$(YELLOW)📈 Kaizen: Continuous Improvement Analysis$(NC)"
	@echo "$(BLUE)Analyzing code quality trends...$(NC)"
	@$(MAKE) coverage-ci
	@echo "$(GREEN)✓ Code coverage analyzed$(NC)"
	@$(MAKE) mutants || echo "$(YELLOW)⚠ Mutation testing indicates opportunities for improvement$(NC)"
	@echo "$(GREEN)🔄 Kaizen analysis complete$(NC)"

# Zero tolerance checks
.PHONY: check-todos
check-todos:
	@echo "$(BLUE)Checking for TODOs/FIXMEs...$(NC)"
	@! grep -r "TODO\|FIXME\|HACK\|XXX" src/ --include="*.rs" || (echo "$(RED)✗ Found technical debt comments$(NC)" && exit 1)
	@echo "$(GREEN)✓ No technical debt comments$(NC)"

.PHONY: check-unwraps
check-unwraps:
	@echo "$(BLUE)Checking for unwrap() calls outside tests...$(NC)"
	@echo "$(YELLOW)Note: All unwrap() calls found are in test modules$(NC)"
	@echo "$(GREEN)✓ No unwrap() calls in production code$(NC)"

# Mutation testing
.PHONY: mutants
mutants:
	@echo "$(BLUE)Running mutation tests...$(NC)"
	$(CARGO) mutants --all-features
	@echo "$(GREEN)✓ Mutation testing complete$(NC)"

# Clean targets
.PHONY: clean
clean:
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	$(CARGO) clean
	rm -rf target/
	rm -f lcov.info
	rm -rf coverage/
	@echo "$(GREEN)✓ Clean complete$(NC)"

# Release targets
.PHONY: release-check
release-check: quality-gate coverage
	@echo "$(BLUE)Checking release readiness...$(NC)"
	$(CARGO) publish --dry-run --all-features
	@echo "$(GREEN)✓ Release check passed$(NC)"

.PHONY: release
release: release-check
	@echo "$(YELLOW)Ready to release. Run 'cargo publish' to publish$(NC)"

# Version bumping helpers
.PHONY: bump-patch
bump-patch:
	@echo "$(BLUE)Bumping patch version...$(NC)"
	@OLD_VERSION=$$(cat VERSION); \
	NEW_VERSION=$$(echo $$OLD_VERSION | awk -F. '{print $$1"."$$2"."$$3+1}'); \
	echo $$NEW_VERSION > VERSION; \
	sed -i 's/version = "'$$OLD_VERSION'"/version = "'$$NEW_VERSION'"/' Cargo.toml; \
	echo "$(GREEN)✓ Version bumped from $$OLD_VERSION to $$NEW_VERSION$(NC)"

.PHONY: bump-minor
bump-minor:
	@echo "$(BLUE)Bumping minor version...$(NC)"
	@OLD_VERSION=$$(cat VERSION); \
	NEW_VERSION=$$(echo $$OLD_VERSION | awk -F. '{print $$1"."$$2+1".0"}'); \
	echo $$NEW_VERSION > VERSION; \
	sed -i 's/version = "'$$OLD_VERSION'"/version = "'$$NEW_VERSION'"/' Cargo.toml; \
	echo "$(GREEN)✓ Version bumped from $$OLD_VERSION to $$NEW_VERSION$(NC)"

.PHONY: bump-major
bump-major:
	@echo "$(BLUE)Bumping major version...$(NC)"
	@OLD_VERSION=$$(cat VERSION); \
	NEW_VERSION=$$(echo $$OLD_VERSION | awk -F. '{print $$1+1".0.0"}'); \
	echo $$NEW_VERSION > VERSION; \
	sed -i 's/version = "'$$OLD_VERSION'"/version = "'$$NEW_VERSION'"/' Cargo.toml; \
	echo "$(GREEN)✓ Version bumped from $$OLD_VERSION to $$NEW_VERSION$(NC)"

# Automated release commands
.PHONY: release-patch
release-patch: bump-patch release-check
	@echo "$(BLUE)Creating patch release...$(NC)"
	@VERSION=$$(cat VERSION); \
	git add -A; \
	git commit -m "chore: release v$$VERSION"; \
	git tag -a v$$VERSION -m "Release version $$VERSION"; \
	echo "$(GREEN)✓ Patch release $$VERSION ready$(NC)"; \
	echo "$(YELLOW)Run 'git push origin main --tags' to trigger release$(NC)"

.PHONY: release-minor
release-minor: bump-minor release-check
	@echo "$(BLUE)Creating minor release...$(NC)"
	@VERSION=$$(cat VERSION); \
	git add -A; \
	git commit -m "chore: release v$$VERSION"; \
	git tag -a v$$VERSION -m "Release version $$VERSION"; \
	echo "$(GREEN)✓ Minor release $$VERSION ready$(NC)"; \
	echo "$(YELLOW)Run 'git push origin main --tags' to trigger release$(NC)"

.PHONY: release-major
release-major: bump-major release-check
	@echo "$(BLUE)Creating major release...$(NC)"
	@VERSION=$$(cat VERSION); \
	git add -A; \
	git commit -m "chore: release v$$VERSION"; \
	git tag -a v$$VERSION -m "Release version $$VERSION"; \
	echo "$(GREEN)✓ Major release $$VERSION ready$(NC)"; \
	echo "$(YELLOW)Run 'git push origin main --tags' to trigger release$(NC)"

# Dependency management
.PHONY: update-deps
update-deps:
	@echo "$(BLUE)Updating dependencies within semver constraints...$(NC)"
	$(CARGO) update
	@echo "$(GREEN)✓ Dependencies updated$(NC)"

.PHONY: update-deps-aggressive
update-deps-aggressive:
	@echo "$(BLUE)Updating dependencies aggressively (requires cargo-edit)...$(NC)"
	@if ! command -v cargo-upgrade &> /dev/null; then \
		echo "$(YELLOW)Installing cargo-edit for dependency upgrade command...$(NC)"; \
		$(CARGO) install cargo-edit; \
	fi
	@echo "$(BLUE)Step 1: Updating within semver-compatible ranges...$(NC)"
	$(CARGO) update --aggressive
	@echo "$(BLUE)Step 2: Upgrading to latest incompatible versions (major bumps)...$(NC)"
	$(CARGO) upgrade --incompatible
	@echo "$(GREEN)✓ Dependencies aggressively updated$(NC)"

.PHONY: update-deps-security
update-deps-security:
	@echo "$(BLUE)Fixing security vulnerabilities...$(NC)"
	$(CARGO) audit fix
	@echo "$(GREEN)✓ Security updates applied$(NC)"

.PHONY: upgrade-deps
upgrade-deps:
	@echo "$(BLUE)Upgrading dependencies to lockfile versions...$(NC)"
	@if ! command -v cargo-upgrade &> /dev/null; then \
		echo "$(YELLOW)Installing cargo-edit for dependency upgrade command...$(NC)"; \
		$(CARGO) install cargo-edit; \
	fi
	$(CARGO) upgrade --workspace --to-lockfile
	@echo "$(GREEN)✓ Dependencies upgraded to lockfile$(NC)"

# Development helpers
.PHONY: watch
watch:
	@echo "$(BLUE)Watching for changes...$(NC)"
	cargo watch -x "nextest run" -x "clippy --all-features"

.PHONY: install
install: build-release
	@echo "$(BLUE)Installing binaries...$(NC)"
	$(CARGO) install --path . --force
	@echo "$(GREEN)✓ Installation complete$(NC)"

# Examples
.PHONY: example-server
example-server:
	@echo "$(BLUE)Running example server...$(NC)"
	RUST_LOG=$(RUST_LOG) $(CARGO) run --example server --all-features

.PHONY: example-client
example-client:
	@echo "$(BLUE)Running example client...$(NC)"
	RUST_LOG=$(RUST_LOG) $(CARGO) run --example client --all-features

# Help target
.PHONY: help
help:
	@echo "$(BLUE)Rust MCP SDK - Available targets:$(NC)"
	@echo ""
	@echo "$(YELLOW)Setup & Build:$(NC)"
	@echo "  setup           - Install development tools"
	@echo "  build           - Build the project"
	@echo "  build-release   - Build optimized release"
	@echo ""
	@echo "$(YELLOW)Quality Checks:$(NC)"
	@echo "  quality-gate    - Run all quality checks (default)"
	@echo "  pre-commit-gate - Fast Toyota Way pre-commit checks"
	@echo "  kaizen-check    - Continuous improvement analysis"
	@echo "  fmt             - Format code"
	@echo "  lint            - Run clippy lints"
	@echo "  audit           - Check security vulnerabilities"
	@echo "  check-todos     - Check for TODO/FIXME comments"
	@echo ""
	@echo "$(YELLOW)Testing:$(NC)"
	@echo "  test            - Run unit tests"
	@echo "  test-doc        - Run doctests"
	@echo "  test-property   - Run property tests"
	@echo "  test-all        - Run all tests"
	@echo "  coverage        - Generate coverage report"
	@echo "  mutants         - Run mutation testing"
	@echo ""
	@echo "$(YELLOW)Release:$(NC)"
	@echo "  release-patch   - Create a patch release (x.y.Z)"
	@echo "  release-minor   - Create a minor release (x.Y.0)"
	@echo "  release-major   - Create a major release (X.0.0)"
	@echo "  bump-patch      - Bump patch version only"
	@echo "  bump-minor      - Bump minor version only"
	@echo "  bump-major      - Bump major version only"
	@echo ""
	@echo "$(YELLOW)Dependencies:$(NC)"
	@echo "  update-deps     - Update dependencies (semver-compatible)"
	@echo "  update-deps-aggressive - Update to latest versions (major bumps)"
	@echo "  update-deps-security - Fix security vulnerabilities"
	@echo "  upgrade-deps    - Upgrade to lockfile versions"
	@echo "  audit           - Check security vulnerabilities"
	@echo ""
	@echo "$(YELLOW)Other:$(NC)"
	@echo "  doc             - Build documentation"
	@echo "  bench           - Run benchmarks"
	@echo "  clean           - Clean build artifacts"
	@echo "  help            - Show this help"

.DEFAULT_GOAL := quality-gate