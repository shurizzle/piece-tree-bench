#!/bin/zsh
# SPDX-License-Identifier: GPL-3.0-or-later
# Full cross-library run: 3 rounds x 3 corpora x (system, mimalloc), alternating, results in $1.
# Usage: tools/run.sh <results dir>   (nothing else should be running: no builds, no other benchmarks)
# By default piece-tree comes from GitHub (the revision pinned in Cargo.lock). To measure a local, maybe
# uncommitted checkout set PIECE_TREE_PATH=<checkout>/crates/piece-tree; Cargo.lock is restored afterwards.
set -e
cd "$(dirname "$0")/.."
D=${1:?results dir}
mkdir -p "$D"
extra=()
if [ -n "$PIECE_TREE_PATH" ]; then
  extra=(--config "patch.\"https://github.com/shurizzle/piece-tree\".piece-tree.path=\"$PIECE_TREE_PATH\"")
  /bin/cp -f Cargo.lock "$D/Cargo.lock.saved"
  trap '/bin/cp -f "$D/Cargo.lock.saved" Cargo.lock' EXIT
fi
cargo build --release "${extra[@]}"
/bin/cp -f target/release/piece-tree-bench "$D/sys"
cargo build --release --features mimalloc "${extra[@]}"
/bin/cp -f target/release/piece-tree-bench "$D/mi"
for r in 1 2 3; do for c in synthetic rust cjk; do for b in sys mi; do
  CORPUS=$c "$D/$b" > "$D/w3-$c-$b-$r.txt" 2>&1
done; done; done
echo done > "$D/done"
