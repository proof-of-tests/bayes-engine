#!/usr/bin/env bash
set -euo pipefail

# Build the client WASM
echo "Building client WASM..."
cd client
wasm-pack build --target web --out-dir ../public/pkg --no-typescript
cd ..

# Build the server worker
echo "Building server worker..."
cd server
worker-build --release --mode no-install
cd ..

# Copy server build output
rm -rf build
cp -r server/build build

echo "Build complete!"
