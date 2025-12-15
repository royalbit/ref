# ref-tools - Reference verification tools
# Build targets for optimized static binaries

.PHONY: help build build-linux install deploy-kveldulf uninstall lint format test clean pre-commit

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
HAS_DOCKER := $(shell command -v docker 2> /dev/null)

# Target for kveldulf (Linux x86_64)
LINUX_TARGET := x86_64-unknown-linux-musl
LINUX_BINARY := target/$(LINUX_TARGET)/release/ref-tools

# Remote deploy target
KVELDULF := kveldulf

help:
	@echo "ref-tools - Available Commands"
	@echo ""
	@echo "Platform: $(PLATFORM) ($(ARCH))"
	@echo "Target:   $(BUILD_TARGET)"
	@echo ""
	@echo "Build Targets:"
	@echo "  make build              - Standard release build"
	@echo "  make build-linux        - Cross-compile for Linux x86_64 (requires cross + Docker)"
	@echo ""
	@echo "Install/Deploy Targets:"
	@echo "  make install            - Install to ~/bin (local macOS)"
	@echo "  make deploy-kveldulf    - Sync source, build on kveldulf, install to ~/bin"
	@echo "  make uninstall          - Remove from ~/bin"
	@echo ""
	@echo "Code Quality:"
	@echo "  make lint               - Run pedantic clippy checks"
	@echo "  make format             - Format code with rustfmt"
	@echo "  make test               - Run all tests"
	@echo ""
	@echo "Workflows:"
	@echo "  make pre-commit         - Full check (format + lint + test)"
	@echo "  make clean              - Remove build artifacts"

build:
	@echo "Building release binary..."
	@cargo build --release
	@echo "Binary: target/release/ref-tools"
	@ls -lh target/release/ref-tools

build-linux:
ifndef HAS_CROSS
	@echo "Error: cross-rs not found. Install: cargo install cross"
	@exit 1
endif
ifndef HAS_DOCKER
	@echo "Error: Docker not found. cross-rs requires Docker."
	@exit 1
endif
	@echo "Cross-compiling for Linux x86_64..."
	@cross build --release --target $(LINUX_TARGET)
	@echo "Binary: $(LINUX_BINARY)"
	@ls -lh $(LINUX_BINARY)

install: build
	@mkdir -p ~/bin
	@install -m 755 target/release/ref-tools ~/bin/ref-tools
	@echo "Installed to ~/bin/ref-tools"

# Deploy to kveldulf by building remotely (more reliable than cross-compile)
deploy-kveldulf:
	@echo "Deploying to $(KVELDULF)..."
	@echo "  1. Syncing source..."
	@rsync -az --delete --exclude='target/' --exclude='.git/' . $(KVELDULF):~/src/ref-tools/
	@echo "  2. Building on $(KVELDULF)..."
	@ssh $(KVELDULF) "cd ~/src/ref-tools && cargo build --release"
	@echo "  3. Installing to ~/bin..."
	@ssh $(KVELDULF) "mkdir -p ~/bin && install -m 755 ~/src/ref-tools/target/release/ref-tools ~/bin/ref-tools"
	@echo "  4. Verifying..."
	@ssh $(KVELDULF) "~/bin/ref-tools --version"
	@echo "Deployed successfully!"

uninstall:
	@rm -f ~/bin/ref-tools

lint:
	@cargo clippy --all-targets -- -D warnings

format:
	@cargo fmt

test:
	@cargo test

clean:
	@cargo clean

pre-commit: format lint test
