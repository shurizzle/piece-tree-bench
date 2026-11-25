// SPDX-License-Identifier: GPL-3.0-or-later
//! Same scenarios as `piece-tree/crates/piece-tree/benches/bench.rs`, run over every adapter in this
//! one binary (same profile, same allocator). Each scenario runs `REPS` times with seeds 1..=REPS, so
//! every library sees the same edits, and reports the minimum and the median.
//! Run: `cargo run --release`.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;


mod adapters;

use std::{
    collections::hash_map::DefaultHasher,
    hash::Hasher,
    hint::black_box,
    time::{Duration, Instant},
};

use adapters::{Buffer, Ropey1, Ropey2, Zed};

const REPS: usize = 7;
const READ_PASSES: usize = 50;

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }
}

/// Text of about `bytes` bytes. `CORPUS` picks the flavor:
/// - `synthetic` (default): source-like lines with one multibyte char each;
/// - `rust`: real `.rs` files from `$RUST_CORPUS_DIR`, default `~/.cargo/registry/src` (variable line lengths, no `\r`);
/// - `cjk`: lines dominated by 3-byte chars, where byte/char conversions matter.
fn corpus_kind() -> &'static str {
    static KIND: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    KIND.get_or_init(|| std::env::var("CORPUS").unwrap_or_else(|_| "synthetic".into()))
}

/// Where the `rust` corpus is read from: `RUST_CORPUS_DIR`, or the cargo registry sources.
fn rust_corpus_dir() -> std::path::PathBuf {
    std::env::var_os("RUST_CORPUS_DIR").map(Into::into).unwrap_or_else(|| {
        std::path::PathBuf::from(std::env::var("HOME").expect("HOME")).join(".cargo/registry/src")
    })
}

fn read_rust_files(limit: usize) -> String {
    let mut s = String::new();
    let mut stack = vec![rust_corpus_dir()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten().collect();
        entries.sort_by_key(|e| e.path());
        for e in entries {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                // Skip files with `\r` (the libraries normalize or count it differently) and non-UTF-8.
                if let Ok(t) = std::fs::read_to_string(&path)
                    && !t.contains('\r')
                {
                    s.push_str(&t);
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                }
            }
            if s.len() >= limit {
                return s;
            }
        }
    }
    assert!(!s.is_empty(), "no .rs files under {}", rust_corpus_dir().display());
    s
}

fn sample(bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 64);
    match corpus_kind() {
        "rust" => {
            // Read once: the registry can change while the benchmark runs, and every library
            // must see byte-identical input.
            static ALL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            let all = ALL.get_or_init(|| read_rust_files(64 << 20));
            let mut end = bytes.min(all.len());
            // Cut at a line boundary so the prefix stays valid UTF-8.
            if end < all.len() {
                end += all.as_bytes()[end..].iter().position(|&b| b == b'\n').map_or(all.len() - end, |i| i + 1);
            }
            while s.len() < bytes {
                s.push_str(&all[..end]);
            }
        }
        "cjk" => {
            let mut i = 0;
            while s.len() < bytes {
                s.push_str(&format!("    let 変数_{i} = 計算({i}, \"日本語のテキストと漢字かな交じり文\");\n"));
                i += 1;
            }
        }
        _ => {
            let mut i = 0;
            while s.len() < bytes {
                s.push_str(&format!("    let value_{i} = compute({i}, \"lorem ipsum ✓\");\n"));
                i += 1;
            }
        }
    }
    s
}

#[derive(Clone, Copy)]
struct Cell {
    min: f64,
    median: f64,
    /// Hash of the resulting text, when the scenario produces one.
    digest: Option<u64>,
}

fn digest<B: Buffer>(b: &B) -> u64 {
    // `Hasher::write` is streaming, so the result does not depend on chunk boundaries.
    let mut h = DefaultHasher::new();
    b.for_each_chunk(&mut |c| h.write(c.as_bytes()));
    h.finish() ^ b.len() as u64
}

