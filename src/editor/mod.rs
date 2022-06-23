/*
TODO:
1. Reorganize the mod file. Project is way bigger than I thought and I need to move components to different files.
2. Collapse whitespace into a single Plain enum variant. - Done
3. Add per file type config - Done
4. Configurable syntax highlighting - In progress
5. Rebindable keys - TODO
6. Unicode support still patchy - Fixed for the time being
7. Move code out of mod.rs very cringe - Done
8. Plugins prolly lua based... cause dynamic cdylib loading very sketch
*/

mod editor;
mod highlight;
mod history;
mod plugin;
pub mod prelude;
mod utils;

use crate::editor::history::EditDiff;
use unescape::unescape;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

#[cfg(debug_assertions)]
pub fn kelp_version() -> String {
    format!(
        "{} - Debug",
        option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown")
    )
}

#[cfg(not(debug_assertions))]
pub fn kelp_version() -> String {
    format!("{}", option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"))
}