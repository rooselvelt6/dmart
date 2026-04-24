#!/bin/bash
# Post-build WASM optimization script
# Run after: trunk build

WASM_DIR="dist"
WASM_OPT="$HOME/.local/bin/wasm-opt"

if [ ! -f "$WASM_OPT" ]; then
    echo "Error: wasm-opt not found. Install binaryen v129 first."
    exit 1
fi

cd "$(dirname "$0")"

for wasm in $WASM_DIR/*_bg.wasm; do
    if [ -f "$wasm" ]; then
        echo "Optimizing $wasm..."
        $WASM_OPT --enable-bulk-memory --enable-gc -O3 "$wasm" -o "${wasm}.tmp"
        mv "${wasm}.tmp" "$wasm"
        size=$(ls -lh "$wasm" | awk '{print $5}')
        echo "Done: $wasm ($size)"
    fi
done