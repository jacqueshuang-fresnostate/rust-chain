#!/usr/bin/env bash

set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "[P0 gate] Source integrity"
python3 scripts/source_integrity_gate.py

echo "[P0 gate] Source integrity scanner tests"
python3 -B -m unittest tests.test_source_integrity_gate

echo "[P0 gate] Rust formatting"
cargo fmt --all -- --check

echo "[P0 gate] Rust lint"
cargo clippy --all-targets -- -D warnings

echo "[P0 gate] Rust tests"
cargo test --all-targets -- --test-threads=1

echo "[P0 gate] Admin frontend"
npm --prefix web run lint
npm --prefix web run typecheck
npm --prefix web test

echo "[P0 gate] PC frontend"
npm --prefix pc run type-check
npm --prefix pc run test:margin
npm --prefix pc run build

echo "[P0 gate] Mobile frontend"
npm --prefix mobile run type-check
npm --prefix mobile test

echo "[P0 gate] All checks passed"
