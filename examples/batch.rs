// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: Text::apply_edits vs one edit at a time (from the end), by batch size and text shape.
use piece_tree::Text;
use std::{hint::black_box, ops::Range, time::Instant};

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }
}

fn sample(bytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < bytes { s.push_str(&format!("    let value_{i} = compute({i}, \"lorem ipsum ✓\");\n")); i += 1; }
    s
}

fn best<T>(mut f: impl FnMut() -> T) -> f64 {
    (0..15).map(|_| { let t = Instant::now(); black_box(f()); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min)
}

/// `k` sorted line edits (inserts and line deletes alternating) inside lines `[lo, hi)` of `t`.
fn make_edits(t: &Text, k: usize, lo: usize, hi: usize, seed: u64) -> Vec<(Range<usize>, &'static str)> {
    let mut r = Rng(seed);
    let mut lines: Vec<usize> = (0..k).map(|_| lo + r.below(hi - lo - 1)).collect();
    lines.sort_unstable();
    lines.dedup();
    lines.iter().enumerate().map(|(i, &l)| {
        let s = t.line_to_offset(l);
        if i % 2 == 0 { (s..s, "edit\n") } else { (s..t.line_to_offset(l + 1), "") }
    }).collect()
}

fn sequential(t: &Text, edits: &[(Range<usize>, &str)]) -> Text {
    let mut t = t.clone();
    for (r, s) in edits.iter().rev() {
        if !r.is_empty() { t = t.delete(r.clone()); }
        if !s.is_empty() { t = t.insert(r.start, s); }
    }
    t
}

fn main() {
    let base = Text::from(sample(10 << 20).as_str());
    // Fragmented: 3000 random edits first (~3000 pieces).
    let mut frag = base.clone();
    let mut r = Rng(1);
    for _ in 0..1500 {
        let off = frag.line_to_offset(r.below(frag.line_count()));
        frag = frag.insert(off, "edit\n");
        let line = r.below(frag.line_count() - 1);
        frag = frag.delete(frag.line_to_offset(line)..frag.line_to_offset(line + 1));
    }
    println!("pieces: base {}, fragmented {}", base.chunks().count(), frag.chunks().count());
    println!("{:<34} {:>6} {:>14} {:>14} {:>8}", "shape", "k", "sequential ns/e", "apply_edits ns/e", "speedup");
    for (name, t) in [("single piece, scattered", &base), ("fragmented, scattered", &frag)] {
        let n = t.line_count();
        for k in [1usize, 2, 4, 8, 16, 32, 128, 1000, 5000] {
            let edits = make_edits(t, k, 0, n, 99);
            let (seq, bat) = (sequential(t, &edits), t.apply_edits(&edits));
            assert_eq!(seq.to_string(), bat.to_string(), "{name} k={k}");
            let k = edits.len();
            let a = best(|| sequential(t, &edits)) * 1e9 / k as f64;
            let b = best(|| t.apply_edits(&edits)) * 1e9 / k as f64;
            println!("{name:<34} {k:>6} {a:>14.0} {b:>14.0} {:>7.2}x", a / b);
        }
    }
    for (name, t) in [("single piece, clustered 200 lines", &base), ("fragmented, clustered 200 lines", &frag)] {
        let mid = t.line_count() / 2;
        for k in [4usize, 16, 64] {
            let edits = make_edits(t, k, mid, mid + 200, 7);
            let (seq, bat) = (sequential(t, &edits), t.apply_edits(&edits));
            assert_eq!(seq.to_string(), bat.to_string(), "{name} k={k}");
            let k = edits.len();
            let a = best(|| sequential(t, &edits)) * 1e9 / k as f64;
            let b = best(|| t.apply_edits(&edits)) * 1e9 / k as f64;
            println!("{name:<34} {k:>6} {a:>14.0} {b:>14.0} {:>7.2}x", a / b);
        }
    }
}
