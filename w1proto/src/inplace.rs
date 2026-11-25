// SPDX-License-Identifier: GPL-3.0-or-later
//! W1b estimate: insert and delete in place (copy-on-write) against the persistent path copy,
//! on the AVL of tree.rs. Scenarios of the harness: paste (insert mid-piece), random edits
//! (insert + delete of ~40 bytes), backspace (trim a piece's end).
use crate::avl::{self, Tree};
use crate::piece::{Chunk, Piece};
use std::{hint::black_box, time::Instant};
use triomphe::Arc;

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

fn warmed(chunk: &Arc<Chunk>, inserts: usize) -> Tree {
    let mut rng = Rng(7);
    let mut t = avl::insert_at(&None, 0, Piece::new(chunk, 0, 10 << 20));
    for _ in 0..inserts {
        let off = (rng.frac() * avl::len(&t) as f64) as usize;
        t = avl::insert_at(&t, off, Piece::new(chunk, 0, 40));
    }
    t
}

fn min_ns(rounds: usize, ops: usize, mut f: impl FnMut() -> f64) -> f64 {
    let _ = ops;
    (0..rounds).map(|_| f()).fold(f64::MAX, f64::min) * 1e9
}

const OPS: usize = 512;

fn offsets(seed: u64) -> Vec<f64> {
    let mut r = Rng(seed);
    (0..OPS).map(|_| r.frac()).collect()
}

/// paste: `t = t.insert(random off, 6-byte piece)`.
fn paste(chunk: &Arc<Chunk>, base: &Tree) -> [f64; 4] {
    let offs = offsets(1);
    let persistent = min_ns(30, OPS, || {
        let mut t = avl::insert_at(base, 0, Piece::new(chunk, 0, 6));
        let t0 = Instant::now();
        for f in &offs {
            t = avl::insert_at(&t, (f * avl::len(&t) as f64) as usize, Piece::new(chunk, 0, 6));
        }
        black_box(&t);
        t0.elapsed().as_secs_f64() / OPS as f64
    });
    let unique = min_ns(30, OPS, || {
        let mut t = avl::insert_at(base, 0, Piece::new(chunk, 0, 6));
        let t0 = Instant::now();
        for f in &offs {
            let off = (f * avl::len(&t) as f64) as usize;
            avl::insert_mut(&mut t, off, Piece::new(chunk, 0, 6));
        }
        black_box(&t);
        t0.elapsed().as_secs_f64() / OPS as f64
    });
    // Worst case for the new code: every version stays alive (a clone per op), so every node on the path is shared.
    let shared = min_ns(30, OPS, || {
        let mut t = avl::insert_at(base, 0, Piece::new(chunk, 0, 6));
        let mut keep: Vec<Tree> = Vec::with_capacity(OPS);
        let t0 = Instant::now();
        for f in &offs {
            keep.push(t.clone());
            let off = (f * avl::len(&t) as f64) as usize;
            avl::insert_mut(&mut t, off, Piece::new(chunk, 0, 6));
        }
        let e = t0.elapsed().as_secs_f64() / OPS as f64;
        black_box((&t, &keep));
        e
    });
    let shared_p = min_ns(30, OPS, || {
        let mut t = avl::insert_at(base, 0, Piece::new(chunk, 0, 6));
        let mut keep: Vec<Tree> = Vec::with_capacity(OPS);
        let t0 = Instant::now();
        for f in &offs {
            keep.push(t.clone());
            t = avl::insert_at(&t, (f * avl::len(&t) as f64) as usize, Piece::new(chunk, 0, 6));
        }
        let e = t0.elapsed().as_secs_f64() / OPS as f64;
        black_box((&t, &keep));
        e
    });
    [persistent, unique, shared, shared_p]
}

/// random edits: insert 6 bytes, then delete 40 bytes inside one piece (a skipped op when it spans pieces).
fn edits(chunk: &Arc<Chunk>, base: &Tree) -> [f64; 2] {
    let (ins, del) = (offsets(2), offsets(3));
    let persistent = min_ns(30, OPS, || {
        let mut t = avl::insert_at(base, 0, Piece::new(chunk, 0, 6));
        let t0 = Instant::now();
        for (fi, fd) in ins.iter().zip(&del) {
            t = avl::insert_at(&t, (fi * avl::len(&t) as f64) as usize, Piece::new(chunk, 0, 6));
            let a = (fd * (avl::len(&t) - 41) as f64) as usize;
            if let Some(n) = avl::delete_in(&t, a, a + 40) {
                t = n;
            }
        }
        black_box(&t);
        t0.elapsed().as_secs_f64() / (2 * OPS) as f64
    });
    let in_place = min_ns(30, OPS, || {
        let mut t = avl::insert_at(base, 0, Piece::new(chunk, 0, 6));
        let t0 = Instant::now();
        for (fi, fd) in ins.iter().zip(&del) {
            let off = (fi * avl::len(&t) as f64) as usize;
            avl::insert_mut(&mut t, off, Piece::new(chunk, 0, 6));
            let a = (fd * (avl::len(&t) - 41) as f64) as usize;
            if !avl::delete_mut(&mut t, a, a + 40) {
                if let Some(n) = avl::delete_in(&t, a, a + 40) {
                    t = n;
                }
            }
        }
        black_box(&t);
        t0.elapsed().as_secs_f64() / (2 * OPS) as f64
    });
    [persistent, in_place]
}

