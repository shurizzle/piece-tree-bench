# SPDX-License-Identifier: GPL-3.0-or-later
import re,glob,statistics as st,sys
D=sys.argv[1]   # results dir of tools/run.sh
OUT=sys.argv[2]  # e.g. ../piece-tree/docs/guides/comparison-tables.md
names=["piece-tree","ropey 1.x","ropey 2 beta","SumTree (Zed rope)"]
def runs(corpus,alloc):
    out=[]
    for f in sorted(glob.glob(f"{D}/w3-{corpus}-{alloc}-*.txt")):
        d={}
        for l in open(f):
            if l.startswith("|") and "scenario" not in l and "---" not in l:
                c=[x.strip() for x in l.strip().strip("|").split("|")]
                d[c[0]]=[tuple(float(v) for v in re.findall(r"[\d.]+",x)[:2]) for x in c[1:5]]
        out.append(d)
    return out
def fmt(v): return f"{v:.1f}" if v<100 else f"{v:,.0f}".replace(","," ")
o=[]
o.append("# Cross-library comparison tables\n")
o.append("**Regenerate with every performance-relevant change** (rule in AGENTS.md): in the harness repository `piece-tree-bench`, `PIECE_TREE_PATH=<checkout>/crates/piece-tree tools/run.sh <dir>` with nothing else running (~25 minutes), then `python3 -I tools/tables.py <dir> <path of this file>`.\n")
o.append("The full results of the last cross-library run, one section per corpus, for every library and both allocators. Nothing here is a claim by itself: the reading, the noise and the history are in [benchmarks.md](benchmarks.md).\n")
o.append("**What was run**: the harness `piece-tree-bench` (a separate repository, not part of this workspace; four libraries in one binary, fat LTO, `codegen-units = 1`): piece-tree (this tree, with `Text::insert_mut` for the typing row, D19), ropey 1.6.1, ropey 2.0.0-beta.1, and Zed's `Rope` on `sum_tree` (`77228009dd267dcf8ab9a20eef5802b2a8ea4afa`). Apple M1 Pro, macOS, 3 runs per corpus and allocator, alternating the system-allocator and the mimalloc build. Each cell is **ns/op, the minimum over the 3 runs of each run's minimum of 7**, followed by the median over the 3 runs of each run's median in parentheses; bold marks the best library of the row (ties within the printed digit are both bold). \"lead\" is the second best over the best. Per-corpus texts: `synthetic` (generated source-like lines), `rust` (~64 MB of Rust sources from the cargo registry, wrapped when shorter than requested), `cjk` (CJK-heavy synthetic).\n")
o.append("Reading hints (from the noise section of benchmarks.md): reads and loads repeat within a few percent; the edit rows (typing with clone per key, paste, random edits, batch) can differ 10-20% between runs of the same binary, and 5-10% between two builds of the same code. Differences under ~10% are not established.\n")
for corpus,title in [("synthetic","Synthetic corpus"),("rust","Rust sources corpus"),("cjk","CJK corpus")]:
    o.append(f"## {title}\n")
    for alloc,aname in [("sys","System allocator"),("mi","mimalloc as global allocator of the whole binary")]:
        rs=runs(corpus,alloc)
        o.append(f"### {title}, {aname} ({len(rs)} runs)\n")
        o.append("| scenario | piece-tree | ropey 1.x | ropey 2 beta | SumTree (Zed rope) | best | lead |")
        o.append("|---|---:|---:|---:|---:|---|---:|")
        for sc in rs[0]:
            mins=[min(r[sc][i][0] for r in rs) for i in range(4)]
            meds=[st.median(r[sc][i][1] for r in rs) for i in range(4)]
            best=min(mins)
            cells=[]
            for i in range(4):
                s=f"{fmt(mins[i])} ({fmt(meds[i])})"
                if round(mins[i],1)<=round(best,1): s=f"**{s}**"
                cells.append(s)
            order=sorted(range(4),key=lambda i:mins[i])
            lead=mins[order[1]]/mins[order[0]]
            winners=[names[i].split(" (")[0] for i in range(4) if round(mins[i],1)<=round(best,1)]
            o.append(f"| {sc} | "+" | ".join(cells)+f" | {' = '.join(winners)} | {lead:.2f}x |")
        o.append("")
open(OUT,"w").write("\n".join(o))
