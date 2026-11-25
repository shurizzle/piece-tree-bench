// Minimal replacement for Zed's `gpui_util` (only what `rope` and `sum_tree` use), see ../PATCHES.md. Written 2026-11-25.
//! Minimal stand-in for Zed's `gpui_util`: only what `rope` uses.

#[macro_export]
macro_rules! debug_panic {
    ($($fmt_arg:tt)*) => {
        if cfg!(debug_assertions) {
            panic!($($fmt_arg)*);
        } else {
            log::error!($($fmt_arg)*);
        }
    };
}

#[inline(always)]
pub const fn is_utf8_char_boundary(b: u8) -> bool {
    (b as i8) >= -0x40
}
