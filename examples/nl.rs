// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: bulk newline positions with NEON (bitmask + trailing_zeros) vs memchr_iter.
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;
use std::{hint::black_box, time::Instant};

#[cfg(target_arch = "aarch64")]
fn positions_neon(b: &[u8], out: &mut Vec<u32>) {
    let mut i = 0;
    // SAFETY: NEON is baseline on aarch64; every load reads 16 in-bounds bytes.
    unsafe {
        let nl = vdupq_n_u8(b'\n');
        while i + 16 <= b.len() {
            let v = vld1q_u8(b.as_ptr().add(i));
            let eq = vceqq_u8(v, nl);
            // 4 bits per input byte: the standard NEON movemask substitute.
            let mut m = vget_lane_u64(vreinterpret_u64_u8(vshrn_n_u16(vreinterpretq_u16_u8(eq), 4)), 0);
            while m != 0 {
                let t = m.trailing_zeros() as usize / 4;
                out.push((i + t) as u32);
                m &= m - 1;
                m &= !0xF_u64.wrapping_shl((t as u32) * 4); // clear the rest of this byte's nibble
            }
            i += 16;
        }
    }
    for (k, &c) in b[i..].iter().enumerate() {
        if c == b'\n' { out.push((i + k) as u32); }
    }
}

/// simdjson-style: 64-byte blocks -> one 64-bit mask -> branch-light flatten (8 unconditional writes).
#[cfg(target_arch = "aarch64")]
fn positions_neon64(b: &[u8], out: &mut Vec<u32>) {
    let mut i = 0;
    // SAFETY: NEON is baseline on aarch64; loads read 64 in-bounds bytes; the output has 64 spare
    // slots before every block, and only `cnt` of the written slots are kept via `set_len`.
    unsafe {
        let nl = vdupq_n_u8(b'\n');
        let weights: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
        let w = vld1q_u8(weights.as_ptr());
        while i + 64 <= b.len() {
            let p = b.as_ptr().add(i);
            let t0 = vandq_u8(vceqq_u8(vld1q_u8(p), nl), w);
            let t1 = vandq_u8(vceqq_u8(vld1q_u8(p.add(16)), nl), w);
            let t2 = vandq_u8(vceqq_u8(vld1q_u8(p.add(32)), nl), w);
            let t3 = vandq_u8(vceqq_u8(vld1q_u8(p.add(48)), nl), w);
            let s0 = vpaddq_u8(t0, t1);
            let s1 = vpaddq_u8(t2, t3);
            let s = vpaddq_u8(s0, s1);
            let s = vpaddq_u8(s, s);
            let mut mask = vgetq_lane_u64(vreinterpretq_u64_u8(s), 0);
            if mask != 0 {
                out.reserve(64);
                let cnt = mask.count_ones() as usize;
                let len = out.len();
                let dst = out.as_mut_ptr().add(len);
                let base = i as u32;
                for k in 0..8 {
                    *dst.add(k) = base + mask.trailing_zeros();
                    mask &= mask.wrapping_sub(1);
                }
                if cnt > 8 {
                    for k in 8..cnt {
                        *dst.add(k) = base + mask.trailing_zeros();
                        mask &= mask.wrapping_sub(1);
                    }
                }
                out.set_len(len + cnt);
            }
            i += 64;
        }
    }
    for (k, &c) in b[i..].iter().enumerate() {
        if c == b'\n' { out.push((i + k) as u32); }
    }
}

fn best<T>(mut f: impl FnMut() -> T) -> f64 {
    (0..7).map(|_| { let t = Instant::now(); black_box(f()); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min)
}

fn main() {
    let mut s = String::new();
    let mut stack = vec![std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cargo/registry/src")];
    while let Some(d) = stack.pop() {
        let mut es: Vec<_> = std::fs::read_dir(&d).into_iter().flatten().flatten().collect();
        es.sort_by_key(|e| e.path());
        for e in es {
            let p = e.path();
            if p.is_dir() { stack.push(p) } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(t) = std::fs::read_to_string(&p) { s.push_str(&t); }
            }
            if s.len() >= 50 << 20 { break; }
        }
        if s.len() >= 50 << 20 { break; }
    }
    let b = s.as_bytes();
    let reference: Vec<u32> = memchr::memchr_iter(b'\n', b).map(|i| i as u32).collect();
    let per = |t: f64| t * 1e9 / b.len() as f64;
    println!("{} MB, {} newlines", b.len() >> 20, reference.len());
    println!("memchr_iter collect  {:.3} ns/B", per(best(|| memchr::memchr_iter(b'\n', b).map(|i| i as u32).collect::<Vec<u32>>())));
    #[cfg(target_arch = "aarch64")]
    {
        let mut v = Vec::new();
        positions_neon(b, &mut v);
        assert_eq!(v, reference, "NEON positions differ from memchr");
        let mut v = Vec::new();
        positions_neon64(b, &mut v);
        assert_eq!(v, reference, "NEON64 positions differ from memchr");
        println!("neon 64B flatten     {:.3} ns/B (same output: ok)", per(best(|| { let mut v = Vec::with_capacity(reference.len()); positions_neon64(b, &mut v); v })));
        println!("neon bitmask         {:.3} ns/B (same output: ok)", per(best(|| { let mut v = Vec::with_capacity(reference.len()); positions_neon(b, &mut v); v })));
    }
}
