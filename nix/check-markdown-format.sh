#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

# Find all markdown files and check if they're formatted
# Configuration: wrap at 120 characters, use sequential numbering
# Note: Skip docs/frameworks.md due to mdformat table formatting issues with wrap enabled
find . -name "*.md" -type f ! -path "./docs/frameworks.md" | while read -r file; do
  echo "Checking $file..."
  mdformat --check --wrap 120 --number "$file" || {
    echo "Error: $file is not properly formatted"
    echo "Run 'nix run nixpkgs#mdformat -- --wrap 120 --number $file' to fix"
    exit 1
  }
done

echo "All markdown files are properly formatted!"
