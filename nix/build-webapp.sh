#!/usr/bin/env bash
set -euo pipefail

echo "=== Building webapp with client WASM and server Worker ==="

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

# Build client WASM and prepare assets directory
echo "Building client WASM..."
mkdir -p assets/pkg
cd client
wasm-pack build --release --target web --out-dir ../assets/pkg --no-typescript --mode no-install
cd ..

# Copy static HTML and CSS to assets
echo "Copying static assets..."
cp public/index.html assets/index.html
cp public/style.css assets/style.css

# Build server worker
echo "Building server worker..."
cd server
cargo --version
worker-build --release --mode no-install --package server
cd ..

# Move server build output to root
echo "Moving build output..."
mv server/build .

echo "=== Build complete ==="