/// Runs `f` `REPS` times with a fresh seed each and returns min/median ns/op and the last output.
fn bench<T>(ops: usize, mut f: impl FnMut(&mut Rng) -> T) -> (Cell, T) {
    let mut times: Vec<Duration> = Vec::with_capacity(REPS);
    let mut out = None;
    for rep in 0..REPS {
        let mut rng = Rng(rep as u64 + 1);
        let start = Instant::now();
        out = Some(black_box(f(&mut rng)));
        times.push(start.elapsed());
    }
    times.sort();
    let per_op = |d: Duration| d.as_nanos() as f64 / ops as f64;
    let cell = Cell { min: per_op(times[0]), median: per_op(times[REPS / 2]), digest: None };
    (cell, out.expect("REPS > 0"))
}

fn bench_text<B: Buffer>(ops: usize, f: impl FnMut(&mut Rng) -> B) -> (Cell, B) {
    let (mut cell, out) = bench(ops, f);
    cell.digest = Some(digest(&out));
    (cell, out)
}

/// Passes over every line, so each rep runs for milliseconds rather than microseconds.
fn read_all_lines<B: Buffer>(t: &B) -> Cell {
    let n = t.line_count();
    bench(n * READ_PASSES, |_| (0..READ_PASSES).map(|_| (0..n).map(|i| t.line_len(i)).sum::<usize>()).sum::<usize>()).0
}

fn type_into<B: Buffer>(base: &B) -> B {
    let mut t = base.clone();
    let start = t.line_to_byte(t.line_count() / 2);
    for (i, off) in (start..start + 10_000).enumerate() {
        t.insert(off, if i % 40 == 39 { "\n" } else { "x" });
    }
    t
}

/// One column of the table: every scenario for one library, in a fixed order.
fn column<B: Buffer>() -> Vec<Cell> {
    eprintln!("running {}", B::NAME);
    let mut out = Vec::new();
    let big = sample(50 << 20);

    out.push(bench(big.len(), |_| B::from_str(&big)).0);
    let mut owned = vec![big.clone(); REPS];
    out.push(bench(big.len(), |_| B::from_string(owned.pop().expect("one per rep"))).0);
    drop(owned);
    let loaded = B::from_str(&big);
    drop(big);
    out.push(
        bench(100_000, |rng| (0..100_000).map(|_| loaded.line_len(rng.below(loaded.line_count()))).sum::<usize>()).0,
    );
    drop(loaded);

    let base = B::from_str(&sample(1 << 20));
    let (cell, typed) = bench_text(10_000, |_| type_into(&base));
    out.push(cell);
    // Undo-like: keep every intermediate version alive (clone per key).
    out.push(
        bench(10_000, |_| {
            let mut t = base.clone();
            let start = t.line_to_byte(t.line_count() / 2);
            let mut history = Vec::with_capacity(10_000);
            for (i, off) in (start..start + 10_000).enumerate() {
                history.push(t.clone());
                t.insert_kept(off, if i % 40 == 39 { "\n" } else { "x" });
            }
            (t, history)
        })
        .0,
    );
    out.push(read_all_lines(&typed));
    out.push(
        bench(50_000, |rng| {
            let mut sum = 0;
            for _ in 0..1_000 {
                let start = rng.below(typed.line_count() - 50);
                sum += (start..start + 50).map(|i| typed.line_len(i)).sum::<usize>();
            }
            sum
        })
        .0,
    );

    // Backspace over the typed text: deletes inside the typed region.
    let end = base.line_to_byte(base.line_count() / 2) + 10_000;
    out.push(
        bench_text(10_000, |_| {
            let mut t = typed.clone();
            for off in (end - 10_000..end).rev() {
                t.delete(off..off + 1);
            }
            t
        })
        .0,
    );
    out.push(
        bench(100_000, |rng| (0..100_000).map(|_| typed.byte_to_line(typed.len() / 2 + rng.below(1 << 16))).sum::<usize>())
            .0,
    );
    drop((base, typed));

    let base = B::from_str(&sample(10 << 20));
    out.push(
        bench_text(1_000, |rng| {
            let mut t = base.clone();
            for _ in 0..1_000 {
                // Synthetic lines start with 4 ASCII spaces (inside the piece); other corpora: line start.
                let skip = if corpus_kind() == "synthetic" { 4 } else { 0 };
                let off = t.line_to_byte(rng.below(t.line_count() - 1)) + skip;
                t.insert(off, "paste\n");
            }
            t
        })
        .0,
    );
    let (cell, edited) = bench_text(2_000, |rng| {
        let mut t = base.clone();
        for _ in 0..1_000 {
            let off = t.line_to_byte(rng.below(t.line_count()));
            t.insert(off, "edit\n");
            let line = rng.below(t.line_count() - 1);
            let range = t.line_to_byte(line)..t.line_to_byte(line + 1);
            t.delete(range);
        }
        t
    });
    out.push(cell);
    out.push(read_all_lines(&edited));

    // Batch: the same ~1000 sorted, non-overlapping edits (line inserts and line deletes) given in
    // original offsets. Zed applies them in one cursor pass; the others one by one from the end.
    let mut r = Rng(99);
    let mut lines: Vec<usize> = (0..1_000).map(|_| r.below(base.line_count() - 1)).collect();
    lines.sort_unstable();
    lines.dedup();
    let edits: Vec<(std::ops::Range<usize>, &str)> = lines
        .iter()
        .enumerate()
        .map(|(k, &l)| {
            let s = base.line_to_byte(l);
            if k % 2 == 0 { (s..s, "edit\n") } else { (s..base.line_to_byte(l + 1), "") }
        })
        .collect();
    out.push(
        bench_text(edits.len(), |_| {
            let mut t = base.clone();
            t.apply_edits(&edits);
            t
        })
        .0,
    );
    out
}

