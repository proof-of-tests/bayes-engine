#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

# Find all markdown files and check if they're formatted
find . -name "*.md" -type f | while read -r file; do
  echo "Checking $file..."
  mdformat --check "$file" || {
    echo "Error: $file is not properly formatted"
    echo "Run 'nix run nixpkgs#mdformat -- $file' to fix"
    exit 1
  }
done

echo "All markdown files are properly formatted!"
