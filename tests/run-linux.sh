#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo test --release --locked --features config-app
cargo clippy --all-targets --features config-app -- -D warnings
cargo build --release --locked --features config-app
python3 tests/ffi_smoke.py
python3 -m unittest discover -s tests -p 'test_*.py'
cmake -S . -B tests/artifacts/linux-build
cmake --build tests/artifacts/linux-build --parallel
