#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo test --release --locked
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
python3 tests/ffi_smoke.py
cmake -S . -B tests/artifacts/linux-build
cmake --build tests/artifacts/linux-build --parallel
