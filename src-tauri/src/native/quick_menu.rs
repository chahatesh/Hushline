// SPDX-License-Identifier: MIT
use crate::quick::{MenuTarget, QuickCommand, QuickSelection};
use windows::{
    core::PCWSTR,
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
};

struct Popup(HMENU);
impl Drop for Popup {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}
unsafe fn create(has_text: bool, busy: bool) -> Result<Popup, String> {
    let popup = Popup(CreatePopupMenu().map_err(|_| "Windows could not open the quick menu.")?);
    for (index, command) in QuickCommand::ALL.into_iter().enumerate() {
        if matches!(
            command,
            QuickCommand::History | QuickCommand::HideHour | QuickCommand::Cancel
        ) {
            AppendMenuW(popup.0, MF_SEPARATOR, 0, PCWSTR::null())
                .map_err(|_| "Could not build the quick menu.")?;
        }
        let label: Vec<u16> = command.label().encode_utf16().chain(Some(0)).collect();
        let flags = MF_STRING
            | if command.enabled(has_text, busy) {
                MF_ENABLED
            } else {
                MF_GRAYED
            };
        AppendMenuW(popup.0, flags, index + 1, PCWSTR(label.as_ptr()))
            .map_err(|_| "Could not build the quick menu.")?;
    }
    Ok(popup)
}
/// Standard native menu: supports keyboard navigation, scaling and outside-click dismissal.
pub(super) unsafe fn show(
    owner: HWND,
    has_text: bool,
    busy: bool,
    animated: bool,
) -> Result<Option<QuickSelection>, String> {
    let popup = create(has_text, busy)?;
    let original = super::capture_focus_anchor().map(|a| MenuTarget {
        hwnd: a.hwnd,
        pid: a.pid,
    });
    let mut point = POINT::default();
    GetCursorPos(&mut point).map_err(|_| "Cannot position the quick menu.")?;
    if !SetForegroundWindow(owner).as_bool() {
        return Err("Windows could not open the menu here. Use the Hush tray menu instead.".into());
    }
    let flags = TPM_RETURNCMD
        | TPM_NONOTIFY
        | TPM_RIGHTBUTTON
        | if animated {
            TRACK_POPUP_MENU_FLAGS(0)
        } else {
            TPM_NOANIMATION
        };
    let selected = TrackPopupMenuEx(popup.0, flags.0, point.x, point.y, owner, None).0 as usize;
    let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
    // Restore only our own temporary menu activation, never a window the user switched to.
    if GetForegroundWindow() == owner {
        if let Some(target) = original {
            super::restore_menu_target(target);
        }
    }
    let Some(command) = selected
        .checked_sub(1)
        .and_then(|index| QuickCommand::ALL.get(index))
        .copied()
    else {
        return Ok(None);
    };
    // No clipboard or field reads while a menu owns input. The worker rechecks this window.
    Ok(Some(QuickSelection {
        command,
        target: if command.needs_target() {
            original
        } else {
            None
        },
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_native_menu_exposes_controls_and_disables_unavailable_actions() {
        unsafe {
            let popup = create(false, true).unwrap();
            assert_eq!(GetMenuItemCount(Some(popup.0)), 12);
            assert_ne!(GetMenuState(popup.0, 1, MF_BYCOMMAND) & MF_GRAYED.0, 0);
            assert_ne!(GetMenuState(popup.0, 3, MF_BYCOMMAND) & MF_GRAYED.0, 0);
            assert_eq!(GetMenuState(popup.0, 4, MF_BYCOMMAND) & MF_GRAYED.0, 0);
        }
    }
}
