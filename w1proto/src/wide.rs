// SPDX-License-Identifier: GPL-3.0-or-later
//! Candidate: persistent B-tree, up to `L` pieces per leaf and `F` children per inner node,
//! every node immutable and shared through `Arc`. Insert only (balanced by construction).
use crate::piece::Piece;
use triomphe::Arc;

pub enum Node<const L: usize, const F: usize> {
    Leaf(Leaf<L>),
    Inner(Inner<L, F>),
}

pub struct Leaf<const L: usize> {
    n: u8,
    len: usize,
    lf: usize,
    p: [Option<Piece>; L],
}

pub struct Inner<const L: usize, const F: usize> {
    n: u8,
    len: usize,
    lf: usize,
    cl: [usize; F],
    cf: [usize; F],
    c: [Option<Arc<Node<L, F>>>; F],
}

pub enum Ins<const L: usize, const F: usize> {
    One(Arc<Node<L, F>>),
    Two(Arc<Node<L, F>>, Arc<Node<L, F>>),
}

impl<const L: usize, const F: usize> Node<L, F> {
    pub fn len(&self) -> usize {
        match self {
            Node::Leaf(l) => l.len,
            Node::Inner(i) => i.len,
        }
    }
    pub fn lf(&self) -> usize {
        match self {
            Node::Leaf(l) => l.lf,
            Node::Inner(i) => i.lf,
        }
    }
}

fn mk_leaf<const L: usize, const F: usize>(items: &mut [Option<Piece>]) -> Arc<Node<L, F>> {
    let mut p: [Option<Piece>; L] = std::array::from_fn(|_| None);
    let (mut len, mut lf) = (0, 0);
    for (i, it) in items.iter_mut().enumerate() {
        let x = it.take().unwrap();
        len += x.len();
        lf += x.lf();
        p[i] = Some(x);
    }
    Arc::new(Node::Leaf(Leaf { n: items.len() as u8, len, lf, p }))
}

fn mk_inner<const L: usize, const F: usize>(items: &mut [Option<Arc<Node<L, F>>>]) -> Arc<Node<L, F>> {
    let mut c: [Option<Arc<Node<L, F>>>; F] = std::array::from_fn(|_| None);
    let (mut cl, mut cf) = ([0usize; F], [0usize; F]);
    let (mut len, mut lf) = (0, 0);
    for (i, it) in items.iter_mut().enumerate() {
        let x = it.take().unwrap();
        cl[i] = x.len();
        cf[i] = x.lf();
        len += cl[i];
        lf += cf[i];
        c[i] = Some(x);
    }
    Arc::new(Node::Inner(Inner { n: items.len() as u8, len, lf, cl, cf, c }))
}

pub fn leaf_single<const L: usize, const F: usize>(p: Piece) -> Arc<Node<L, F>> {
    mk_leaf(&mut [Some(p)])
}

pub fn insert<const L: usize, const F: usize>(root: &Arc<Node<L, F>>, off: usize, piece: Piece) -> Arc<Node<L, F>> {
    match insert_node(root, off, piece) {
        Ins::One(n) => n,
        Ins::Two(a, b) => mk_inner(&mut [Some(a), Some(b)]),
    }
}

