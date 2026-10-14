#!/usr/bin/env bash
# Rebuild the wasm module into viewer/pkg.
# Usage: ./build.sh [--test] [--serve]
set -euo pipefail

cd "$(dirname "$0")"
export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"

run_tests=0
serve=0
for arg in "$@"; do
  case "$arg" in
    --test)  run_tests=1 ;;
    --serve) serve=1 ;;
    *) echo "unknown option: $arg" >&2; exit 1 ;;
  esac
done

if [ "$run_tests" -eq 1 ]; then
  cargo test
fi

python3 -I art/tools/export_textures.py viewer/textures.json

wasm-pack build crates/web --target web --release --out-dir ../../viewer/pkg

wasm=viewer/pkg/terrain_web_bg.wasm
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int "$wasm" -o "$wasm"

raw=$(wc -c < "$wasm" | tr -d ' ')
gz=$(gzip -9 -c "$wasm" | wc -c | tr -d ' ')
echo "built $wasm: $raw bytes raw, $gz bytes gzipped"

if [ "$serve" -eq 1 ]; then
  echo "serving viewer at http://localhost:8000"
  exec python3 -m http.server -d viewer 8000
fi
