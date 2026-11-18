#!/usr/bin/env bash
set -euo pipefail

# Resolve repository root whether this script lives in ./ or ./scripts/
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -f "$SCRIPT_DIR/scripts/mdr.rs" ]]; then
    ROOT_DIR="$SCRIPT_DIR"
else
    ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
fi

SRC="$ROOT_DIR/scripts/mdr.rs"
BIN="${TMPDIR:-/tmp}/mdr-compiler"

CHECK_MODE=false
FORCE_BUILD=false
TARGET="README"

for arg in "$@"; do
    case "$arg" in
        --check|-c)
            CHECK_MODE=true
            ;;
        --rebuild|-r)
            FORCE_BUILD=true
            ;;
        --help|-h)
            echo "Usage: $(basename "$0") [TARGET] [--check|-c] [--rebuild|-r]"
            echo "  TARGET        Base name or file path (default: README)"
            echo "  --check, -c   Verify .md matches .mdr without overwriting"
            echo "  --rebuild, -r Force recompilation of scripts/mdr.rs"
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

# Allow passing "README", "README.mdr", or "README.md"
BASE="${TARGET%.mdr}"
BASE="${BASE%.md}"

# Fall back to repo root if invoked from a subdirectory
if [[ ! -f "${BASE}.mdr" && -f "$ROOT_DIR/${BASE}.mdr" ]]; then
    BASE="$ROOT_DIR/$BASE"
fi

INPUT="${BASE}.mdr"
OUTPUT="${BASE}.md"

if [[ ! -f "$INPUT" ]]; then
    echo "Error: source file '$INPUT' not found." >&2
    exit 1
fi

# Only recompile when mdr.rs is newer than the cached binary
if [[ "$FORCE_BUILD" == true || ! -x "$BIN" || "$SRC" -nt "$BIN" ]]; then
    rustc -O "$SRC" -o "$BIN"
fi

if [[ "$CHECK_MODE" == true ]]; then
    TMP_OUT="$(mktemp)"
    trap 'rm -f "$TMP_OUT"' EXIT
    "$BIN" "$INPUT" "$TMP_OUT"
    if ! diff -u "$OUTPUT" "$TMP_OUT"; then
        echo "Error: '$OUTPUT' is out of sync with '$INPUT'." >&2
        exit 1
    fi
    echo "OK: '$OUTPUT' is up to date."
else
    "$BIN" "$INPUT" "$OUTPUT"
    echo "Compiled $INPUT -> $OUTPUT"
fi
