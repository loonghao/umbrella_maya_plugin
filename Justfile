set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]

default:
    just --list

fmt:
    cargo fmt

fmt-check:
    cargo fmt -- --check

# Reconcile every pinned tool version (rust-toolchain.toml, vx.toml, Cargo.toml,
# CI actions) and fail loudly on the Windows GNU/raw-dylib host defect instead of
# silently working around it. Self-test needs no toolchain: --self-test.
check-toolchain:
    python scripts/check_toolchain_pins.py

check:
    cargo check --workspace --all-targets
    cargo check --workspace --all-targets --features python

clippy:
    cargo clippy --workspace --all-targets --features python -- -D warnings

test:
    cargo test --workspace --all-targets

coverage:
    python -c "import os; os.makedirs('coverage', exist_ok=True)"
    cargo llvm-cov --workspace --all-targets --features python --lcov --output-path coverage/lcov.info

preflight: check-toolchain fmt-check check clippy test build-cli build-pyd

build-tool:
    cargo build --bin cargo-maya-build --release

build-cli:
    cargo build --bin umbrella-maya --release

build-pyd:
    cargo build --release --features python
    python scripts/package-python-extension.py

clean:
    cargo run --bin cargo-maya-build -- --clean

build maya="2024":
    cargo run --bin cargo-maya-build -- --current-only --maya-version {{maya}}

package maya="2024":
    cargo run --bin cargo-maya-build -- --current-only --maya-version {{maya}}

package-all maya="2024":
    just build-cli
    just build-pyd
    just package {{maya}}

package-current maya="2024":
    cargo run --bin cargo-maya-build -- --current-only --maya-version {{maya}}
    cargo build --bin umbrella-maya --release
    cargo build --release --features python
    python scripts/package-python-extension.py

install maya="2024":
    cargo run --bin cargo-maya-build -- --current-only --maya-version {{maya}}
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/install-maya-module.ps1 -MayaVersion {{maya}}

build-platform platform maya="2024":
    cargo run --bin cargo-maya-build -- --platform {{platform}} --maya-version {{maya}}

build-all-versions:
    cargo run --bin cargo-maya-build -- --current-only --all-versions
