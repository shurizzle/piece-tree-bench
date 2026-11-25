// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: typing forever (for `sample`), 1 MB text, 10k keys per round, old version dropped each key.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use piece_tree::Text;
use std::{hint::black_box, time::{Duration, Instant}};

fn main() {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < (1 << 20) { s.push_str(&format!("    let value_{i} = compute({i}, \"lorem ipsum ✓\");\n")); i += 1; }
    let small = Text::from(s.as_str());
    let start = small.line_to_offset(small.line_count() / 2);
    let secs: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(20);
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        let mut t = small.clone();
        for (i, off) in (start..start + 10_000).enumerate() {
            t = t.insert(off, if i % 40 == 39 { "\n" } else { "x" });
        }
        black_box(&t);
    }
}
