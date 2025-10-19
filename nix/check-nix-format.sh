#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

# Find all nix files and check if they're formatted
find . -name "*.nix" -type f | while read -r file; do
  echo "Checking $file..."
  nixpkgs-fmt --check "$file" || {
    echo "Error: $file is not properly formatted"
    echo "Run 'nix fmt' to fix formatting"
    exit 1
  }
done

echo "All Nix files are properly formatted!"
