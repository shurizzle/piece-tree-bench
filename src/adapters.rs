// SPDX-License-Identifier: GPL-3.0-or-later
//! One adapter per library, all exposing byte offsets. Ropey 1.x indexes by char, so its
//! adapter converts with `byte_to_char` on every edit, as an editor working in bytes would.

use std::ops::Range;

pub trait Buffer: Clone {
    const NAME: &'static str;
    fn from_str(s: &str) -> Self;
    /// Libraries that cannot take over a `String` just copy it.
    fn from_string(s: String) -> Self;
    fn len(&self) -> usize;
    fn line_count(&self) -> usize;
    fn line_to_byte(&self, line: usize) -> usize;
    fn byte_to_line(&self, byte: usize) -> usize;
    fn insert(&mut self, byte: usize, s: &str);
    /// Insert when the previous version stays alive (undo-like). Only piece-tree has a persistent
    /// variant apart from its in-place one; the other libraries mutate in place either way.
    fn insert_kept(&mut self, byte: usize, s: &str) {
        self.insert(byte, s);
    }
    fn delete(&mut self, range: Range<usize>);
    /// Byte length of line `i` (including its `\n`), summed over the chunks the library hands out
    /// for it: the cheapest way to get at the line text without copying it.
    fn line_len(&self, line: usize) -> usize;
    /// Applies sorted, non-overlapping `(range, replacement)` edits given in original offsets.
    /// Default: one edit at a time from the end, so earlier offsets stay valid.
    fn apply_edits(&mut self, edits: &[(Range<usize>, &str)]) {
        for (r, s) in edits.iter().rev() {
            if !r.is_empty() {
                self.delete(r.clone());
            }
            if !s.is_empty() {
                self.insert(r.start, s);
            }
        }
    }
    /// Feeds the whole text to `f`, chunk by chunk (for the cross-library sanity check).
    fn for_each_chunk(&self, f: &mut dyn FnMut(&str));
}

impl Buffer for piece_tree::Text {
    const NAME: &'static str = "piece-tree";
    fn from_str(s: &str) -> Self {
        Self::from(s)
    }
    fn from_string(s: String) -> Self {
        Self::from(s)
    }
    fn len(&self) -> usize {
        piece_tree::Text::len(self)
    }
    fn line_count(&self) -> usize {
        piece_tree::Text::line_count(self)
    }
    fn line_to_byte(&self, line: usize) -> usize {
        self.line_to_offset(line)
    }
    fn byte_to_line(&self, byte: usize) -> usize {
        self.offset_to_line(byte)
    }
    // The in-place API of a `&mut` value, like the other libraries' (D19 and its successor): nodes
    // nobody else owns are edited where they are, shared ones are cloned.
    fn insert(&mut self, byte: usize, s: &str) {
        piece_tree::Text::insert_mut(self, byte, s);
    }
    fn insert_kept(&mut self, byte: usize, s: &str) {
        *self = piece_tree::Text::insert(self, byte, s);
    }
    fn delete(&mut self, range: Range<usize>) {
        piece_tree::Text::delete_mut(self, range);
    }
    fn line_len(&self, line: usize) -> usize {
        self.line(line).len()
    }
    /// piece-tree's own batch API (picks a rebuild or path copies by itself).
    fn apply_edits(&mut self, edits: &[(Range<usize>, &str)]) {
        *self = piece_tree::Text::apply_edits(self, edits);
    }
    fn for_each_chunk(&self, f: &mut dyn FnMut(&str)) {
        self.chunks().for_each(f);
    }
}

#[derive(Clone)]
pub struct Ropey1(ropey::Rope);

