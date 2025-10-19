#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

echo "Running Clippy lints..."
cargo clippy -- -D warnings || {
  echo "Error: Clippy found issues in the code"
  echo "Run 'cargo clippy' to see detailed warnings"
  exit 1
}

echo "All Clippy checks passed!"
