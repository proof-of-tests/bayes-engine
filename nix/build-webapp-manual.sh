#!/usr/bin/env bash
set -euo pipefail

echo "=== Building webapp with client WASM and server Worker (manual build) ==="

# Setup environment for wasm-pack and cargo
export HOME=$TMPDIR
mkdir -p "$HOME/.cache"

# Unset any cargo target that crane might have set
unset CARGO_BUILD_TARGET

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

# Build server WASM
echo "Building server WASM..."
cd server
cargo build --release --target wasm32-unknown-unknown --package server
cd ..

# Run wasm-bindgen on the server WASM
echo "Running wasm-bindgen..."
mkdir -p build/worker
wasm-bindgen \
  target/wasm32-unknown-unknown/release/server.wasm \
  --out-dir build/worker \
  --target web \
  --no-typescript

# Copy the JavaScript worker wrapper
echo "Copying worker wrapper..."
cp server/worker.js build/worker/shim.mjs

# Generate content hash for deployment comparison
echo "Generating content hash..."
# Hash all files in the assets directory, sorted for consistency
CONTENT_HASH=$(find assets -type f -exec sha256sum {} \; | sort | sha256sum | cut -d' ' -f1)
echo "$CONTENT_HASH" > assets/content-hash.txt
echo "Content hash: $CONTENT_HASH"

echo "=== Build complete ==="
