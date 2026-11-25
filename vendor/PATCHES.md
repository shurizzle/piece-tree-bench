# Vendored Zed crates

Source: https://github.com/zed-industries/zed at commit `77228009dd267dcf8ab9a20eef5802b2a8ea4afa`
(`crates/sum_tree`, `crates/rope`).

**Licenses differ**: `sum_tree` and `gpui_util` are Apache-2.0 (`sum_tree/LICENSE-APACHE`), `rope` is
GPL-3.0-or-later (`rope/LICENSE-GPL`). Resolved by publishing this whole repository under GPL-3.0-or-later
(Apache-2.0 code may be included in a GPL-3.0 work): see `../LICENSE` and `../NOTICE.md`. The vendored files
are modified (this list); each carries a notice with the date of the modification (2026-11-25).

Changes (algorithmic code is untouched):
- Removed every `#[cfg(test)] mod tests` and `sum_tree/src/property_test.rs` (they need `util`, `zlog`, `gpui`).
- Removed `ztracing::instrument` imports and `#[instrument(skip_all)]` attributes (tracing only).
- `gpui_util` replaced by `vendor/gpui_util`: `debug_panic!` and `is_utf8_char_boundary` only.
- Manifests rewritten without workspace inheritance and lints. `rayon`, `heapless`,
  `unicode-segmentation` and `log` are kept as in upstream, so large loads still use rayon like in Zed.
