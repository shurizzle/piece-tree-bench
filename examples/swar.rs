// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: SWAR mask64 (no intrinsics) vs NEON mask64 vs memchr_iter, across newline densities.
use std::arch::aarch64::*;
use std::{hint::black_box, time::Instant};

#[inline(always)]
fn mask8_swar(p: *const u8) -> u64 {
    // SAFETY: caller guarantees 8 readable bytes.
    let x = u64::from_le_bytes(unsafe { p.cast::<[u8; 8]>().read_unaligned() });
    let t = x ^ 0x0A0A_0A0A_0A0A_0A0A; // zero byte where the byte was '\n'
    // Exact zero-byte test: no carry crosses bytes because (t & 0x7F) + 0x7F <= 0xFE.
    let nonzero = ((t & 0x7F7F_7F7F_7F7F_7F7F) + 0x7F7F_7F7F_7F7F_7F7F) | t;
    let zero_hi = !nonzero & 0x8080_8080_8080_8080;
    // Gather the 8 high bits (positions 7, 15, ..) into one byte: bit i = byte i.
    ((zero_hi >> 7).wrapping_mul(0x0102_0408_1020_4080)) >> 56
}

#[inline(always)]
unsafe fn mask64_swar(p: *const u8) -> u64 {
    let mut m = 0u64;
    for i in 0..8 {
        m |= unsafe { mask8_swar(p.add(i * 8)) } << (i * 8);
    }
    m
}

#[inline(always)]
unsafe fn mask64_neon(p: *const u8) -> u64 {
    unsafe {
        let nl = vdupq_n_u8(b'\n');
        const W: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
        let w = vld1q_u8(W.as_ptr());
        let t0 = vandq_u8(vceqq_u8(vld1q_u8(p), nl), w);
        let t1 = vandq_u8(vceqq_u8(vld1q_u8(p.add(16)), nl), w);
        let t2 = vandq_u8(vceqq_u8(vld1q_u8(p.add(32)), nl), w);
        let t3 = vandq_u8(vceqq_u8(vld1q_u8(p.add(48)), nl), w);
        let s = vpaddq_u8(vpaddq_u8(t0, t1), vpaddq_u8(t2, t3));
        let s = vpaddq_u8(s, s);
        vgetq_lane_u64(vreinterpretq_u64_u8(s), 0)
    }
}

/// memchr's trick: OR the four compares and test once; only blocks with a newline build the mask.
#[inline(always)]
unsafe fn mask64_neon_skip(p: *const u8) -> u64 {
    unsafe {
        let nl = vdupq_n_u8(b'\n');
        let e0 = vceqq_u8(vld1q_u8(p), nl);
        let e1 = vceqq_u8(vld1q_u8(p.add(16)), nl);
        let e2 = vceqq_u8(vld1q_u8(p.add(32)), nl);
        let e3 = vceqq_u8(vld1q_u8(p.add(48)), nl);
        if vmaxvq_u8(vorrq_u8(vorrq_u8(e0, e1), vorrq_u8(e2, e3))) == 0 {
            return 0;
        }
        const W: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
        let w = vld1q_u8(W.as_ptr());
        let s = vpaddq_u8(vpaddq_u8(vandq_u8(e0, w), vandq_u8(e1, w)), vpaddq_u8(vandq_u8(e2, w), vandq_u8(e3, w)));
        let s = vpaddq_u8(s, s);
        vgetq_lane_u64(vreinterpretq_u64_u8(s), 0)
    }
}

/// Same early-out for SWAR: OR the eight exact zero-byte tests, skip empty blocks.
#[inline(always)]
unsafe fn mask64_swar_skip(p: *const u8) -> u64 {
    let mut zero = [0u64; 8];
    let mut any = 0u64;
    for i in 0..8 {
        let x = u64::from_le_bytes(unsafe { p.add(i * 8).cast::<[u8; 8]>().read_unaligned() });
        let t = x ^ 0x0A0A_0A0A_0A0A_0A0A;
        let z = !(((t & 0x7F7F_7F7F_7F7F_7F7F) + 0x7F7F_7F7F_7F7F_7F7F) | t) & 0x8080_8080_8080_8080;
        zero[i] = z;
        any |= z;
    }
    if any == 0 {
        return 0;
    }
    let mut m = 0u64;
    for i in 0..8 {
        m |= (((zero[i] >> 7).wrapping_mul(0x0102_0408_1020_4080)) >> 56) << (i * 8);
    }
    m
}

