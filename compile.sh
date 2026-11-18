#!/usr/bin/env bash
rustc -O scripts/mdr.rs -o mdr
./mdr $1.mdr $1.md
