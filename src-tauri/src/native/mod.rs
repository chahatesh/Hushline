// SPDX-License-Identifier: MIT
#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod hooks;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use clipboard::copy_text;
#[cfg(windows)]
pub use hooks::{input_epoch, set_active, start_hooks, update_shortcuts};
#[cfg(windows)]
pub use windows::*;
pub const INPUT_TAG: usize = 0x48555348;

#[derive(Debug, Clone)]
pub struct Target {
    pub hwnd: isize,
    pub runtime: Vec<i32>,
    pub caret: Vec<i64>,
    pub selected: String,
    pub app: String,
    pub epoch: u64,
    pub terminal: bool,
}
