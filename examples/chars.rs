// SPDX-License-Identifier: GPL-3.0-or-later
//! Scratch: char_count / offset_to_char / char_to_offset on a 50 MB text, ns per byte.
use piece_tree::Text;
use std::{hint::black_box, time::Instant};

fn corpus(kind: &str, bytes: usize) -> String {
    let unit = match kind {
        "ascii" => "    let value = compute(1, \"lorem ipsum\");\n".to_string(),
        "cjk" => "日本語のテキスト、これは試験です。\n".to_string(),
        _ => "naïve café — ✓ 🦀 line of mixed text\n".to_string(),
    };
    unit.repeat(bytes / unit.len() + 1)
}

fn best(mut f: impl FnMut()) -> f64 {
    (0..9).map(|_| { let t = Instant::now(); f(); t.elapsed().as_secs_f64() }).fold(f64::MAX, f64::min)
}

fn main() {
    for kind in ["ascii", "mixed", "cjk"] {
        let s = corpus(kind, 50 << 20);
        let t = Text::from(s.as_str());
        let n = black_box(t.len()) as f64;
        let chars = t.char_count();
        let a = best(|| { black_box(t.char_count()); });
        let b = best(|| { black_box(t.offset_to_char(t.floor_char_boundary(t.len() - 1))); });
        let c = best(|| { black_box(t.char_to_offset(chars - 1)); });
        let d = best(|| { black_box(s.chars().count()); });
        println!("{kind:<6} char_count {:.3} ns/B ({:.1} ms)  offset_to_char {:.3}  char_to_offset {:.3}  std chars().count() on &str {:.3}", a * 1e9 / n, a * 1e3, b * 1e9 / n, c * 1e9 / n, d * 1e9 / n);
    }
}
