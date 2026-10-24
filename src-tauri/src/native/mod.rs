// SPDX-License-Identifier: MIT
#[cfg(windows)]
mod clipboard;
#[cfg(windows)]
mod cue;
#[cfg(windows)]
mod edit;
#[cfg(windows)]
mod hooks;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use clipboard::copy_text;
#[cfg(windows)]
pub use cue::play_recording_cue;
#[cfg(windows)]
pub use hooks::{input_epoch, set_active, shortcut_capture, start_hooks, update_shortcuts};
#[cfg(windows)]
pub use overlay::{
    configure as configure_overlay, start as start_overlay, update as update_overlay,
};
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
    pub native_edit: Option<NativeSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelection {
    pub hwnd: isize,
    pub start: u32,
    pub end: u32,
    pub length: usize,
}

/// An explicitly requested, transient copy of one editable field. Never persisted.
#[derive(Debug, Clone)]
pub struct WholeFieldSnapshot {
    pub target: Target,
    pub original: String,
}
