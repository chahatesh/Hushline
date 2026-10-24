// SPDX-License-Identifier: MIT
//! Atomic insertion into documented Unicode Windows edit controls. No typing,
//! clipboard mutation, surrounding-text capture, focus changes or privilege bypass.
use super::{input_epoch, NativeSelection, Target};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use windows::Win32::{
    Foundation::*, System::Threading::GetCurrentProcessId, UI::WindowsAndMessaging::*,
};

const EM_GETSEL: u32 = 0x00B0;
const EM_REPLACESEL: u32 = 0x00C2;
const EDIT_PROTECTED: isize = 0x20 | 0x800; // ES_PASSWORD | ES_READONLY
fn supported(class: &str) -> bool {
    matches!(
        class.to_ascii_lowercase().as_str(),
        "edit" | "richedit20w" | "richedit50w" | "richeditd2dpt"
    )
}
unsafe fn message(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM, timeout: Duration) -> Option<usize> {
    let mut value = 0;
    (SendMessageTimeoutW(
        hwnd,
        msg,
        w,
        l,
        SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
        timeout.as_millis() as u32,
        Some(&mut value),
    )
    .0 != 0)
        .then_some(value)
}
pub fn inspect(hwnd: HWND, foreground: HWND) -> Option<NativeSelection> {
    unsafe {
        // Foreign-process system messages marshal their pointer parameters.
        // A timed-out in-process recipient could retain borrowed pointers, so
        // Hush's own UI always uses the clipboard path instead.
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == GetCurrentProcessId() {
            return None;
        }
        if hwnd.0.is_null()
            || !IsWindowUnicode(hwnd).as_bool()
            || GetAncestor(hwnd, GA_ROOT) != foreground
            || GetWindowLongPtrW(hwnd, GWL_STYLE) & EDIT_PROTECTED != 0
        {
            return None;
        }
        let mut class = [0u16; 128];
        let count = GetClassNameW(hwnd, &mut class);
        if count == 0 || !supported(&String::from_utf16_lossy(&class[..count as usize])) {
            return None;
        }
        let mut start = 0u32;
        let mut end = 0u32;
        message(
            hwnd,
            EM_GETSEL,
            WPARAM(&mut start as *mut u32 as usize),
            LPARAM(&mut end as *mut u32 as isize),
            Duration::from_millis(150),
        )?;
        let length = message(
            hwnd,
            WM_GETTEXTLENGTH,
            WPARAM(0),
            LPARAM(0),
            Duration::from_millis(150),
        )?;
        if start > end || end as usize > length {
            return None;
        }
        Some(NativeSelection {
            hwnd: hwnd.0 as isize,
            start,
            end,
            length,
        })
    }
}
fn windows_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}
pub fn insert(
    target: &Target,
    selection: &NativeSelection,
    text: &str,
    cancel: &CancellationToken,
) -> Result<String, String> {
    if text.contains('\0') || text.len() > 1_000_000 {
        return Err(
            "This transcript cannot be inserted safely. Your text is ready to copy.".into(),
        );
    }
    let hwnd = HWND(selection.hwnd as *mut _);
    let foreground = HWND(target.hwnd as *mut _);
    if inspect(hwnd, foreground).as_ref() != Some(selection) {
        return Err("The field changed before insertion. Your text is ready to copy.".into());
    }
    let text: Vec<u16> = windows_lines(text).encode_utf16().chain(Some(0)).collect();
    // The last fence is immediately before the single, undoable native edit.
    if cancel.is_cancelled()
        || input_epoch() != target.epoch
        || unsafe { GetForegroundWindow() } != foreground
    {
        return Err("Cancelled or the field changed. Your text is ready to copy.".into());
    }
    let delivered = unsafe {
        message(
            hwnd,
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(text.as_ptr() as isize),
            Duration::from_millis(600),
        )
    };
    if delivered.is_none() {
        // A synchronous recipient might have accepted the edit before timing out.
        // Never follow an uncertain dispatch with clipboard/typing or auto-retry.
        return Err("Windows could not confirm the edit. Check the field before pasting again; your text is preserved.".into());
    }
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_documented_edit_classes_are_supported() {
        for class in ["Edit", "RICHEDIT50W", "RichEditD2DPT"] {
            assert!(supported(class));
        }
        for class in [
            "Chrome_RenderWidgetHostHWND",
            "ConsoleWindowClass",
            "RichEdit20A",
            "password",
            "CustomEdit",
        ] {
            assert!(!supported(class));
        }
        assert_eq!(
            windows_lines("First\nSecond\r\nThird\rFourth"),
            "First\r\nSecond\r\nThird\r\nFourth"
        );
    }
}
#[cfg(test)]
#[path = "edit_tests.rs"]
mod integration;
