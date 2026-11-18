#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    ROOT_DIR="$SCRIPT_DIR"
else
    ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
fi

MANIFEST="$ROOT_DIR/Cargo.toml"
BIN="$ROOT_DIR/target/release/mdr"

CHECK_MODE=false
REFRESH_IMAGES=false
TARGET="README"

for arg in "$@"; do
    case "$arg" in
        --check|-c)
            CHECK_MODE=true
            ;;
        --refresh-images)
            REFRESH_IMAGES=true
            ;;
        --help|-h)
            echo "Usage: $(basename "$0") [TARGET] [OPTIONS]"
            echo "  TARGET             Base name or file path (default: README)"
            echo "  --check, -c        Verify .md and local images match .mdr without modifying files"
            echo "  --refresh-images   Re-download all external images and live fallback snapshots"
            exit 0
            ;;
        -*)
            echo "Unknown option: $arg" >&2
            exit 1
            ;;
        *)
            TARGET="$arg"
            ;;
    esac
done

BASE="${TARGET%.mdr}"
BASE="${BASE%.md}"

if [[ ! -f "${BASE}.mdr" && -f "$ROOT_DIR/${BASE}.mdr" ]]; then
    BASE="$ROOT_DIR/$BASE"
fi

INPUT="${BASE}.mdr"
OUTPUT="${BASE}.md"
IMAGES_DIR="$(cd "$(dirname "$OUTPUT")" && pwd)/images/mdr"

if [[ ! -f "$INPUT" ]]; then
    echo "Error: source file '$INPUT' not found." >&2
    exit 1
fi

cargo build --release --quiet --manifest-path "$MANIFEST"

EXTRA_FLAGS=(--images-dir "$IMAGES_DIR")
if [[ "$REFRESH_IMAGES" == true ]]; then
    EXTRA_FLAGS+=(--refresh-images)
fi

if [[ "$CHECK_MODE" == true ]]; then
    cargo test --quiet --manifest-path "$MANIFEST"

    TMP_OUT="$(mktemp)"
    trap 'rm -f "$TMP_OUT"' EXIT
    "$BIN" "$INPUT" "$TMP_OUT" --no-fetch "${EXTRA_FLAGS[@]}"

    if ! diff -u "$OUTPUT" "$TMP_OUT"; then
        echo "Error: '$OUTPUT' is out of sync with '$INPUT'." >&2
        exit 1
    fi

    while IFS= read -r img_path; do
        if [[ ! -f "$(dirname "$OUTPUT")/$img_path" ]]; then
            echo "Error: missing localized image '$img_path'. Run scripts/build.sh." >&2
            exit 1
        fi
    done < <(grep -oE 'images/mdr/[a-zA-Z0-9._-]+' "$TMP_OUT" | sort -u)

    echo "OK: '$OUTPUT' and local images are up to date."
else
    "$BIN" "$INPUT" "$OUTPUT" "${EXTRA_FLAGS[@]}"
    echo "Compiled $INPUT -> $OUTPUT (assets in $IMAGES_DIR)"
fi
