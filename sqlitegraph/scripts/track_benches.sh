#!/usr/bin/env bash
set -euo pipefail
cargo bench "$@"
stamp=$(date +"%F_%H-%M-%S")
dest="bench_history/${stamp}"
mkdir -p "$dest"
cp -r target/criterion "$dest/"
echo "Stored Criterion output in $dest"
