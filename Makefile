# Cartograph developer commands.
#
# Every target here is what CI runs. If `make check` passes locally, CI should
# agree — divergence between the two is a bug in this file.

.PHONY: help check fmt fmt-fix lint test build bench validate gates clean clean-sidecars

PYTHON := python3

## Show available targets
help:
	@echo ""
	@echo "  Cartograph — make targets"
	@echo ""
	@echo "  make check           fmt + lint + test + validate  (run before pushing)"
	@echo "  make gates           Quality gates QG-001 … QG-009, and the calibration tests"
	@echo ""
	@echo "  make fmt             Check formatting"
	@echo "  make fmt-fix         Apply formatting"
	@echo "  make lint            Clippy, warnings denied"
	@echo "  make test            Test the workspace"
	@echo "  make build           Release build"
	@echo "  make bench           Criterion benchmarks"
	@echo "  make validate        AgentOS framework health"
	@echo ""
	@echo "  make clean           Remove build artifacts"
	@echo "  make clean-sidecars  Remove macOS ._* files (exFAT volumes)"
	@echo ""

## Everything CI checks
check: fmt lint test validate
	@echo "✓ all checks passed"

## Check formatting without changing anything
fmt:
	cargo fmt --all -- --check

## Apply formatting
fmt-fix:
	cargo fmt --all

## Clippy across every target and feature, warnings denied
lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

## Run the test suite
test:
	cargo test --workspace

## Release build
build:
	cargo build --workspace --release

## Criterion benchmarks. Results are not published; see docs/benchmarks/.
bench:
	cargo bench --workspace

## AgentOS framework health report
validate:
	@$(PYTHON) agentos/tools/scripts/validate_agentos.py

## Cartograph quality gates
gates:
	@$(PYTHON) benchmarks/m08/test_calibration.py -q
	@$(PYTHON) agentos/tools/scripts/run_gates.py

## Remove build artifacts
clean:
	cargo clean

## Remove macOS AppleDouble sidecar files.
##
## The repository may live on an exFAT volume, which has no native extended
## attribute support, so macOS materialises xattrs as `._name` files — including
## inside .git, where Git reports them as "non-monotonic index" errors.
## See docs/development/external-storage.md.
clean-sidecars:
	@find . -name '._*' -delete 2>/dev/null || true
	@find . -name '.DS_Store' -delete 2>/dev/null || true
	@echo "✓ sidecar files removed"
