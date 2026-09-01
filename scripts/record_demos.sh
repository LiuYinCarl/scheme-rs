#!/bin/bash
# Record all demo scripts (scripts/demos/*.demo) into docs/screenshots/.
#
# Requires: asciinema, agg, python3, a built target/release/scheme-rs.
# agg 未安装时可用 AGG_BIN 指定（如 AGG_BIN=/tmp/agg-bin/agg）。
#
# Per demo NAME.demo the pipeline is:
#   record_demo.py drives the REPL in a pty -> asciinema .cast
#   -> agg renders docs/screenshots/NAME.gif
#
# Useful overrides: DEMO_SIZE, AGG_THEME, AGG_SPEED, AGG_FONT
# （本机 agg 默认字体栈找不到字体时，设 AGG_FONT="Source Code Pro" 之类）。
set -euo pipefail
cd "$(dirname "$0")/.."

IMG=docs/screenshots
OUT=/tmp/scheme-rs-demo/out
SIZE="${DEMO_SIZE:-90x24}"
THEME="${AGG_THEME:-monokai}"
SPEED="${AGG_SPEED:-1.3}"
ORDER="repl callcc stdlib"
AGG="${AGG_BIN:-agg}"
FONT_ARGS=()
[ -n "${AGG_FONT:-}" ] && FONT_ARGS=(--text-font-family "$AGG_FONT")

for tool in asciinema "$AGG"; do
    command -v "$tool" >/dev/null || { echo "error: $tool not installed" >&2; exit 1; }
done
[ -x target/release/scheme-rs ] || { echo "error: run cargo build --release first" >&2; exit 1; }

mkdir -p "$OUT" "$IMG"

for name in $ORDER; do
    echo "== recording $name"
    asciinema rec --headless --overwrite --quiet --window-size "$SIZE" \
        --idle-time-limit 2 \
        --command "python3 scripts/record_demo.py scripts/demos/$name.demo" \
        "$OUT/$name.cast"
    "$AGG" --theme "$THEME" --speed "$SPEED" --last-frame-duration 2.5 \
        "${FONT_ARGS[@]+"${FONT_ARGS[@]}"}" \
        "$OUT/$name.cast" "$IMG/$name.gif" >/dev/null
done
echo "done -> $IMG"
