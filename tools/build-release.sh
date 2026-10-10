#!/usr/bin/env bash
# Builds a self-contained release bundle in ./dist that can be uploaded to any
# folder of any static web server (no base path configuration needed).
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=target/dx/ashen-web/release/web/public
rm -rf "$OUT" dist
dx build --platform web -p ashen-web --release
# dx emits a root-absolute wasm URL ("/./assets/..."); make it page-relative.
sed -i 's#"/\./assets/#"./assets/#g; s#"/assets/#"./assets/#g' "$OUT"/assets/*.js "$OUT"/index.html
if grep -qE '"/(\./)?assets/' "$OUT"/assets/*.js "$OUT"/index.html; then
    echo "error: absolute asset paths remain" >&2
    exit 1
fi
cp -r "$OUT" dist
echo "Release bundle ready in ./dist ($(du -sh dist | cut -f1)):"
find dist -type f | sort
