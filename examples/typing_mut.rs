// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: typing 10k keys in a 1 MB text: `insert` (path copy) against `insert_mut` (in place when
//! unshared), and `Document` typing inside a group. ns per key, min over reps.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use piece_tree::{Document, Text};
use std::{hint::black_box, time::Instant};

fn best(reps: usize, ops: usize, mut f: impl FnMut()) -> f64 {
    (0..reps).map(|_| { let t = Instant::now(); f(); t.elapsed().as_nanos() as f64 / ops as f64 }).fold(f64::MAX, f64::min)
}

fn main() {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < (1 << 20) { s.push_str(&format!("    let value_{i} = compute({i}, \"lorem ipsum ✓\");\n")); i += 1; }
    let small = Text::from(s.as_str());
    let start = small.line_to_offset(small.line_count() / 2);
    let key = |i: usize| if i % 40 == 39 { "\n" } else { "x" };
    let copy = best(80, 10_000, || {
        let mut t = small.clone();
        for (i, off) in (start..start + 10_000).enumerate() { t = t.insert(off, key(i)); }
        black_box(t);
    });
    let in_place = best(80, 10_000, || {
        let mut t = small.clone();
        for (i, off) in (start..start + 10_000).enumerate() { t.insert_mut(off, key(i)); }
        black_box(t);
    });
    let base_doc = Document::new(&s);
    let doc_plain = best(40, 10_000, || {
        let mut d = Document::new("");
        d.insert(0, "seed");
        d = base_doc_clone(&base_doc);
        for (i, off) in (start..start + 10_000).enumerate() { d.insert(off, key(i)); }
        black_box(d);
    });
    let doc_group = best(40, 10_000, || {
        let mut d = base_doc_clone(&base_doc);
        d.begin_group();
        for (i, off) in (start..start + 10_000).enumerate() { d.insert(off, key(i)); }
        d.end_group();
        black_box(d);
    });
    println!("Text insert {copy:.1}  insert_mut {in_place:.1}  | Document no group {doc_plain:.1}  in a group {doc_group:.1}  (ns/key)");
}

fn base_doc_clone(d: &Document) -> Document {
    let mut n = Document::new("");
    n.insert(0, &d.text().to_string());
    n
}
