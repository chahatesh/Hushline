// SPDX-License-Identifier: MIT
//! Independent raw-keyboard notifications survive low-level hook removal.
//! No timer, keystroke history, text capture, or input injection.
use std::mem::size_of;
use windows::{
    core::w,
    Win32::{
        Foundation::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::*, WindowsAndMessaging::*},
    },
};

pub(super) const IDLE_REPAIR_MS: u64 = 30_000;

#[derive(Clone, Copy)]
pub(super) struct KeyEvent {
    pub key: u16,
    pub down: bool,
    pub tick: u32,
}

#[derive(Default)]
pub(super) struct Activity {
    last_input: Option<u64>,
}
impl Activity {
    pub fn observe(&mut self, now: u64) -> bool {
        self.last_input
            .replace(now)
            .is_none_or(|last| now.saturating_sub(last) >= IDLE_REPAIR_MS)
    }
}

pub(super) fn needs_repair(idle: bool, tracked: bool, reported: bool, physical: bool) -> bool {
    idle || (tracked != reported && reported == physical)
}

/// Hook actions may already be ahead of queued raw input. A replay must not
/// toggle hands-free twice, restart recording, or repeat a paste/open action.
pub(super) fn action_already_seen(last: Option<u32>, event: u32) -> bool {
    last.is_some_and(|last| last.wrapping_sub(event) < (1 << 31))
}
pub(super) fn physical_keys(mut pressed: impl FnMut(u16) -> bool) -> [bool; 256] {
    std::array::from_fn(|key| {
        // Normal mouse buttons never enter the shortcut state machine. Generic
        // modifier aliases duplicate side-specific releases and must stay up.
        (key >= 8 || matches!(key, 5 | 6)) && !matches!(key, 0x10..=0x12) && pressed(key as u16)
    })
}

fn normalize_key(key: u16, scan: u16, flags: u16) -> Option<u16> {
    if key >= 0xff || scan == 0xff {
        return None;
    }
    // RAWKEYBOARD RI_KEY_E0 identifies the right Ctrl/Alt. Generic modifier
    // aliases must not remain down after a side-specific key-up arrives.
    Some(match key {
        0x10 => {
            if scan == 0x36 {
                0xa1
            } else {
                0xa0
            }
        }
        0x11 => {
            if flags & 2 != 0 {
                0xa3
            } else {
                0xa2
            }
        }
        0x12 => {
            if flags & 2 != 0 {
                0xa5
            } else {
                0xa4
            }
        }
        _ => key,
    })
}

