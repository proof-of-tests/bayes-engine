#!/usr/bin/env bash
set -euo pipefail

SOURCE_DIR="${1:-.}"

cd "$SOURCE_DIR"

# Run statix on all nix files
echo "Running statix checks..."
statix check . || {
  echo "Error: Nix files have lint issues"
  echo "Run 'nix run nixpkgs#statix check' to see issues"
  echo "Run 'nix run nixpkgs#statix fix' to auto-fix"
  exit 1
}

echo "All Nix files passed linting!"
