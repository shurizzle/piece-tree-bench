# piece-tree-bench

Cross-library benchmark harness for [piece-tree](https://github.com/shurizzle/piece-tree): the same scenarios over piece-tree, ropey 1.6.1,
ropey 2.0.0-beta.1 and Zed's `Rope` (on `sum_tree`), in one binary built with fat LTO and `codegen-units = 1`.

License: **GPL-3.0-or-later** (it vendors Zed's GPL `rope`), see `LICENSE` and `NOTICE.md`.

## The library under test

`piece-tree` is a git dependency (`https://github.com/shurizzle/piece-tree`, the revision pinned in `Cargo.lock`;
`cargo update -p piece-tree` moves it). To measure a local checkout, possibly with uncommitted changes, patch it in
for one build:

```sh
cargo run --release --config 'patch."https://github.com/shurizzle/piece-tree".piece-tree.path="../piece-tree/crates/piece-tree"'
```

or set `PIECE_TREE_PATH=<checkout>/crates/piece-tree` for `tools/run.sh`. (Cargo rewrites `Cargo.lock` for a patched
build; `tools/run.sh` restores it, for a manual build run `git checkout Cargo.lock`.)

## Run

```sh
cargo run --release                          # all scenarios, all libraries, system allocator
cargo run --release --features mimalloc      # mimalloc as the global allocator of the whole binary
CORPUS=synthetic|rust|cjk cargo run --release
ONLY="ropey 2 beta" cargo run --release      # one library
RUST_CORPUS_DIR=/path/to/rs/files CORPUS=rust cargo run --release
```

Output: a table (minimum and median ns/op per library, winner and lead), then a sanity check that every library ends
with the same text after the edit scenarios. `CORPUS=rust` reads `*.rs` files from `$RUST_CORPUS_DIR`, by default
`~/.cargo/registry/src` (~64 MB are used, wrapped when shorter); nothing of it is stored here.

A full comparison (3 runs per corpus, system allocator and mimalloc alternating) and its tables:

```sh
tools/run.sh <results dir>                   # ~25 minutes; nothing else should be running
python3 -I tools/tables.py <results dir> <output.md>
```

Noise: the same binary repeats within a few percent; two binaries built from different code can differ 5-10% by
code placement alone. Treat differences under ~10% as unproven and prefer alternating runs of two binaries.

## Other programs

- `examples/`: scratch programs used by the piece-tree benchmark notes: `edits.rs` / `edits_mut.rs` (tight A/B of the
  edit paths, persistent and in-place API), `batch.rs` (`apply_edits`), `load.rs`, `paste.rs`, `chars.rs`,
  `typing_mut.rs`, `typing_loop.rs` (a long loop to attach `sample` to), `nl.rs` and `swar.rs` (newline scan
  prototypes). They use piece-tree only.
- `w1proto/`: a standalone crate (own `Cargo.toml`) with the throwaway prototypes behind the wide-leaf and in-place
  estimates (`cargo run --release -- inplace`).
- `vendor/`: Zed's `sum_tree` and `rope` at commit `77228009dd267dcf8ab9a20eef5802b2a8ea4afa`, tests and tracing
  stripped; the changes are listed in `vendor/PATCHES.md`.

To A/B against an older piece-tree, copy its workspace to a standalone directory, patch it in as above (one harness
copy per version) and alternate the two binaries run by run.
