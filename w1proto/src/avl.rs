// SPDX-License-Identifier: GPL-3.0-or-later
//! The current design: persistent join-based AVL, one piece per node (copied from tree.rs).
use crate::piece::Piece;
use triomphe::Arc;

pub type Tree = Option<Arc<Node>>;

#[derive(Clone)]
pub struct Node {
    pub left: Tree,
    pub right: Tree,
    pub piece: Piece,
    height: u8,
    len: usize,
    lf: usize,
}

pub fn height(t: &Tree) -> u8 {
    t.as_ref().map_or(0, |n| n.height)
}
pub fn len(t: &Tree) -> usize {
    t.as_ref().map_or(0, |n| n.len)
}
pub fn lf(t: &Tree) -> usize {
    t.as_ref().map_or(0, |n| n.lf)
}

fn node(left: Tree, piece: Piece, right: Tree) -> Tree {
    Some(Arc::new(Node {
        height: 1 + height(&left).max(height(&right)),
        len: len(&left) + piece.len() + len(&right),
        lf: lf(&left) + piece.lf() + lf(&right),
        left,
        right,
        piece,
    }))
}

fn expose(n: Arc<Node>) -> (Tree, Piece, Tree) {
    match Arc::try_unwrap(n) {
        Ok(n) => (n.left, n.piece, n.right),
        Err(n) => (n.left.clone(), n.piece.clone(), n.right.clone()),
    }
}

fn rotate_left(t: Tree) -> Tree {
    let (a, x, r) = expose(t.unwrap());
    let (b, y, c) = expose(r.unwrap());
    node(node(a, x, b), y, c)
}

fn rotate_right(t: Tree) -> Tree {
    let (l, y, c) = expose(t.unwrap());
    let (a, x, b) = expose(l.unwrap());
    node(a, x, node(b, y, c))
}

fn join_right(l: Tree, k: Piece, r: Tree) -> Tree {
    let (ll, lk, lr) = expose(l.unwrap());
    if height(&lr) <= height(&r) + 1 {
        let t = node(lr, k, r);
        if height(&t) <= height(&ll) + 1 { node(ll, lk, t) } else { rotate_left(node(ll, lk, rotate_right(t))) }
    } else {
        let t = join_right(lr, k, r);
        let unbalanced = height(&t) > height(&ll) + 1;
        let t = node(ll, lk, t);
        if unbalanced { rotate_left(t) } else { t }
    }
}

fn join_left(l: Tree, k: Piece, r: Tree) -> Tree {
    let (rl, rk, rr) = expose(r.unwrap());
    if height(&rl) <= height(&l) + 1 {
        let t = node(l, k, rl);
        if height(&t) <= height(&rr) + 1 { node(t, rk, rr) } else { rotate_right(node(rotate_left(t), rk, rr)) }
    } else {
        let t = join_left(l, k, rl);
        let unbalanced = height(&t) > height(&rr) + 1;
        let t = node(t, rk, rr);
        if unbalanced { rotate_right(t) } else { t }
    }
}

pub fn join(l: Tree, k: Piece, r: Tree) -> Tree {
    let (hl, hr) = (height(&l), height(&r));
    if hl > hr + 1 {
        join_right(l, k, r)
    } else if hr > hl + 1 {
        join_left(l, k, r)
    } else {
        node(l, k, r)
    }
}

pub fn insert_at(t: &Tree, off: usize, p: Piece) -> Tree {
    let Some(n) = t else { return node(None, p, None) };
    let (ll, pl) = (len(&n.left), n.piece.len());
    if off <= ll {
        join(insert_at(&n.left, off, p), n.piece.clone(), n.right.clone())
    } else if off >= ll + pl {
        join(n.left.clone(), n.piece.clone(), insert_at(&n.right, off - ll - pl, p))
    } else {
        let (p1, p2) = n.piece.split(off - ll);
        let right = insert_at(&insert_at(&n.right, 0, p2), 0, p);
        join(n.left.clone(), p1, right)
    }
}

