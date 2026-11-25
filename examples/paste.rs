// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: paste of N bytes (exponential line lengths, given mean) into a 10 MB text, ns per byte.
use piece_tree::Text;
use std::{hint::black_box, time::Instant};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn make_text(bytes: usize, mean: f64, r: &mut Rng) -> String {
    let mut s = String::with_capacity(bytes + 64);
    while s.len() < bytes {
        let u = (r.next() as f64 + 1.0) / (1u64 << 31) as f64;
        let l = (-mean * u.ln()) as usize;
        for _ in 0..l.min(bytes - s.len().min(bytes)) { s.push('x'); }
        s.push('\n');
    }
    s.truncate(bytes);
    s
}

fn main() {
    let mut r = Rng(5);
    let base = Text::from(make_text(10 << 20, 40.0, &mut r).as_str());
    println!("{:>8} {:>6} {:>10} {:>10}", "bytes", "mean", "ns/paste", "ns/byte");
    for n in [256usize, 1024, 4096, 65536] {
        for mean in [8.0f64, 20.0, 60.0, 400.0] {
            let pastes: Vec<String> = (0..64).map(|_| make_text(n, mean, &mut r)).collect();
            let offs: Vec<usize> = (0..64).map(|_| (r.next() as usize) % base.len()).collect();
            let reps = (4_000_000 / n).max(20);
            let best = (0..9).map(|_| {
                let mut t = base.clone();
                let t0 = Instant::now();
                for i in 0..reps {
                    let k = i % 64;
                    let off = t.floor_char_boundary(offs[k] % t.len());
                    t = t.insert(off, &pastes[k]);
                }
                black_box(&t);
                t0.elapsed().as_secs_f64() / reps as f64
            }).fold(f64::MAX, f64::min);
            println!("{n:>8} {mean:>6} {:>10.0} {:>10.3}", best * 1e9, best * 1e9 / n as f64);
        }
    }
}
