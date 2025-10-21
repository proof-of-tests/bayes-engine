#!/usr/bin/env bash
set -euo pipefail

# Script to report sizes of WASM files and static assets

if [ $# -ne 1 ]; then
  echo "Usage: $0 <webapp-output-dir>"
  exit 1
fi

WEBAPP_DIR="$1"

if [ ! -d "$WEBAPP_DIR" ]; then
  echo "Error: Directory $WEBAPP_DIR does not exist"
  exit 1
fi

echo "Build Size Report"

# Function to format bytes in a human-readable way
format_bytes() {
  numfmt --to=iec-i --suffix=B "$1" 2>/dev/null || echo "$1 bytes"
}

# Function to report file size
report_file() {
  local file="$1"
  local label="$2"

  if [ -f "$file" ]; then
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    local formatted
    formatted=$(format_bytes "$size")
    printf "  %-24s %8s\n" "$label:" "$formatted"
  else
    printf "  %-24s %8s\n" "$label:" "NOT FOUND"
  fi
}

# Report WASM files
report_file "$WEBAPP_DIR/worker/server_bg.wasm" "Server WASM"
report_file "$WEBAPP_DIR/assets/pkg/client_bg.wasm" "Client WASM"

# Report JavaScript files
report_file "$WEBAPP_DIR/worker/server.js" "Server JS"
report_file "$WEBAPP_DIR/worker/worker.js" "Worker wrapper JS"
report_file "$WEBAPP_DIR/assets/pkg/client.js" "Client JS"

# Calculate total JS snippets size
if [ -d "$WEBAPP_DIR/assets/pkg/snippets" ]; then
  total_snippets_size=0
  snippet_count=0

  while IFS= read -r -d '' file; do
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    total_snippets_size=$((total_snippets_size + size))
    snippet_count=$((snippet_count + 1))
  done < <(find "$WEBAPP_DIR/assets/pkg/snippets" -type f -name "*.js" -print0)

  formatted=$(format_bytes "$total_snippets_size")
  printf "  %-24s %8s (%d files)\n" "JS Snippets:" "$formatted" "$snippet_count"
fi

# Report Static Assets
report_file "$WEBAPP_DIR/assets/index.html" "HTML"
report_file "$WEBAPP_DIR/assets/style.css" "CSS"

# Calculate totals
total_wasm=0
total_static=0
total_js=0

# Add server WASM
if [ -f "$WEBAPP_DIR/worker/server_bg.wasm" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/worker/server_bg.wasm" 2>/dev/null || stat -c%s "$WEBAPP_DIR/worker/server_bg.wasm" 2>/dev/null)
  total_wasm=$((total_wasm + size))
fi

# Add client WASM
if [ -f "$WEBAPP_DIR/assets/pkg/client_bg.wasm" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/assets/pkg/client_bg.wasm" 2>/dev/null || stat -c%s "$WEBAPP_DIR/assets/pkg/client_bg.wasm" 2>/dev/null)
  total_wasm=$((total_wasm + size))
fi

# Add static assets
if [ -f "$WEBAPP_DIR/assets/index.html" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/assets/index.html" 2>/dev/null || stat -c%s "$WEBAPP_DIR/assets/index.html" 2>/dev/null)
  total_static=$((total_static + size))
fi

if [ -f "$WEBAPP_DIR/assets/style.css" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/assets/style.css" 2>/dev/null || stat -c%s "$WEBAPP_DIR/assets/style.css" 2>/dev/null)
  total_static=$((total_static + size))
fi

# Add all JS files
if [ -f "$WEBAPP_DIR/worker/server.js" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/worker/server.js" 2>/dev/null || stat -c%s "$WEBAPP_DIR/worker/server.js" 2>/dev/null)
  total_js=$((total_js + size))
fi

if [ -f "$WEBAPP_DIR/worker/worker.js" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/worker/worker.js" 2>/dev/null || stat -c%s "$WEBAPP_DIR/worker/worker.js" 2>/dev/null)
  total_js=$((total_js + size))
fi

if [ -f "$WEBAPP_DIR/assets/pkg/client.js" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/assets/pkg/client.js" 2>/dev/null || stat -c%s "$WEBAPP_DIR/assets/pkg/client.js" 2>/dev/null)
  total_js=$((total_js + size))
fi

# Add snippets to JS total
if [ -d "$WEBAPP_DIR/assets/pkg/snippets" ]; then
  while IFS= read -r -d '' file; do
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    total_js=$((total_js + size))
  done < <(find "$WEBAPP_DIR/assets/pkg/snippets" -type f -name "*.js" -print0)
fi

# Print totals
echo "---"
formatted=$(format_bytes "$total_wasm")
printf "  %-24s %8s\n" "Total WASM:" "$formatted"

formatted=$(format_bytes "$total_js")
printf "  %-24s %8s\n" "Total JS:" "$formatted"

formatted=$(format_bytes "$total_static")
printf "  %-24s %8s\n" "Total Static:" "$formatted"

grand_total=$((total_wasm + total_static + total_js))
formatted=$(format_bytes "$grand_total")
printf "  %-24s %8s\n" "Total:" "$formatted"
