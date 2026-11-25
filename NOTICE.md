# Third-party components

This repository is licensed under **GPL-3.0-or-later** (`LICENSE`). It contains or builds against:

| Component | Where | License | Notes |
|---|---|---|---|
| Zed `rope` | `vendor/rope` | GPL-3.0-or-later (`vendor/rope/LICENSE-GPL`) | from https://github.com/zed-industries/zed at commit `77228009dd267dcf8ab9a20eef5802b2a8ea4afa`; modified, see `vendor/PATCHES.md` |
| Zed `sum_tree` | `vendor/sum_tree` | Apache-2.0 (`vendor/sum_tree/LICENSE-APACHE`) | same commit; modified, see `vendor/PATCHES.md` |
| `gpui_util` replacement | `vendor/gpui_util` | Apache-2.0 | minimal stand-in for Zed's crate (`debug_panic!`, `is_utf8_char_boundary`) |
| `piece-tree` | git dependency, https://github.com/shurizzle/piece-tree | MIT | the library under test |
| `ropey` 1.6 and 2.0.0-beta.1 | crates.io | MIT | |
| `memchr`, `mimalloc`, `triomphe` | crates.io | MIT / Unlicense / Apache-2.0 | see each crate |

Apache-2.0 code is compatible with GPL-3.0 (version 3 only), which is why the combined work is GPL-3.0-or-later.
The text corpus of the `rust` scenario is read from the user's own cargo registry at run time and is not part of
this repository.
