// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: tight A/B of the edit hot paths (piece-tree only), min over many reps.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use piece_tree::Text;
use std::{hint::black_box, time::Instant};

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

fn best(reps: usize, ops: usize, mut f: impl FnMut(&mut Rng)) -> f64 {
    (0..reps).map(|r| { let mut rng = Rng(r as u64 + 1); let t = Instant::now(); f(&mut rng); t.elapsed().as_nanos() as f64 / ops as f64 }).fold(f64::MAX, f64::min)
}

fn main() {
    let base = Text::from(sample(10 << 20).as_str());
    let small = Text::from(sample(1 << 20).as_str());
    let paste = best(40, 1000, |rng| {
        let mut t = base.clone();
        for _ in 0..1000 {
            let off = t.line_to_offset(rng.below(t.line_count() - 1)) + 4;
            t.insert_mut(off, "paste\n");
        }
        black_box(t);
    });
    let edits = best(40, 2000, |rng| {
        let mut t = base.clone();
        for _ in 0..1000 {
            let off = t.line_to_offset(rng.below(t.line_count()));
            t.insert_mut(off, "edit\n");
            let line = rng.below(t.line_count() - 1);
            let r = t.line_to_offset(line)..t.line_to_offset(line + 1);
            t.delete_mut(r);
        }
        black_box(t);
    });
    let typing = best(60, 10_000, |_| {
        let mut t = small.clone();
        let start = t.line_to_offset(t.line_count() / 2);
        for (i, off) in (start..start + 10_000).enumerate() {
            t.insert_mut(off, if i % 40 == 39 { "\n" } else { "x" });
        }
        black_box(t);
    });
    let start = small.line_to_offset(small.line_count() / 2);
    let mut typed = small.clone();
    for (i, off) in (start..start + 10_000).enumerate() {
        typed = typed.insert(off, if i % 40 == 39 { "\n" } else { "x" });
    }
    let end = start + 10_000;
    let backspace = best(60, 10_000, |_| {
        let mut t = typed.clone();
        for off in (end - 10_000..end).rev() {
            t.delete_mut(off..off + 1);
        }
        black_box(t);
    });
    let lines = best(40, typed.line_count() * 20, |_| {
        let mut sum = 0;
        for _ in 0..20 {
            for i in 0..typed.line_count() {
                sum += typed.line(i).len();
            }
        }
        black_box(sum);
    });
    println!("paste {paste:.1}  edits {edits:.1}  typing {typing:.1}  backspace {backspace:.1}  line {lines:.1}");
}