pub fn lf_before(t: &Tree, off: usize) -> usize {
    let (mut t, mut off, mut acc) = (t, off, 0);
    while let Some(n) = t {
        let ll = len(&n.left);
        if off < ll {
            t = &n.left;
            continue;
        }
        acc += lf(&n.left);
        off -= ll;
        if off <= n.piece.len() {
            return acc + n.piece.lf_before(off);
        }
        acc += n.piece.lf();
        off -= n.piece.len();
        t = &n.right;
    }
    acc
}

pub fn count(t: &Tree) -> usize {
    t.as_ref().map_or(0, |n| 1 + count(&n.left) + count(&n.right))
}

/// Typing: the piece ending exactly at `off` grows by `grow` bytes, path copy, no rebalancing.
pub fn extend_at(t: &Tree, off: usize, grow: u32) -> Option<Tree> {
    let n = t.as_ref()?;
    let (ll, pl) = (len(&n.left), n.piece.len());
    if off <= ll {
        let left = extend_at(&n.left, off, grow)?;
        Some(node(left, n.piece.clone(), n.right.clone()))
    } else if off < ll + pl {
        None
    } else if off == ll + pl {
        let mut p = n.piece.clone();
        p.len += grow;
        Some(node(n.left.clone(), p, n.right.clone()))
    } else {
        let right = extend_at(&n.right, off - ll - pl, grow)?;
        Some(node(n.left.clone(), n.piece.clone(), right))
    }
}

/// Typing when the nodes on the path are uniquely owned: grow the piece in place, no allocation.
/// Returns false (nothing changed) when the path is shared or no piece ends at `off`.
pub fn extend_in_place(t: &mut Tree, off: usize, grow: u32) -> bool {
    let Some(arc) = t.as_mut() else { return false };
    let Some(n) = Arc::get_mut(arc) else { return false };
    let (ll, pl) = (len(&n.left), n.piece.len());
    let ok = if off <= ll {
        extend_in_place(&mut n.left, off, grow)
    } else if off < ll + pl {
        false
    } else if off == ll + pl {
        n.piece.len += grow;
        true
    } else {
        extend_in_place(&mut n.right, off - ll - pl, grow)
    };
    if ok {
        n.len += grow as usize;
    }
    ok
}

// ---- persistent delete (copied from tree.rs: split_last, join2, delete_in) ----

fn split_last(n: Arc<Node>) -> (Tree, Piece) {
    let (l, p, r) = expose(n);
    match r {
        None => (l, p),
        Some(r) => {
            let (r, last) = split_last(r);
            (join(l, p, r), last)
        }
    }
}

fn join2(l: Tree, r: Tree) -> Tree {
    match l {
        None => r,
        Some(l) => {
            let (l, last) = split_last(l);
            join(l, last, r)
        }
    }
}

/// Deletes `[a, b)` when it lies inside one piece, copying one path; `None` when it spans pieces.
pub fn delete_in(t: &Tree, a: usize, b: usize) -> Option<Tree> {
    let n = t.as_ref()?;
    let (ll, pl) = (len(&n.left), n.piece.len());
    if b <= ll {
        Some(join(delete_in(&n.left, a, b)?, n.piece.clone(), n.right.clone()))
    } else if a >= ll + pl {
        Some(join(n.left.clone(), n.piece.clone(), delete_in(&n.right, a - ll - pl, b - ll - pl)?))
    } else if a < ll || b > ll + pl {
        None
    } else {
        let (a, b) = (a - ll, b - ll);
        let (l, r) = (n.left.clone(), n.right.clone());
        Some(match (a == 0, b == pl) {
            (true, true) => join2(l, r),
            (true, false) => join(l, n.piece.sub(b, pl), r),
            (false, true) => join(l, n.piece.sub(0, a), r),
            (false, false) => join(l, n.piece.sub(0, a), insert_at(&r, 0, n.piece.sub(b, pl))),
        })
    }
}

// ---- in place with copy-on-write (Arc::make_mut): the candidate ----

fn upd(n: &mut Node) {
    n.height = 1 + height(&n.left).max(height(&n.right));
    n.len = len(&n.left) + n.piece.len() + len(&n.right);
    n.lf = lf(&n.left) + n.piece.lf() + lf(&n.right);
}

