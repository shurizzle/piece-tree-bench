// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: whole-load time of `Text::from(String)` (50 MB) across newline densities.
use std::{hint::black_box, time::Instant};

fn rust_text(bytes: usize) -> String {
    let mut s = String::new();
    let mut stack = vec![std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cargo/registry/src")];
    while let Some(d) = stack.pop() {
        let mut es: Vec<_> = std::fs::read_dir(&d).into_iter().flatten().flatten().collect();
        es.sort_by_key(|e| e.path());
        for e in es {
            let p = e.path();
            if p.is_dir() { stack.push(p) } else if p.extension().is_some_and(|x| x == "rs") {
                if let Ok(t) = std::fs::read_to_string(&p) && !t.contains('\r') { s.push_str(&t); if !s.ends_with('\n') { s.push('\n'); } }
            }
            if s.len() >= bytes { s.truncate(s.rfind('\n').unwrap() + 1); return s; }
        }
    }
    s
}

fn synthetic(bytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < bytes { s.push_str(&format!("    let value_{i} = compute({i}, \"lorem ipsum ✓\");\n")); i += 1; }
    s
}

fn cjk(bytes: usize) -> String {
    let mut s = String::new();
    let mut i = 0;
    while s.len() < bytes { s.push_str(&format!("    let 変数_{i} = 計算({i}, \"日本語のテキストと漢字かな交じり文\");\n")); i += 1; }
    s
}

/// Exponentially distributed line lengths, like real text; `avg` is the mean (0 = a single line).
fn exp_lines(bytes: usize, avg: usize) -> String {
    if avg == 0 {
        return "x".repeat(bytes);
    }
    let mut s = String::new();
    let mut x = 0x9E3779B97F4A7C15u64;
    while s.len() < bytes {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        let u = ((x >> 11) as f64 + 1.0) / (1u64 << 53) as f64;
        let len = 1 + (-u.ln() * (avg as f64 - 1.0)) as usize;
        s.push_str(&"x".repeat(len.min(20_000) - 1));
        s.push('\n');
    }
    s
}

fn main() {
    let n = 50 << 20;
    let corpora: Vec<(String, String)> = vec![
        ("rust (34 B/line)".into(), rust_text(n)),
        ("synthetic (59 B)".into(), synthetic(n)),
        ("cjk (89 B)".into(), cjk(n)),
        ("exp avg 100".into(), exp_lines(n, 100)),
        ("exp avg 200".into(), exp_lines(n, 200)),
        ("exp avg 400".into(), exp_lines(n, 400)),
        ("exp avg 1000".into(), exp_lines(n, 1000)),
        ("exp avg 5000".into(), exp_lines(n, 5000)),
        ("one line".into(), exp_lines(n, 0)),
    ];
    for (name, text) in &corpora {
        let mut owned: Vec<String> = (0..7).map(|_| text.clone()).collect();
        let t = (0..7).map(|_| { let s = owned.pop().unwrap(); let t = Instant::now(); black_box(piece_tree::Text::from(s)); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min);
        println!("{name:<20} {:.3} ns/B", t * 1e9 / text.len() as f64);
    }
}
