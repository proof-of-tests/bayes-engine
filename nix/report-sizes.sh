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

echo "=== WASM and Asset Size Report ==="
echo ""

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
    printf "%-30s %10s (%s bytes)\n" "$label:" "$formatted" "$size"
  else
    printf "%-30s %10s\n" "$label:" "NOT FOUND"
  fi
}

# Report Server WASM
echo "## Server WASM"
report_file "$WEBAPP_DIR/index_bg.wasm" "Server Worker WASM"
echo ""

# Report Client WASM
echo "## Client WASM"
report_file "$WEBAPP_DIR/assets/pkg/client_bg.wasm" "Client WASM"
echo ""

# Report Static Assets (excluding WASM)
echo "## Static Assets (excluding WASM)"
report_file "$WEBAPP_DIR/assets/index.html" "HTML"
report_file "$WEBAPP_DIR/assets/style.css" "CSS"
echo ""

# Report JavaScript files
echo "## JavaScript Assets"
report_file "$WEBAPP_DIR/index.js" "Server Worker JS"
report_file "$WEBAPP_DIR/assets/pkg/client.js" "Client JS"
echo ""

# Calculate total JS snippets size
echo "## JavaScript Snippets"
if [ -d "$WEBAPP_DIR/assets/pkg/snippets" ]; then
  total_snippets_size=0
  snippet_count=0

  while IFS= read -r -d '' file; do
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    total_snippets_size=$((total_snippets_size + size))
    snippet_count=$((snippet_count + 1))
  done < <(find "$WEBAPP_DIR/assets/pkg/snippets" -type f -name "*.js" -print0)

  formatted=$(format_bytes "$total_snippets_size")
  printf "%-30s %10s (%s bytes, %d files)\n" "Total Snippets:" "$formatted" "$total_snippets_size" "$snippet_count"
else
  echo "No snippets directory found"
fi
echo ""

# Calculate totals
echo "## Totals"
total_wasm=0
total_static=0
total_js=0

# Add server WASM
if [ -f "$WEBAPP_DIR/index_bg.wasm" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/index_bg.wasm" 2>/dev/null || stat -c%s "$WEBAPP_DIR/index_bg.wasm" 2>/dev/null)
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
if [ -f "$WEBAPP_DIR/index.js" ]; then
  size=$(stat -f%z "$WEBAPP_DIR/index.js" 2>/dev/null || stat -c%s "$WEBAPP_DIR/index.js" 2>/dev/null)
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

formatted=$(format_bytes "$total_wasm")
printf "%-30s %10s (%s bytes)\n" "Total WASM:" "$formatted" "$total_wasm"

formatted=$(format_bytes "$total_static")
printf "%-30s %10s (%s bytes)\n" "Total Static Assets:" "$formatted" "$total_static"

formatted=$(format_bytes "$total_js")
printf "%-30s %10s (%s bytes)\n" "Total JavaScript:" "$formatted" "$total_js"

grand_total=$((total_wasm + total_static + total_js))
formatted=$(format_bytes "$grand_total")
echo ""
printf "%-30s %10s (%s bytes)\n" "Grand Total:" "$formatted" "$grand_total"