impl Buffer for Ropey1 {
    const NAME: &'static str = "ropey 1.x";
    fn from_str(s: &str) -> Self {
        Self(ropey::Rope::from_str(s))
    }
    fn from_string(s: String) -> Self {
        Self(ropey::Rope::from_str(&s))
    }
    fn len(&self) -> usize {
        self.0.len_bytes()
    }
    fn line_count(&self) -> usize {
        self.0.len_lines()
    }
    fn line_to_byte(&self, line: usize) -> usize {
        self.0.line_to_byte(line)
    }
    fn byte_to_line(&self, byte: usize) -> usize {
        self.0.byte_to_line(byte)
    }
    fn insert(&mut self, byte: usize, s: &str) {
        let ch = self.0.byte_to_char(byte);
        self.0.insert(ch, s);
    }
    fn delete(&mut self, range: Range<usize>) {
        let (a, b) = (self.0.byte_to_char(range.start), self.0.byte_to_char(range.end));
        self.0.remove(a..b);
    }
    fn line_len(&self, line: usize) -> usize {
        self.0.line(line).chunks().map(str::len).sum()
    }
    fn for_each_chunk(&self, f: &mut dyn FnMut(&str)) {
        self.0.chunks().for_each(f);
    }
}

#[derive(Clone)]
pub struct Ropey2(ropey2::Rope);

const LF_CR: ropey2::LineType = ropey2::LineType::LF_CR;

impl Buffer for Ropey2 {
    const NAME: &'static str = "ropey 2 beta";
    fn from_str(s: &str) -> Self {
        Self(ropey2::Rope::from_str(s))
    }
    fn from_string(s: String) -> Self {
        Self(ropey2::Rope::from(s))
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn line_count(&self) -> usize {
        self.0.len_lines(LF_CR)
    }
    fn line_to_byte(&self, line: usize) -> usize {
        self.0.line_to_byte_idx(line, LF_CR)
    }
    fn byte_to_line(&self, byte: usize) -> usize {
        self.0.byte_to_line_idx(byte, LF_CR)
    }
    fn insert(&mut self, byte: usize, s: &str) {
        self.0.insert(byte, s);
    }
    fn delete(&mut self, range: Range<usize>) {
        self.0.remove(range);
    }
    fn line_len(&self, line: usize) -> usize {
        self.0.line(line, LF_CR).chunks().map(str::len).sum()
    }
    fn for_each_chunk(&self, f: &mut dyn FnMut(&str)) {
        self.0.chunks().for_each(f);
    }
}

#[derive(Clone)]
pub struct Zed(rope::Rope);

impl Zed {
    fn row_start(&self, row: usize) -> usize {
        self.0.point_to_offset(rope::Point::new(row as u32, 0))
    }
}

impl Buffer for Zed {
    const NAME: &'static str = "SumTree (Zed rope)";
    fn from_str(s: &str) -> Self {
        Self(rope::Rope::from(s))
    }
    fn from_string(s: String) -> Self {
        Self(rope::Rope::from(s))
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn line_count(&self) -> usize {
        self.0.max_point().row as usize + 1
    }
    fn line_to_byte(&self, line: usize) -> usize {
        self.row_start(line)
    }
    fn byte_to_line(&self, byte: usize) -> usize {
        self.0.offset_to_point(byte).row as usize
    }
    fn insert(&mut self, byte: usize, s: &str) {
        self.0.replace(byte..byte, s);
    }
    fn delete(&mut self, range: Range<usize>) {
        self.0.replace(range, "");
    }
    fn line_len(&self, line: usize) -> usize {
        let start = self.row_start(line);
        // Last line has no successor row: it ends at the end of the text.
        let end = if line + 1 < self.line_count() { self.row_start(line + 1) } else { self.len() };
        self.0.chunks_in_range(start..end).map(str::len).sum()
    }
    /// Zed's own pattern (`Buffer::edit`): one cursor pass that rebuilds the rope for all edits.
    fn apply_edits(&mut self, edits: &[(Range<usize>, &str)]) {
        let mut out = rope::Rope::new();
        let mut cursor = self.0.cursor(0);
        for (r, s) in edits {
            out.append(cursor.slice(r.start));
            cursor.seek_forward(r.end);
            out.push(s);
        }
        out.append(cursor.suffix());
        self.0 = out;
    }
    fn for_each_chunk(&self, f: &mut dyn FnMut(&str)) {
        self.0.chunks().for_each(f);
    }
}
