#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

# Find all shell scripts and check with shellcheck
find . -name "*.sh" -type f | while read -r file; do
  echo "Checking $file..."
  shellcheck "$file" || {
    echo "Error: $file has shellcheck issues"
    echo "Run 'nix run nixpkgs#shellcheck -- $file' to see details"
    exit 1
  }
done

echo "All shell scripts passed shellcheck!"
