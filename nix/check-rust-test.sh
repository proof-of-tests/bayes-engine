#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

echo "Running Rust tests..."
cargo test || {
  echo "Error: Some tests failed"
  echo "Run 'cargo test' to see detailed test results"
  exit 1
}

echo "All tests passed!"
