#!/usr/bin/env bash
set -euo pipefail

WASM_BUILD_DIR=$1

echo "=== Post-processing WASM files ==="

# Create output directories
mkdir -p assets/pkg
mkdir -p build/worker

# Process client WASM
echo "Running wasm-bindgen on client..."
wasm-bindgen \
  "$WASM_BUILD_DIR/client.wasm" \
  --out-dir assets/pkg \
  --target web \
  --no-typescript

# Optimize client WASM with wasm-opt
echo "Optimizing client WASM..."
wasm-opt \
  assets/pkg/client_bg.wasm \
  -o assets/pkg/client_bg.wasm \
  -Oz \
  --enable-bulk-memory \
  --enable-mutable-globals \
  --enable-sign-ext \
  --enable-nontrapping-float-to-int

# Process server WASM
echo "Running wasm-bindgen on server..."
wasm-bindgen \
  "$WASM_BUILD_DIR/server.wasm" \
  --out-dir build/worker \
  --target web \
  --no-typescript

# Copy static files
echo "Copying static assets..."
cp public/index.html assets/index.html
cp public/style.css assets/style.css

# Copy worker wrapper
echo "Copying worker wrapper..."
cp server/worker.js build/worker/shim.mjs

# Generate content hash for deployment comparison
echo "Generating content hash..."
CONTENT_HASH=$(find assets -type f -exec sha256sum {} \; | sort | sha256sum | cut -d' ' -f1)
echo "$CONTENT_HASH" > assets/content-hash.txt
echo "Content hash: $CONTENT_HASH"

echo "=== Post-processing complete ==="
