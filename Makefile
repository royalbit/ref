# ref-tools - Reference verification tools
# Build targets for optimized static binaries

.PHONY: help build build-static build-compressed build-all install install-user uninstall lint lint-fix format format-check test test-unit test-e2e coverage clean check pre-commit

# ═══════════════════════════════════════════════════════════════════════════════
# OS AND ARCHITECTURE DETECTION
# ═══════════════════════════════════════════════════════════════════════════════

UNAME_S := $(shell uname -s 2>/dev/null || echo Windows)
UNAME_M := $(shell uname -m 2>/dev/null || echo x86_64)

ifeq ($(UNAME_M),arm64)
    ARCH := aarch64
else ifeq ($(UNAME_M),aarch64)
    ARCH := aarch64
else
    ARCH := x86_64
endif

ifeq ($(UNAME_S),Linux)
    PLATFORM := linux
    BUILD_TARGET := $(ARCH)-unknown-linux-musl
    STATIC_BINARY := target/$(BUILD_TARGET)/release/ref-tools
    TARGET_FLAG := --target $(BUILD_TARGET)
    UPX_SUPPORTED := true
else ifeq ($(UNAME_S),Darwin)
    PLATFORM := macos
    BUILD_TARGET := $(ARCH)-apple-darwin
    STATIC_BINARY := target/release/ref-tools
    TARGET_FLAG :=
    UPX_SUPPORTED := false
else
    PLATFORM := unknown
    BUILD_TARGET :=
    STATIC_BINARY := target/release/ref-tools
    TARGET_FLAG :=
    UPX_SUPPORTED := false
endif

HAS_UPX := $(shell command -v upx 2> /dev/null)
HAS_CROSS := $(shell command -v cross 2> /dev/null)

CROSS_TARGETS := x86_64-unknown-linux-musl aarch64-unknown-linux-musl

help:
	@echo "ref-tools - Available Commands"
	@echo ""
	@echo "Platform: $(PLATFORM) ($(ARCH))"
	@echo "Target:   $(BUILD_TARGET)"
	@echo ""
	@echo "Build Targets:"
	@echo "  make build              - Standard release build"
	@echo "  make build-static       - Static release build (musl on Linux)"
	@echo "  make build-compressed   - Static + UPX compressed (Linux only)"
	@echo "  make build-all          - Cross-compile for Linux x86_64 + aarch64"
	@echo ""
	@echo "Install Targets:"
	@echo "  make install            - Install to ~/.local/bin"
	@echo "  make install-user       - Same as install"
	@echo "  make uninstall          - Remove from ~/.local/bin"
	@echo ""
	@echo "Code Quality:"
	@echo "  make lint               - Run pedantic clippy checks"
	@echo "  make lint-fix           - Auto-fix clippy warnings"
	@echo "  make format             - Format code with rustfmt"
	@echo "  make format-check       - Check formatting"
	@echo ""
	@echo "Test Targets:"
	@echo "  make test               - Run all tests"
	@echo "  make test-unit          - Run unit tests only"
	@echo "  make test-e2e           - Run E2E tests"
	@echo "  make coverage           - Run coverage (100% required)"
	@echo ""
	@echo "Workflows:"
	@echo "  make pre-commit         - Full check (format + lint + test)"
	@echo "  make check              - Quick check during development"
	@echo "  make clean              - Remove build artifacts"

build:
	@echo "Building release binary..."
	@cargo build --release
	@echo "Binary: target/release/ref-tools"
	@ls -lh target/release/ref-tools

build-static:
	@echo "Building static release binary..."
	@echo "   Platform: $(PLATFORM) ($(ARCH))"
	@echo "   Target:   $(BUILD_TARGET)"
ifeq ($(PLATFORM),linux)
	@cargo build --release $(TARGET_FLAG)
else
	@cargo build --release
endif
	@echo "Binary: $(STATIC_BINARY)"
	@ls -lh $(STATIC_BINARY)

build-compressed: build-static
ifeq ($(UPX_SUPPORTED),true)
ifdef HAS_UPX
	@echo "Compressing with UPX..."
	@ls -lh $(STATIC_BINARY)
	@upx --best --lzma $(STATIC_BINARY)
	@echo "After compression:"
	@ls -lh $(STATIC_BINARY)
else
	@echo "UPX not found - binary not compressed"
endif
else
	@echo "UPX not supported on $(PLATFORM)"
endif

build-all:
ifndef HAS_CROSS
	@echo "cross-rs not found. Install: cargo install cross"
	@exit 1
endif
	@mkdir -p dist
	@for target in $(CROSS_TARGETS); do \
		echo "Building $$target..."; \
		cross build --release --target $$target && \
		cp target/$$target/release/ref-tools dist/ref-tools-$$target; \
	done
	@ls -lh dist/

install-user: build
	@mkdir -p ~/.local/bin
	@install -m 755 target/release/ref-tools ~/.local/bin/ref-tools
	@echo "Installed to ~/.local/bin/ref-tools"

install: install-user

uninstall:
	@rm -f ~/.local/bin/ref-tools

lint:
	@cargo clippy --all-targets -- -D warnings

lint-fix:
	@cargo clippy --fix --allow-dirty --allow-staged

format:
	@cargo fmt

format-check:
	@cargo fmt -- --check

test:
	@cargo test

test-unit:
	@cargo test --lib

test-e2e:
	@cargo test --test '*'

coverage:
	@cargo llvm-cov --fail-under-lines 100 --ignore-filename-regex 'tests/'

clean:
	@cargo clean
	@rm -rf dist/

check: format-check lint test-unit

pre-commit: format-check lint test