#[inline(always)]
unsafe fn flatten(mut mask: u64, base: u32, dst: *mut u32) -> usize {
    let hits = mask.count_ones() as usize;
    for k in 0..8 {
        unsafe { dst.add(k).write(base.wrapping_add(mask.trailing_zeros())) };
        mask &= mask.wrapping_sub(1);
    }
    for k in 8..hits {
        unsafe { dst.add(k).write(base + mask.trailing_zeros()) };
        mask &= mask.wrapping_sub(1);
    }
    hits
}

macro_rules! scan_with {
    ($name:ident, $mask:ident) => {
        unsafe fn $name(bytes: &[u8], dst: *mut u32) -> usize {
            let mut n = 0;
            let mut base = 0u32;
            let (blocks, rest) = bytes.as_chunks::<64>();
            for b in blocks {
                n += unsafe { flatten($mask(b.as_ptr()), base, dst.add(n)) };
                base += 64;
            }
            if !rest.is_empty() {
                let mut pad = [0u8; 64];
                pad[..rest.len()].copy_from_slice(rest);
                n += unsafe { flatten($mask(pad.as_ptr()), base, dst.add(n)) };
            }
            n
        }
    };
}
scan_with!(scan_swar, mask64_swar);
scan_with!(scan_neon, mask64_neon);
scan_with!(scan_neon_skip, mask64_neon_skip);
scan_with!(scan_swar_skip, mask64_swar_skip);

macro_rules! scan_nz {
    ($name:ident, $mask:ident) => {
        /// Same, but an empty mask skips `flatten` too (the 8 junk writes).
        unsafe fn $name(bytes: &[u8], dst: *mut u32) -> usize {
            let mut n = 0;
            let mut base = 0u32;
            let (blocks, rest) = bytes.as_chunks::<64>();
            for b in blocks {
                let m = unsafe { $mask(b.as_ptr()) };
                if m != 0 {
                    n += unsafe { flatten(m, base, dst.add(n)) };
                }
                base += 64;
            }
            if !rest.is_empty() {
                let mut pad = [0u8; 64];
                pad[..rest.len()].copy_from_slice(rest);
                n += unsafe { flatten($mask(pad.as_ptr()), base, dst.add(n)) };
            }
            n
        }
    };
}
scan_nz!(scan_neon_nz, mask64_neon);
scan_nz!(scan_neon_skip_nz, mask64_neon_skip);
scan_nz!(scan_swar_skip_nz, mask64_swar_skip);

unsafe fn scan_memchr(bytes: &[u8], dst: *mut u32) -> usize {
    let mut n = 0;
    for i in memchr::memchr_iter(b'\n', bytes) {
        unsafe { dst.add(n).write(i as u32) };
        n += 1;
    }
    n
}

/// memchr's portable word-at-a-time implementation: what `memchr_iter` runs where memchr has no SIMD.
unsafe fn scan_memchr_all(bytes: &[u8], dst: *mut u32) -> usize {
    let mut n = 0;
    for i in memchr::arch::all::memchr::One::new(b'\n').iter(bytes) {
        unsafe { dst.add(n).write(i as u32) };
        n += 1;
    }
    n
}

