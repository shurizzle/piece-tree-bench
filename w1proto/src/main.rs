// SPDX-License-Identifier: GPL-3.0-or-later
//! Throwaway estimate for W1: persistent insert and lookup cost of the current binary AVL (one
//! piece per node) against a persistent B-tree with wide leaves, same Piece, same Arc, same allocator.
mod avl;
mod inplace;
mod piece;
mod wide;

use piece::{Chunk, Piece};
use std::{hint::black_box, time::Instant};
use triomphe::Arc;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static A: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn frac(&mut self) -> f64 {
        self.next() as f64 / (1u64 << 31) as f64
    }
}

trait Tree: Sized {
    fn new(base: Piece) -> Self;
    fn insert(&self, off: usize, p: Piece) -> Self;
    fn len(&self) -> usize;
    fn lf_before(&self, off: usize) -> usize;
    fn extend(&self, off: usize, grow: u32) -> Option<Self>;
    fn extend_mut(&mut self, off: usize, grow: u32) -> bool;
    fn shape(&self) -> String;
}

struct Avl(avl::Tree);
impl Tree for Avl {
    fn new(base: Piece) -> Self {
        Avl(avl::insert_at(&None, 0, base))
    }
    fn insert(&self, off: usize, p: Piece) -> Self {
        Avl(avl::insert_at(&self.0, off, p))
    }
    fn len(&self) -> usize {
        avl::len(&self.0)
    }
    fn lf_before(&self, off: usize) -> usize {
        avl::lf_before(&self.0, off)
    }
    fn extend_mut(&mut self, off: usize, grow: u32) -> bool {
        avl::extend_in_place(&mut self.0, off, grow)
    }
    fn extend(&self, off: usize, grow: u32) -> Option<Self> {
        avl::extend_at(&self.0, off, grow).map(Avl)
    }
    fn shape(&self) -> String {
        format!("{} nodes, height {}, node {} B", avl::count(&self.0), avl::height(&self.0), std::mem::size_of::<avl::Node>() + 8)
    }
}

