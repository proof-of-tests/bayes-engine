#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

echo "Checking Rust code formatting..."
cargo fmt --check || {
  echo "Error: Rust code is not properly formatted"
  echo "Run 'cargo fmt' to fix formatting"
  exit 1
}

echo "All Rust code is properly formatted!"
