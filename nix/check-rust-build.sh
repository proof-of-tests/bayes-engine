#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

echo "Building Rust project..."
cargo build --release || {
  echo "Error: Rust project failed to build"
  echo "Run 'cargo build' to see detailed error messages"
  exit 1
}

echo "Rust project built successfully!"
