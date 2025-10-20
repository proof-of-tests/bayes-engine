#!/usr/bin/env bash
set -euo pipefail

echo "=== Building server Worker ==="

# Setup environment for worker-build
export HOME=$TMPDIR
mkdir -p "$HOME/.cache/worker-build"

# Unset any cargo target that crane might have set
unset CARGO_BUILD_TARGET

# Determine platform for esbuild cache
if [ "$(uname -s)" = "Linux" ]; then
  if [ "$(uname -m)" = "x86_64" ]; then
    ESBUILD_PLATFORM="linux-x64"
  elif [ "$(uname -m)" = "aarch64" ]; then
    ESBUILD_PLATFORM="linux-arm64"
  fi
elif [ "$(uname -s)" = "Darwin" ]; then
  if [ "$(uname -m)" = "arm64" ]; then
    ESBUILD_PLATFORM="darwin-arm64"
  else
    echo "Unsupported Darwin platform (only aarch64-darwin is supported)"
    exit 1
  fi
fi

# Symlink esbuild 0.25.10 to the cache location
ln -sf "$(command -v esbuild)" "$HOME/.cache/worker-build/esbuild-$ESBUILD_PLATFORM-0.25.10"

# Build server worker
echo "Building server worker..."
cd server
cargo --version
worker-build --release --mode no-install --package server
cd ..

echo "=== Server build complete ==="