fn insert_node<const L: usize, const F: usize>(n: &Arc<Node<L, F>>, off: usize, piece: Piece) -> Ins<L, F> {
    match &**n {
        Node::Leaf(l) => {
            let mut tmp: [Option<Piece>; 20] = std::array::from_fn(|_| None);
            let (mut cnt, mut cum) = (0, 0);
            let mut new = Some(piece);
            for i in 0..l.n as usize {
                let p = l.p[i].as_ref().unwrap();
                if new.is_some() && off <= cum {
                    tmp[cnt] = new.take();
                    cnt += 1;
                }
                if new.is_some() && off < cum + p.len() {
                    let (a, b) = p.split(off - cum);
                    tmp[cnt] = Some(a);
                    tmp[cnt + 1] = new.take();
                    tmp[cnt + 2] = Some(b);
                    cnt += 3;
                } else {
                    tmp[cnt] = Some(p.clone());
                    cnt += 1;
                }
                cum += p.len();
            }
            if new.is_some() {
                tmp[cnt] = new.take();
                cnt += 1;
            }
            if cnt <= L {
                Ins::One(mk_leaf(&mut tmp[..cnt]))
            } else {
                let mid = cnt / 2;
                let (a, b) = tmp[..cnt].split_at_mut(mid);
                Ins::Two(mk_leaf(a), mk_leaf(b))
            }
        }
        Node::Inner(inn) => {
            let (mut j, mut cum) = (0, 0);
            while j + 1 < inn.n as usize && off >= cum + inn.cl[j] {
                cum += inn.cl[j];
                j += 1;
            }
            let r = insert_node(inn.c[j].as_ref().unwrap(), off - cum, piece);
            let mut tmp: [Option<Arc<Node<L, F>>>; 20] = std::array::from_fn(|_| None);
            let mut cnt = 0;
            for i in 0..inn.n as usize {
                if i == j {
                    match &r {
                        Ins::One(x) => {
                            tmp[cnt] = Some(x.clone());
                            cnt += 1;
                        }
                        Ins::Two(a, b) => {
                            tmp[cnt] = Some(a.clone());
                            tmp[cnt + 1] = Some(b.clone());
                            cnt += 2;
                        }
                    }
                } else {
                    tmp[cnt] = inn.c[i].clone();
                    cnt += 1;
                }
            }
            if cnt <= F {
                Ins::One(mk_inner(&mut tmp[..cnt]))
            } else {
                let mid = cnt / 2;
                let (a, b) = tmp[..cnt].split_at_mut(mid);
                Ins::Two(mk_inner(a), mk_inner(b))
            }
        }
    }
}

pub fn lf_before<const L: usize, const F: usize>(root: &Arc<Node<L, F>>, off: usize) -> usize {
    let (mut n, mut off, mut acc) = (root, off, 0);
    loop {
        match &**n {
            Node::Inner(inn) => {
                let mut j = 0;
                while j + 1 < inn.n as usize && off >= inn.cl[j] {
                    off -= inn.cl[j];
                    acc += inn.cf[j];
                    j += 1;
                }
                n = inn.c[j].as_ref().unwrap();
            }
            Node::Leaf(l) => {
                for i in 0..l.n as usize {
                    let p = l.p[i].as_ref().unwrap();
                    if off <= p.len() || i + 1 == l.n as usize {
                        return acc + p.lf_before(off);
                    }
                    off -= p.len();
                    acc += p.lf();
                }
                return acc;
            }
        }
    }
}

pub fn stats<const L: usize, const F: usize>(n: &Arc<Node<L, F>>) -> (usize, usize, usize) {
    // (pieces, leaves, inner nodes)
    match &**n {
        Node::Leaf(l) => (l.n as usize, 1, 0),
        Node::Inner(i) => {
            let mut t = (0, 0, 1);
            for c in i.c.iter().take(i.n as usize) {
                let s = stats(c.as_ref().unwrap());
                t = (t.0 + s.0, t.1 + s.1, t.2 + s.2);
            }
            t
        }
    }
}

/// Typing: the piece ending exactly at `off` grows by `grow` bytes, path copy. Nodes are built
/// straight from the old ones (no scratch array), the cheapest shape a real implementation has.
pub fn extend<const L: usize, const F: usize>(n: &Arc<Node<L, F>>, off: usize, grow: u32) -> Option<Arc<Node<L, F>>> {
    match &**n {
        Node::Leaf(l) => {
            let (mut cum, mut hit) = (0, usize::MAX);
            for i in 0..l.n as usize {
                cum += l.p[i].as_ref().unwrap().len();
                if cum == off {
                    hit = i;
                    break;
                }
            }
            if hit == usize::MAX {
                return None;
            }
            let p = std::array::from_fn(|i| {
                l.p[i].as_ref().map(|p| {
                    let mut q = p.clone();
                    if i == hit {
                        q.len += grow;
                    }
                    q
                })
            });
            Some(Arc::new(Node::Leaf(Leaf { n: l.n, len: l.len + grow as usize, lf: l.lf, p })))
        }
        Node::Inner(inn) => {
            let (mut j, mut cum) = (0, 0);
            while j + 1 < inn.n as usize && off > cum + inn.cl[j] {
                cum += inn.cl[j];
                j += 1;
            }
            let r = extend(inn.c[j].as_ref().unwrap(), off - cum, grow)?;
            let mut cl = inn.cl;
            cl[j] += grow as usize;
            let c = std::array::from_fn(|i| if i == j { Some(r.clone()) } else { inn.c[i].clone() });
            Some(Arc::new(Node::Inner(Inner { n: inn.n, len: inn.len + grow as usize, lf: inn.lf, cl, cf: inn.cf, c })))
        }
    }
}
