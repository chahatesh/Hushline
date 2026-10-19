// SPDX-License-Identifier: MIT
//! Fail-closed inspection and insertion. No screenshots, window titles or surrounding text.
use super::{input_epoch, Target};
use std::{mem::size_of, ptr};
use windows::{
    core::{w, PCWSTR, PWSTR},
    Win32::{
        Foundation::*,
        Security::Credentials::*,
        System::{
            Com::*,
            Ole::*,
            Threading::*,
            Variant::{VariantClear, VariantToBoolean},
        },
        UI::{Accessibility::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
};
use zeroize::Zeroize;
type Result<T> = std::result::Result<T, String>;

struct Com;
impl Com {
    fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|_| "Windows accessibility is unavailable.")?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn automation() -> Result<IUIAutomation> {
    unsafe {
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "Windows accessibility is unavailable.".into())
    }
}
fn array_i32(array: *mut SAFEARRAY) -> Result<Vec<i32>> {
    if array.is_null() {
        return Err("Field identity is unavailable.".into());
    }
    unsafe {
        let result = (|| {
            let lo = SafeArrayGetLBound(array, 1).map_err(|_| "Invalid field identity.")?;
            let hi = SafeArrayGetUBound(array, 1).map_err(|_| "Invalid field identity.")?;
            if hi - lo > 128 {
                return Err("Invalid field identity.".into());
            }
            let mut out = Vec::new();
            for i in lo..=hi {
                let mut value = 0i32;
                SafeArrayGetElement(array, &i, &mut value as *mut _ as *mut _)
                    .map_err(|_| "Cannot read field identity.")?;
                out.push(value);
            }
            Ok(out)
        })();
        let _ = SafeArrayDestroy(array);
        result
    }
}
fn array_f64(array: *mut SAFEARRAY) -> Vec<i64> {
    if array.is_null() {
        return vec![];
    }
    unsafe {
        let mut out = Vec::new();
        if let (Ok(lo), Ok(hi)) = (SafeArrayGetLBound(array, 1), SafeArrayGetUBound(array, 1)) {
            if hi - lo <= 256 {
                for i in lo..=hi {
                    let mut value = 0f64;
                    if SafeArrayGetElement(array, &i, &mut value as *mut _ as *mut _).is_ok() {
                        out.push((value * 10.0).round() as i64);
                    }
                }
            }
        }
        let _ = SafeArrayDestroy(array);
        out
    }
}
pub fn capture_target(read_selection: bool) -> Result<Target> {
    let _com = Com::new()?;
    let uia = automation()?;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Err("No active application. Transcript will be kept for recovery.".into());
        }
        let element = uia.GetFocusedElement().map_err(|_| {
            "Cannot inspect the focused field. Transcript will be kept for recovery."
        })?;
        // UIA providers can lag foreground changes. Verify ancestry without reading text.
        let walker = uia
            .RawViewWalker()
            .map_err(|_| "Cannot verify the field's owning window.")?;
        let mut ancestor = element.clone();
        let mut owned = false;
        for _ in 0..32 {
            if let Ok(window) = ancestor.CurrentNativeWindowHandle() {
                if !window.0.is_null() && (window == hwnd || GetAncestor(window, GA_ROOT) == hwnd) {
                    owned = true;
                    break;
                }
            }
            match walker.GetParentElement(&ancestor) {
                Ok(parent) => ancestor = parent,
                Err(_) => break,
            }
        }
        if !owned {
            return Err(
                "The focused field does not belong to the active window. Transcript preserved."
                    .into(),
            );
        }
        if element
            .CurrentIsPassword()
            .map_err(|_| "Cannot determine field protection.")?
            .as_bool()
        {
            return Err("Dictation is disabled in password fields.".into());
        }
        if !element
            .CurrentIsEnabled()
            .map_err(|_| "Cannot determine field state.")?
            .as_bool()
        {
            return Err("The focused field is disabled.".into());
        }
        let control = element
            .CurrentControlType()
            .map_err(|_| "Cannot determine field type.")?;
        if control != UIA_EditControlTypeId
            && control != UIA_DocumentControlTypeId
            && control != UIA_ComboBoxControlTypeId
        {
            return Err(
                "Focus an editable text field. Transcript will be kept for recovery.".into(),
            );
        }
        let runtime = array_i32(
            element
                .GetRuntimeId()
                .map_err(|_| "Cannot identify this text field.")?,
        )?;
        let mut selected = String::new();
        let mut caret = vec![];
        let mut editable = false;
        if let Ok(value) =
            element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
        {
            if let Ok(read_only) = value.CurrentIsReadOnly() {
                if read_only.as_bool() {
                    return Err("The focused field is read-only. Transcript preserved.".into());
                }
                editable = true;
            }
        }
        if let Ok(pattern) =
            element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
        {
            if !editable {
                if let Ok(document) = pattern.DocumentRange() {
                    if let Ok(mut attribute) = document.GetAttributeValue(UIA_IsReadOnlyAttributeId)
                    {
                        editable = VariantToBoolean(&attribute).is_ok_and(|v| !v.as_bool());
                        let _ = VariantClear(&mut attribute);
                    }
                }
            }
            if let Ok(ranges) = pattern.GetSelection() {
                if ranges.Length().unwrap_or(0) == 1 {
                    if let Ok(range) = ranges.GetElement(0) {
                        caret = range
                            .GetBoundingRectangles()
                            .map(array_f64)
                            .unwrap_or_default();
                        if read_selection {
                            selected = range
                                .GetText(32001)
                                .map_err(|_| "Cannot read selected text.")?
                                .to_string();
                            if selected.len() > 32000 {
                                return Err(
                                    "Select fewer than 32,000 characters for Command Mode.".into(),
                                );
                            }
                        }
                    }
                }
            }
        }
        if !editable {
            return Err("The focused field is read-only or does not expose safe editing. Transcript preserved.".into());
        }
        let pid = element
            .CurrentProcessId()
            .map_err(|_| "Cannot identify the application.")? as u32;
        let app = process_name(pid).unwrap_or_else(|| "Unknown app".into());
        let terminal = [
            "windowsterminal.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "conhost.exe",
            "wezterm-gui.exe",
            "mintty.exe",
            "alacritty.exe",
        ]
        .contains(&app.to_ascii_lowercase().as_str());
        Ok(Target {
            hwnd: hwnd.0 as isize,
            runtime,
            caret,
            selected,
            app,
            epoch: input_epoch(),
            terminal,
        })
    }
}
fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        std::path::Path::new(&String::from_utf16_lossy(&buffer[..size as usize]))
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
    }
}
pub fn insert(
    target: &Target,
    text: &str,
    method: &str,
    authorized_preview: bool,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<String> {
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    if target.terminal {
        return Err("Automatic insertion into terminals is disabled to avoid executing commands. Copy the transcript and paste it yourself.".into());
    }
    if authorized_preview {
        // A dashboard approval explicitly authorizes bringing the original field forward.
        unsafe {
            let _ = SetForegroundWindow(HWND(target.hwnd as *mut _));
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    } else if input_epoch() != target.epoch {
        return Err("Focus or cursor changed. Your transcript is ready to copy.".into());
    }
    // Never send shortcuts while the user's modifiers are physically held.
    for _ in 0..80 {
        if cancel.is_cancelled() {
            return Err("Cancelled".into());
        }
        if unsafe {
            [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
                .iter()
                .all(|k| GetAsyncKeyState(k.0 as i32) >= 0)
        } {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if unsafe {
        [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
            .iter()
            .any(|k| GetAsyncKeyState(k.0 as i32) < 0)
    } {
        return Err("Release keyboard modifiers, then paste the saved transcript.".into());
    }
    if cancel.is_cancelled() {
        return Err("Cancelled".into());
    }
    if method == "clipboard" {
        // paste_text performs the final UIA check after preparing the clipboard.
        // Avoid two redundant provider traversals on the insertion critical path.
        return super::clipboard::paste_text(target, text, authorized_preview, cancel);
    }
    let final_check = capture_target(!target.selected.is_empty())?;
    if !target_matches(target, &final_check, authorized_preview) {
        return Err("The target changed before insertion. Transcript preserved.".into());
    }
    unicode_text(text)?;
    Ok("Inserted using Windows Unicode input. The receiving application does not provide a delivery receipt.".into())
}
fn target_matches(a: &Target, b: &Target, explicit: bool) -> bool {
    a.hwnd == b.hwnd
        && !a.runtime.is_empty()
        && a.runtime == b.runtime
        && a.caret == b.caret
        && a.selected == b.selected
        && (explicit || a.epoch == b.epoch)
}
fn unicode_text(text: &str) -> Result<()> {
    if text.contains(['\r', '\n']) {
        return Err("Multiline text is ready to copy. Choose clipboard insertion in Settings for multiline paste; Unicode typing never sends Enter.".into());
    }
    // One batch minimizes the gap between validation and delivery. No Enter key or submit command.
    let mut inputs = Vec::with_capacity(text.len() * 2);
    for code in text.encode_utf16() {
        for up in [false, true] {
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: code,
                        dwFlags: KEYEVENTF_UNICODE
                            | if up {
                                KEYEVENTF_KEYUP
                            } else {
                                KEYBD_EVENT_FLAGS(0)
                            },
                        time: 0,
                        dwExtraInfo: super::INPUT_TAG,
                    },
                },
            });
        }
    }
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err("Windows blocked some input, possibly because the target runs as administrator. Transcript preserved; check for partial insertion before pasting.".into());
    }
    Ok(())
}
pub fn save_key(mut key: String) -> Result<()> {
    let clean = key.trim();
    if clean.len() < 20 || clean.len() > 512 || clean.chars().any(char::is_whitespace) {
        key.zeroize();
        return Err("Enter a valid Groq API key.".into());
    }
    let mut bytes = clean.as_bytes().to_vec();
    let target: Vec<u16> = "Hushline/Groq".encode_utf16().chain(Some(0)).collect();
    let user: Vec<u16> = "Groq".encode_utf16().chain(Some(0)).collect();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_ptr() as *mut _),
        CredentialBlobSize: bytes.len() as u32,
        CredentialBlob: bytes.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user.as_ptr() as *mut _),
        ..Default::default()
    };
    let result = unsafe { CredWriteW(&credential, 0) }
        .map_err(|_| "Windows Credential Manager could not save your key.".into());
    bytes.zeroize();
    key.zeroize();
    result
}
pub fn read_key() -> Result<zeroize::Zeroizing<String>> {
    unsafe {
        let mut ptr_credential = ptr::null_mut();
        CredReadW(
            w!("Hushline/Groq"),
            CRED_TYPE_GENERIC,
            None,
            &mut ptr_credential,
        )
        .map_err(|_| "Add your Groq API key in Settings to start dictating.")?;
        let c = &*ptr_credential;
        let value = String::from_utf8(
            std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec(),
        );
        std::slice::from_raw_parts_mut(c.CredentialBlob, c.CredentialBlobSize as usize).zeroize();
        CredFree(ptr_credential as *const _);
        value
            .map(zeroize::Zeroizing::new)
            .map_err(|_| "Stored key is invalid. Enter it again.".into())
    }
}
pub fn delete_key() -> Result<()> {
    unsafe {
        CredDeleteW(
            PCWSTR(w!("Hushline/Groq").as_ptr()),
            CRED_TYPE_GENERIC,
            None,
        )
    }
    .map_err(|_| "Could not remove the saved key.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_multiline_cannot_submit_a_message() {
        assert!(unicode_text("first line\nsecond line").is_err());
        assert!(unicode_text("first line\rsecond line").is_err());
    }
    #[test]
    #[ignore = "Temporarily stores a dummy credential in Hushline's Windows credential slot; refuses to replace an existing key."]
    fn credential_manager_roundtrip() {
        assert!(
            read_key().is_err(),
            "An existing Hushline key is present; refusing to overwrite it"
        );
        struct Cleanup;
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = delete_key();
            }
        }
        let dummy = "hushline-test-only-not-a-real-groq-key";
        save_key(dummy.into()).unwrap();
        let _cleanup = Cleanup;
        assert_eq!(read_key().unwrap().as_str(), dummy);
        delete_key().unwrap();
        assert!(read_key().is_err());
    }
    #[test]
    fn changed_focus_is_rejected() {
        let t = Target {
            hwnd: 1,
            runtime: vec![1, 2],
            caret: vec![3],
            selected: "x".into(),
            app: "Notepad".into(),
            epoch: 2,
            terminal: false,
        };
        let mut b = t.clone();
        assert!(target_matches(&t, &b, false));
        b.epoch = 3;
        assert!(!target_matches(&t, &b, false));
        assert!(target_matches(&t, &b, true));
        b.selected = "y".into();
        assert!(!target_matches(&t, &b, true));
    }
}