fn best<T>(mut f: impl FnMut() -> T) -> f64 {
    (0..9).map(|_| { let t = Instant::now(); black_box(f()); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min)
}

/// Whole function: no count pass, capacity grown on demand (the junk writes need 64 spare slots).
macro_rules! positions_vec {
    ($name:ident, $mask:ident) => {
        fn $name(bytes: &[u8]) -> Vec<u32> {
            let mut out: Vec<u32> = Vec::with_capacity(bytes.len() / 64 + 64);
            let mut base = 0u32;
            let (blocks, rest) = bytes.as_chunks::<64>();
            for b in blocks {
                let m = unsafe { $mask(b.as_ptr()) };
                if m != 0 {
                    out.reserve(64);
                    let len = out.len();
                    let n = unsafe { flatten(m, base, out.as_mut_ptr().add(len)) };
                    unsafe { out.set_len(len + n) };
                }
                base += 64;
            }
            if !rest.is_empty() {
                let mut pad = [0u8; 64];
                pad[..rest.len()].copy_from_slice(rest);
                let m = unsafe { $mask(pad.as_ptr()) };
                out.reserve(64);
                let len = out.len();
                let n = unsafe { flatten(m, base, out.as_mut_ptr().add(len)) };
                unsafe { out.set_len(len + n) };
            }
            out
        }
    };
}
positions_vec!(vec_neon_skip, mask64_neon_skip);
positions_vec!(vec_swar_skip, mask64_swar_skip);

fn run(name: &str, bytes: &[u8]) {
    let reference: Vec<u32> = memchr::memchr_iter(b'\n', bytes).map(|i| i as u32).collect();
    let per = |t: f64| t * 1e9 / bytes.len() as f64;
    let check = |v: Vec<u32>| { assert_eq!(v, reference, "{name}: output differs"); };
    check(vec_neon_skip(bytes));
    check(vec_swar_skip(bytes));
    let old = per(best(|| memchr::memchr_iter(b'\n', bytes).map(|i| i as u32).collect::<Vec<u32>>()));
    let old_scalar = per(best(|| memchr::arch::all::memchr::One::new(b'\n').iter(bytes).map(|i| i as u32).collect::<Vec<u32>>()));
    let neon = per(best(|| vec_neon_skip(bytes)));
    let swar = per(best(|| vec_swar_skip(bytes)));
    println!("{name:<22} {:>6.1} B/line  OLD memchr collect {old:.3} (scalar {old_scalar:.3}) | NEW neon+skip {neon:.3} ({:.2}x)  swar+skip {swar:.3} ({:.2}x vs scalar)", bytes.len() as f64 / reference.len().max(1) as f64, old / neon, old_scalar / swar);
}

fn main() {
    let mut rust = String::new();
    let mut stack = vec![std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cargo/registry/src")];
    'o: while let Some(d) = stack.pop() {
        let mut es: Vec<_> = std::fs::read_dir(&d).into_iter().flatten().flatten().collect();
        es.sort_by_key(|e| e.path());
        for e in es {
            let p = e.path();
            if p.is_dir() { stack.push(p) } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(t) = std::fs::read_to_string(&p) { rust.push_str(&t); }
            }
            if rust.len() >= 32 << 20 { break 'o; }
        }
    }
    let mk = |line: &str, mb: usize| { let mut s = String::new(); while s.len() < mb << 20 { s.push_str(line); s.push('\n'); } s };
    run("rust sources", rust.as_bytes());
    run("blank lines (1 B)", "\n".repeat(32 << 20).as_bytes());
    // Exponentially distributed line lengths (many short, a few long) like real text: the
    // uniform-length runs above hide the branch mispredictions of the per-hit iterators.
    for avg in [16usize, 24, 32, 48, 64, 96, 128, 192, 256, 512, 1000] {
        let mut s = String::new();
        let mut x = 0x9E3779B97F4A7C15u64;
        while s.len() < 32 << 20 {
            x ^= x << 13; x ^= x >> 7; x ^= x << 17;
            let u = ((x >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
            let len = 1 + (-u.ln() * (avg as f64 - 1.0)) as usize;
            s.push_str(&"x".repeat(len.min(20_000) - 1));
            s.push('\n');
        }
        run(&format!("exp. lines, avg {avg}"), s.as_bytes());
    }
    run("one line (no newline)", "x".repeat(32 << 20).as_bytes());
}