struct Wide<const L: usize, const F: usize>(Arc<wide::Node<L, F>>);
impl<const L: usize, const F: usize> Tree for Wide<L, F> {
    fn new(base: Piece) -> Self {
        Wide(wide::leaf_single(base))
    }
    fn insert(&self, off: usize, p: Piece) -> Self {
        Wide(wide::insert(&self.0, off, p))
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn lf_before(&self, off: usize) -> usize {
        wide::lf_before(&self.0, off)
    }
    fn extend_mut(&mut self, _off: usize, _grow: u32) -> bool {
        false
    }
    fn extend(&self, off: usize, grow: u32) -> Option<Self> {
        wide::extend(&self.0, off, grow).map(Wide)
    }
    fn shape(&self) -> String {
        let (p, l, i) = wide::stats(&self.0);
        format!("{p} pieces, {l} leaves + {i} inner, leaf {} B, inner {} B", std::mem::size_of::<wide::Leaf<L>>() + 24, std::mem::size_of::<wide::Inner<L, F>>() + 24)
    }
}

fn warmed<T: Tree>(chunk: &Arc<Chunk>, inserts: usize) -> T {
    let mut rng = Rng(7);
    let mut t = T::new(Piece::new(chunk, 0, 10 << 20));
    for _ in 0..inserts {
        let off = (rng.frac() * t.len() as f64) as usize;
        t = t.insert(off, Piece::new(chunk, 0, 40));
    }
    assert_eq!(t.len(), (10 << 20) + 40 * inserts, "totals must add up");
    t
}

/// ns per insert (min over rounds): each round starts from the warmed tree and drops the
/// previous version at every step, like `t = t.insert(..)` in the paste scenario.
fn bench_insert<T: Tree>(chunk: &Arc<Chunk>, base: &T) -> f64 {
    let mut rng = Rng(99);
    let offs: Vec<f64> = (0..512).map(|_| rng.frac()).collect();
    let rounds = 40;
    (0..rounds)
        .map(|_| {
            let mut t = T::insert(base, 0, Piece::new(chunk, 0, 40));
            let t0 = Instant::now();
            for f in &offs {
                let off = (f * t.len() as f64) as usize;
                t = t.insert(off, Piece::new(chunk, 0, 40));
            }
            let e = t0.elapsed().as_secs_f64() / offs.len() as f64;
            black_box(&t);
            e
        })
        .fold(f64::MAX, f64::min)
        * 1e9
}

/// Typing proxy: a piece of 1 byte in the middle, grown by 1 byte per key (`extend_at`).
fn bench_typing<T: Tree>(chunk: &Arc<Chunk>, base: &T) -> f64 {
    let cursor = base.len() / 2;
    let rounds = 40;
    (0..rounds)
        .map(|_| {
            let mut t = base.insert(cursor, Piece::new(chunk, 0, 1));
            let mut end = cursor + 1;
            let keys = 512;
            let t0 = Instant::now();
            for _ in 0..keys {
                t = t.extend(end, 1).expect("the piece ends at the cursor");
                end += 1;
            }
            let e = t0.elapsed().as_secs_f64() / keys as f64;
            assert_eq!(t.len(), base.len() + 1 + keys);
            black_box(&t);
            e
        })
        .fold(f64::MAX, f64::min)
        * 1e9
}

/// Typing on an unshared tree: `extend_mut`, falling back to the copy when the path is shared.
fn bench_typing_mut<T: Tree>(chunk: &Arc<Chunk>, base: &T) -> f64 {
    let cursor = base.len() / 2;
    (0..40)
        .map(|_| {
            let mut t = base.insert(cursor, Piece::new(chunk, 0, 1));
            let mut end = cursor + 1;
            let keys = 512;
            let t0 = Instant::now();
            for _ in 0..keys {
                if !t.extend_mut(end, 1) {
                    t = t.extend(end, 1).expect("the piece ends at the cursor");
                }
                end += 1;
            }
            let e = t0.elapsed().as_secs_f64() / keys as f64;
            black_box(&t);
            e
        })
        .fold(f64::MAX, f64::min)
        * 1e9
}

fn bench_lookup<T: Tree>(t: &T) -> f64 {
    let mut rng = Rng(5);
    let offs: Vec<usize> = (0..100_000).map(|_| (rng.frac() * t.len() as f64) as usize).collect();
    (0..9)
        .map(|_| {
            let t0 = Instant::now();
            let mut s = 0;
            for &o in &offs {
                s += t.lf_before(o);
            }
            black_box(s);
            t0.elapsed().as_secs_f64() / offs.len() as f64
        })
        .fold(f64::MAX, f64::min)
        * 1e9
}

fn row<T: Tree>(name: &str, chunk: &Arc<Chunk>, sizes: &[usize]) {
    let mut cells = Vec::new();
    let mut shape = String::new();
    for &n in sizes {
        let t: T = warmed(chunk, n / 2);
        if n == sizes[1] {
            shape = t.shape();
        }
        cells.push(format!("{:>5.0} / {:>4.0} / {:>4.0} / {:>5.1}", bench_insert(chunk, &t), bench_typing(chunk, &t), bench_typing_mut(chunk, &t), bench_lookup(&t)));
    }
    println!("{name:<14} {}   [{shape}]", cells.join("   "));
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("inplace") {
        inplace::run();
        return;
    }
    let chunk = Arc::new(Chunk { _pad: [0; 16] });
    let sizes = [0usize, 1_000, 3_000, 30_000];
    println!("insert ns / typing (copy) ns / typing (unshared, in place) ns / lookup ns, by piece count {sizes:?}");
    row::<Avl>("AVL (now)", &chunk, &sizes);
    row::<Wide<4, 4>>("wide L4 F4", &chunk, &sizes);
    row::<Wide<8, 8>>("wide L8 F8", &chunk, &sizes);
    row::<Wide<16, 8>>("wide L16 F8", &chunk, &sizes);
    row::<Wide<16, 16>>("wide L16 F16", &chunk, &sizes);
}
