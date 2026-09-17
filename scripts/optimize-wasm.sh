#!/usr/bin/env bash
# Optimiza con binaryen (wasm-opt) los módulos generados por `trunk build`.
# Uso: scripts/optimize-wasm.sh [dist_dir]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="${1:-$ROOT/dist}"

WASM_OPT="${WASM_OPT:-}"
if [ -z "$WASM_OPT" ]; then
    WASM_OPT="$(command -v wasm-opt || true)"
fi
if [ -z "$WASM_OPT" ] && [ -x "$HOME/.local/bin/wasm-opt" ]; then
    WASM_OPT="$HOME/.local/bin/wasm-opt"
fi
if [ -z "$WASM_OPT" ] || [ ! -x "$WASM_OPT" ]; then
    echo "Error: wasm-opt no encontrado (instala binaryen v129 o define WASM_OPT)" >&2
    exit 1
fi

shopt -s nullglob
found=0
for wasm in "$WASM_DIR"/*_bg.wasm; do
    found=1
    before=$(stat -c%s "$wasm")
    echo "Optimizando $wasm ($before bytes)..."
    "$WASM_OPT" \
        --enable-bulk-memory \
        --enable-nontrapping-float-to-int \
        --enable-mutable-globals \
        --enable-sign-ext \
        -O3 "$wasm" -o "${wasm}.tmp"
    mv "${wasm}.tmp" "$wasm"
    after=$(stat -c%s "$wasm")
    echo "  -> $after bytes (reducción $(( (before - after) * 100 / before ))%)"
done

if [ "$found" -eq 0 ]; then
    echo "No se encontraron archivos *_bg.wasm en $WASM_DIR" >&2
    exit 1
fi