const ROWS: [&str; 13] = [
    "load 50MB from &str (per byte)",
    "load 50MB from String (per byte)",
    "line() random, loaded",
    "typing 10k chars in 1MB",
    "typing 10k, clone per key",
    "line() all, after typing",
    "line() 10x50 windows, typed",
    "backspace 10k",
    "offset_to_line random, typed",
    "paste mid-piece 1k on 10MB",
    "random edits 1k+1k on 10MB",
    "line() all, after edits",
    "batch 1k edits, one pass, 10MB",
];

fn main() {
    let names = [piece_tree::Text::NAME, Ropey1::NAME, Ropey2::NAME, Zed::NAME];
    // `ONLY=<name>` runs a single library (others print 0), to rule out cross-library interference.
    let only = std::env::var("ONLY").ok();
    let run = |name: &str, f: fn() -> Vec<Cell>| match &only {
        Some(o) if o != name => vec![Cell { min: 0.0, median: 0.0, digest: None }; ROWS.len()],
        _ => f(),
    };
    let cols = [
        run(names[0], column::<piece_tree::Text>),
        run(names[1], column::<Ropey1>),
        run(names[2], column::<Ropey2>),
        run(names[3], column::<Zed>),
    ];
    assert!(cols.iter().all(|c| c.len() == ROWS.len()), "row labels out of sync with scenarios");

    println!("corpus: {}; ns/op, min (median) of {REPS} runs\n", corpus_kind());
    print!("| {:<32} |", "scenario");
    for n in names {
        print!(" {n:<20} |");
    }
    println!(" {:<26} |", "winner (lead over 2nd)");
    println!("|{:-<34}|{}{:-<28}|", "", format!("{:-<22}|", "").repeat(names.len()), "");
    for (r, row) in ROWS.iter().enumerate() {
        print!("| {row:<32} |");
        for col in &cols {
            let c = col[r];
            print!(" {:<20} |", format!("{:.1} ({:.1})", c.min, c.median));
        }
        // Lowest min wins; the lead is the runner-up's min divided by the winner's.
        let best = (0..cols.len()).min_by(|&a, &b| cols[a][r].min.total_cmp(&cols[b][r].min)).expect("columns");
        let second = (0..cols.len()).filter(|&i| i != best).map(|i| cols[i][r].min).fold(f64::MAX, f64::min);
        println!(" {:<26} |", format!("{} ({:.2}x)", names[best], second / cols[best][r].min));
    }

    // Every library must have produced the same text for the same edits.
    let mut ok = true;
    for (r, row) in ROWS.iter().enumerate() {
        // Columns skipped with `ONLY` have no digest and are left out of the comparison.
        let ds: Vec<_> = cols.iter().filter_map(|c| c[r].digest).collect();
        if ds.windows(2).any(|w| w[0] != w[1]) {
            ok = false;
            println!("\nMISMATCH in '{row}': digests {ds:x?}");
        }
    }
    println!("\nsanity check (same text after every edit scenario): {}", if ok { "ok" } else { "FAILED" });
}