pub(super) struct InputWindow(HWND);
impl InputWindow {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let instance = HINSTANCE(
                GetModuleHandleW(None)
                    .map_err(|_| "Windows shortcut recovery could not load.")?
                    .0,
            );
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: w!("HushShortcutRecovery"),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                return Err("Windows shortcut recovery could not start.".into());
            }
            let window = Self(
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("HushShortcutRecovery"),
                    w!(""),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    Some(instance),
                    None,
                )
                .map_err(|_| "Windows shortcut recovery could not start.")?,
            );
            RegisterRawInputDevices(&[RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 6,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: window.0,
            }], size_of::<RAWINPUTDEVICE>() as u32)
                .map_err(|_| "Windows could not enable shortcut recovery. Reopen Hush at normal user permissions.")?;
            Ok(window)
        }
    }
    pub fn is_input(&self, message: &MSG) -> bool {
        message.hwnd == self.0 && message.message == WM_INPUT
    }
    pub fn is_device_change(&self, message: &MSG) -> bool {
        message.hwnd == self.0 && message.message == WM_INPUT_DEVICE_CHANGE
    }
    pub fn read(&self, message: &MSG) -> Option<KeyEvent> {
        unsafe {
            let mut input = RAWINPUT::default();
            let mut bytes = size_of::<RAWINPUT>() as u32;
            let read = GetRawInputData(
                HRAWINPUT(message.lParam.0 as *mut _),
                RID_INPUT,
                Some(&mut input as *mut _ as *mut _),
                &mut bytes,
                size_of::<RAWINPUTHEADER>() as u32,
            );
            // DefWindowProc performs the required foreground raw-input cleanup.
            let _ = DefWindowProcW(
                message.hwnd,
                message.message,
                message.wParam,
                message.lParam,
            );
            if read == u32::MAX
                || read < (size_of::<RAWINPUTHEADER>() + size_of::<RAWKEYBOARD>()) as u32
                || bytes > size_of::<RAWINPUT>() as u32
                || input.header.dwType != RIM_TYPEKEYBOARD.0
            {
                return None;
            }
            let keyboard = input.data.keyboard;
            if keyboard.ExtraInformation as usize == super::INPUT_TAG {
                return None;
            }
            let key = normalize_key(keyboard.VKey, keyboard.MakeCode, keyboard.Flags)?;
            if !matches!(
                keyboard.Message,
                WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
            ) {
                return None;
            }
            Some(KeyEvent {
                key,
                down: matches!(keyboard.Message, WM_KEYDOWN | WM_SYSKEYDOWN),
                tick: message.time,
            })
        }
    }
}
impl Drop for InputWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = RegisterRawInputDevices(
                &[RAWINPUTDEVICE {
                    usUsagePage: 1,
                    usUsage: 6,
                    dwFlags: RIDEV_REMOVE,
                    hwndTarget: HWND::default(),
                }],
                size_of::<RAWINPUTDEVICE>() as u32,
            );
            let _ = DestroyWindow(self.0);
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, message, w, l)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_input_after_hours_repairs_once_without_an_idle_timer() {
        let mut activity = Activity::default();
        assert!(activity.observe(1));
        assert!(!activity.observe(2));
        assert!(!activity.observe(29_999));
        assert!(activity.observe(6 * 60 * 60 * 1000));
        assert!(!activity.observe(6 * 60 * 60 * 1000 + 1));
    }
    #[test]
    fn missed_hooks_repair_but_queued_old_key_messages_do_not() {
        assert!(needs_repair(false, false, true, true));
        assert!(needs_repair(false, true, false, false));
        assert!(!needs_repair(false, false, true, false));
        assert!(!needs_repair(false, true, false, true));
        assert!(!needs_repair(false, true, true, true));
        assert!(needs_repair(true, false, false, false));
    }
    #[test]
    fn recent_actions_deduplicate_raw_messages_across_windows_tick_wrap() {
        assert!(!action_already_seen(None, 10));
        assert!(action_already_seen(Some(10), 10));
        assert!(action_already_seen(Some(11), 10));
        assert!(!action_already_seen(Some(9), 10));
        assert!(action_already_seen(Some(2), u32::MAX));
        assert!(!action_already_seen(Some(u32::MAX), 2));
    }
    #[test]
    fn physical_snapshot_ignores_mouse_buttons_and_generic_modifier_aliases() {
        let state = physical_keys(|_| true);
        for key in [0, 1, 2, 3, 4, 7, 0x10, 0x11, 0x12] {
            assert!(!state[key]);
        }
        for key in [5, 6, 0x41, 0x5b, 0x5c, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5] {
            assert!(state[key]);
        }
    }
    #[test]
    fn raw_modifiers_match_the_low_level_side_specific_keys() {
        for (key, scan, flags, expected) in [
            (0x10, 0x2a, 0, 0xa0),
            (0x10, 0x36, 0, 0xa1),
            (0x11, 0x1d, 0, 0xa2),
            (0x11, 0x1d, 2, 0xa3),
            (0x12, 0x38, 0, 0xa4),
            (0x12, 0x38, 2, 0xa5),
            (0x5b, 0x5b, 2, 0x5b),
            (0x5c, 0x5c, 2, 0x5c),
        ] {
            assert_eq!(normalize_key(key, scan, flags), Some(expected));
        }
        assert_eq!(normalize_key(0xff, 0, 0), None);
        assert_eq!(normalize_key(0x11, 0xff, 0), None);
    }
    #[test]
    fn recovery_window_registers_without_activating_or_reading_other_apps() {
        let window = InputWindow::new().unwrap();
        assert_ne!(window.0, unsafe { GetForegroundWindow() });
        assert!(!unsafe { IsWindowVisible(window.0) }.as_bool());
    }
}
