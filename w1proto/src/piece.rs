// SPDX-License-Identifier: GPL-3.0-or-later
use triomphe::Arc;

pub struct Chunk {
    pub _pad: [u8; 16],
}

/// Same layout as `piece_tree::tree::Piece` (24 bytes).
#[derive(Clone)]
pub struct Piece {
    pub chunk: Arc<Chunk>,
    pub start: u32,
    pub len: u32,
    pub lf: u32,
    pub nl_first: u32,
}

impl Piece {
    pub fn new(chunk: &Arc<Chunk>, start: u32, len: u32) -> Self {
        Piece { chunk: chunk.clone(), start, len, lf: len / 40, nl_first: 0 }
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn lf(&self) -> usize {
        self.lf as usize
    }
    /// Split at `at` (0 < at < len), like `Piece::split`: two clones of the chunk Arc.
    pub fn split(&self, at: usize) -> (Piece, Piece) {
        let lf = (at / 40) as u32;
        let at = at as u32;
        let left = Piece { len: at, lf, ..self.clone() };
        let right = Piece { chunk: self.chunk.clone(), start: self.start + at, len: self.len - at, lf: self.lf - lf, nl_first: self.nl_first + lf };
        (left, right)
    }
    /// Sub-piece `[from, to)`, like `Piece::sub` (one chunk Arc clone).
    pub fn sub(&self, from: usize, to: usize) -> Piece {
        let (a, b) = ((from / 40) as u32, (to / 40) as u32);
        Piece { chunk: self.chunk.clone(), start: self.start + from as u32, len: (to - from) as u32, lf: b - a, nl_first: self.nl_first + a }
    }
    pub fn lf_before(&self, off: usize) -> usize {
        off / 40
    }
}