fn rotate_right_mut(t: &mut Tree) {
    let mut x = t.take().unwrap();
    let xn = Arc::make_mut(&mut x);
    let mut y = xn.left.take().unwrap();
    let yn = Arc::make_mut(&mut y);
    xn.left = yn.right.take();
    upd(xn);
    yn.right = Some(x);
    upd(yn);
    *t = Some(y);
}

fn rotate_left_mut(t: &mut Tree) {
    let mut x = t.take().unwrap();
    let xn = Arc::make_mut(&mut x);
    let mut y = xn.right.take().unwrap();
    let yn = Arc::make_mut(&mut y);
    xn.right = yn.left.take();
    upd(xn);
    yn.left = Some(x);
    upd(yn);
    *t = Some(y);
}

fn rebalance_mut(t: &mut Tree) {
    let n = t.as_ref().unwrap();
    let (hl, hr) = (height(&n.left), height(&n.right));
    if hl > hr + 1 {
        let l = n.left.as_ref().unwrap();
        if height(&l.left) < height(&l.right) {
            rotate_left_mut(&mut Arc::make_mut(t.as_mut().unwrap()).left);
        }
        rotate_right_mut(t);
    } else if hr > hl + 1 {
        let r = n.right.as_ref().unwrap();
        if height(&r.right) < height(&r.left) {
            rotate_right_mut(&mut Arc::make_mut(t.as_mut().unwrap()).right);
        }
        rotate_left_mut(t);
    }
}

/// AVL insert of `p` at byte `off`, editing every node that has one owner in place and cloning
/// (only) the ones that are shared: `Arc::make_mut` along the path.
pub fn insert_mut(t: &mut Tree, off: usize, p: Piece) {
    let Some(arc) = t else {
        *t = node(None, p, None);
        return;
    };
    let n = Arc::make_mut(arc);
    let (ll, pl) = (len(&n.left), n.piece.len());
    if off <= ll {
        insert_mut(&mut n.left, off, p);
    } else if off >= ll + pl {
        insert_mut(&mut n.right, off - ll - pl, p);
    } else {
        let (p1, p2) = n.piece.split(off - ll);
        let at = ll + p1.len();
        n.piece = p1;
        upd(n);
        insert_mut(t, at, p2);
        insert_mut(t, at, p);
        return;
    }
    upd(n);
    rebalance_mut(t);
}

/// Deletes `[a, b)` when it lies inside one piece (but not the whole piece); `false` otherwise,
/// with the text unchanged.
pub fn delete_mut(t: &mut Tree, a: usize, b: usize) -> bool {
    let Some(arc) = t else { return false };
    let (ll, pl) = (len(&arc.left), arc.piece.len());
    if b <= ll || a >= ll + pl {
        let n = Arc::make_mut(arc);
        let ok = if b <= ll { delete_mut(&mut n.left, a, b) } else { delete_mut(&mut n.right, a - ll - pl, b - ll - pl) };
        if ok {
            upd(n);
            rebalance_mut(t);
        }
        return ok;
    }
    if a < ll || b > ll + pl {
        return false;
    }
    let (a, b) = (a - ll, b - ll);
    if a == 0 && b == pl {
        return false;
    }
    let n = Arc::make_mut(arc);
    if a == 0 {
        n.piece = n.piece.sub(b, pl);
    } else if b == pl {
        n.piece = n.piece.sub(0, a);
    } else {
        let p2 = n.piece.sub(b, pl);
        n.piece = n.piece.sub(0, a);
        upd(n);
        insert_mut(t, ll + a, p2);
        return true;
    }
    upd(n);
    true
}

/// Test helper: in-order `(start, len)` of every piece, checking balance and the cached totals.
pub fn check(t: &Tree, out: &mut Vec<(u32, u32)>) -> (u8, usize, usize) {
    let Some(n) = t else { return (0, 0, 0) };
    let (hl, ll, fl) = check(&n.left, out);
    out.push((n.piece.start, n.piece.len));
    let (hr, lr, fr) = check(&n.right, out);
    assert!(hl.abs_diff(hr) <= 1, "unbalanced");
    let (h, l, f) = (1 + hl.max(hr), ll + n.piece.len() + lr, fl + n.piece.lf() + fr);
    assert_eq!((n.height, n.len, n.lf), (h, l, f), "totals");
    (h, l, f)
}