/// backspace: a 10 000-byte piece in the middle, delete its last byte 512 times.
fn backspace(chunk: &Arc<Chunk>, base: &Tree) -> [f64; 2] {
    let mid = avl::len(base) / 2;
    let run = |in_place: bool| {
        min_ns(30, OPS, || {
            let mut t = avl::insert_at(base, mid, Piece::new(chunk, 0, 10_000));
            let mut end = mid + 10_000;
            let t0 = Instant::now();
            for _ in 0..OPS {
                if in_place {
                    assert!(avl::delete_mut(&mut t, end - 1, end));
                } else {
                    t = avl::delete_in(&t, end - 1, end).unwrap();
                }
                end -= 1;
            }
            black_box(&t);
            t0.elapsed().as_secs_f64() / OPS as f64
        })
    };
    [run(false), run(true)]
}

/// The in-place operations give the same pieces as the persistent ones, balanced, with the right totals;
/// versions kept before the edits do not change.
fn verify(chunk: &Arc<Chunk>) {
    let mut rng = Rng(11);
    for &n in &[0usize, 40, 3000] {
        let base = warmed(chunk, n);
        let (mut a, mut b) = (base.clone(), base.clone());
        let mut snaps: Vec<(Tree, Vec<(u32, u32)>)> = Vec::new();
        for step in 0..4000 {
            if step % 97 == 0 {
                let mut v = Vec::new();
                avl::check(&a, &mut v);
                snaps.push((a.clone(), v));
            }
            let len = avl::len(&a);
            let off = (rng.frac() * len as f64) as usize;
            let mut piece = Piece::new(chunk, rng.next() as u32 % 1000, 1 + rng.next() as u32 % 50);
            piece.start += 7;
            let p2 = piece.clone();
            avl::insert_mut(&mut a, off, piece);
            b = avl::insert_at(&b, off, p2);
            if step % 3 == 0 && avl::len(&a) > 200 {
                let (x, w) = ((rng.frac() * (avl::len(&a) - 150) as f64) as usize, 1 + (rng.next() % 120) as usize);
                let done = avl::delete_mut(&mut a, x, x + w);
                let want = avl::delete_in(&b, x, x + w);
                match (done, want) {
                    (true, Some(nb)) => b = nb,
                    (false, _) => {
                        // Unsupported or spanning: the persistent one decides for both.
                        if let Some(nb) = avl::delete_in(&a, x, x + w) {
                            a = nb.clone();
                            b = nb;
                        }
                    }
                    (true, None) => panic!("in place deleted a range that spans pieces"),
                }
            }
            let (mut va, mut vb) = (Vec::new(), Vec::new());
            avl::check(&a, &mut va);
            avl::check(&b, &mut vb);
            assert_eq!(va, vb, "in place and persistent diverged at step {step}");
        }
        for (t, v) in &snaps {
            let mut now = Vec::new();
            avl::check(t, &mut now);
            assert_eq!(&now, v, "a kept version changed");
        }
    }
    println!("verify: ok");
}

pub fn run() {
    verify(&Arc::new(Chunk { _pad: [0; 16] }));
    let chunk = Arc::new(Chunk { _pad: [0; 16] });
    println!("ns per op (min of 30 rounds of {OPS}); pieces after warm-up in brackets");
    for &n in &[0usize, 1_000, 3_000, 30_000] {
        let base = warmed(&chunk, n / 2);
        let pieces = avl::count(&base);
        let p = paste(&chunk, &base);
        let e = edits(&chunk, &base);
        let b = backspace(&chunk, &base);
        println!(
            "[{pieces:>6}] paste: persistent {:>5.0}  in place {:>5.0}  (a clone kept per op: persistent {:>5.0}, in place {:>5.0})   edits(ins+del avg): persistent {:>5.0}  in place {:>5.0}   backspace: persistent {:>4.0}  in place {:>4.0}",
            p[0], p[1], p[3], p[2], e[0], e[1], b[0], b[1]
        );
    }
}
