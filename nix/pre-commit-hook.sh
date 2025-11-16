#!/usr/bin/env bash
set -euo pipefail

# Pre-commit hook that runs nix flake check and e2e tests
# Captures all output and only displays it on failure

# Create temporary file for capturing output
TMP_OUTPUT=$(mktemp)
trap 'rm -f "$TMP_OUTPUT"' EXIT

# Function to run a command and capture output
run_check() {
  local cmd="$1"
  local name="$2"
  
  echo "Running $name..."
  if "$cmd" > "$TMP_OUTPUT" 2>&1; then
    echo "$name passed ✓"
    return 0
  else
    local exit_code=$?
    echo ""
    echo "========================================="
    echo "$name FAILED"
    echo "========================================="
    cat "$TMP_OUTPUT"
    echo "========================================="
    return $exit_code
  fi
}

# Run nix flake check
run_check "nix flake check" "nix flake check" || exit $?

# Run e2e tests
run_check "nix run .#run-e2e-tests" "nix run .#run-e2e-tests" || exit $?

echo ""
echo "All pre-commit checks passed ✓"
exit 0

