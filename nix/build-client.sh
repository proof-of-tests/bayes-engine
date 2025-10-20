#!/usr/bin/env bash
set -euo pipefail

echo "=== Building client WASM ==="

# Setup environment - wasm-pack may need to write to HOME
export HOME=$TMPDIR
mkdir -p "$HOME/.cache"

# Build client WASM with wasm-pack
mkdir -p assets/pkg
cd client
wasm-pack build --release --target web --out-dir ../assets/pkg --no-typescript --mode no-install
cd ..

# Copy static HTML and CSS to assets
echo "Copying static assets..."
cp public/index.html assets/index.html
cp public/style.css assets/style.css

# Generate content hash for deployment comparison
echo "Generating content hash..."
# Hash all files in the assets directory, sorted for consistency
CONTENT_HASH=$(find assets -type f -exec sha256sum {} \; | sort | sha256sum | cut -d' ' -f1)
echo "$CONTENT_HASH" > assets/content-hash.txt
echo "Content hash: $CONTENT_HASH"

echo "=== Client build complete ==="
