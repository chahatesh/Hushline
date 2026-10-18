// SPDX-License-Identifier: MIT
//! Nonactivating native overlay. No WebView, idle animation, or audio dependency.
use crate::{model::View, shortcuts::Action};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        atomic::{AtomicIsize, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use windows::{
    core::{w, BOOL},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
};
const UPDATE: u32 = WM_APP + 17;
struct Bridge {
    hwnd: AtomicIsize,
    view: Arc<Mutex<OverlayView>>,
}
#[derive(Clone, Default)]
struct OverlayView {
    phase: String,
    message: String,
    level: f32,
    seconds: f32,
}
static BRIDGE: OnceLock<Bridge> = OnceLock::new();
struct Painter {
    view: Arc<Mutex<OverlayView>>,
    tx: mpsc::Sender<Action>,
    tick: Cell<u32>,
    dpi: Cell<u32>,
    font: Cell<HFONT>,
    small: Cell<HFONT>,
    animated: bool,
    alpha: Cell<u8>,
    claimed: Rc<Cell<bool>>,
}
impl Drop for Painter {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.font.get().0));
            let _ = DeleteObject(HGDIOBJ(self.small.get().0));
        }
    }
}

pub fn start(tx: mpsc::Sender<Action>) -> Result<(), String> {
    let view = Arc::new(Mutex::new(OverlayView {
        phase: "idle".into(),
        ..OverlayView::default()
    }));
    let shared = view.clone();
    let (ready, rx) = mpsc::sync_channel::<Result<isize, String>>(1);
    std::thread::spawn(move || unsafe {
        let instance = match GetModuleHandleW(None) {
            Ok(h) => HINSTANCE(h.0),
            Err(_) => {
                let _ = ready.send(Err("Cannot initialize the native overlay.".into()));
                return;
            }
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: w!("HushlineNativeOverlay"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            let _ = ready.send(Err("Cannot register the native overlay.".into()));
            return;
        }
        let mut animations = BOOL(1);
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut animations as *mut _ as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let claimed = Rc::new(Cell::new(false));
        let painter = Rc::new(Painter {
            view: shared,
            tx,
            tick: Cell::new(0),
            dpi: Cell::new(0),
            font: Cell::new(HFONT::default()),
            small: Cell::new(HFONT::default()),
            animated: animations.as_bool(),
            alpha: Cell::new(245),
            claimed: claimed.clone(),
        });
        let pointer = Rc::into_raw(painter);
        match CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            w!("HushlineNativeOverlay"),
            w!("Hushline dictation"),
            WS_POPUP,
            0,
            0,
            360,
            64,
            None,
            None,
            Some(instance),
            Some(pointer as *const _),
        ) {
            Ok(hwnd) => {
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 245, LWA_ALPHA);
                let _ = ready.send(Ok(hwnd.0 as isize));
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let _ = DestroyWindow(hwnd);
            }
            Err(_) => {
                if !claimed.get() {
                    drop(Rc::from_raw(pointer));
                }
                let _ = ready.send(Err("Cannot create the native overlay.".into()));
            }
        }
    });
    let hwnd = rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "Native overlay did not initialize.")??;
    BRIDGE
        .set(Bridge {
            hwnd: AtomicIsize::new(hwnd),
            view,
        })
        .map_err(|_| "Native overlay is already running.".to_string())
}
pub fn update(view: &View) {
    if let Some(bridge) = BRIDGE.get() {
        if let Ok(mut v) = bridge.view.lock() {
            *v = OverlayView {
                phase: view.phase.clone(),
                message: view.message.clone(),
                level: view.level,
                seconds: view.seconds,
            };
        }
        let hwnd = HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _);
        unsafe {
            let _ = PostMessageW(Some(hwnd), UPDATE, WPARAM(0), LPARAM(0));
        }
    }
}
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}
unsafe fn fonts(hwnd: HWND, p: &Painter) {
    let dpi = GetDpiForWindow(hwnd).max(96);
    if p.dpi.get() == dpi {
        return;
    }
    for f in [p.font.get(), p.small.get()] {
        if !f.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(f.0));
        }
    }
    p.dpi.set(dpi);
    let make = |size: i32| {
        CreateFontW(
            -size * dpi as i32 / 96,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            0,
            w!("Segoe UI"),
        )
    };
    p.font.set(make(12));
    p.small.set(make(9));
}
unsafe fn position(hwnd: HWND, p: &Painter) {
    let monitor = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST);
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() {
        return;
    }
    fonts(hwnd, p);
    let scale = p.dpi.get() as f64 / 96.0;
    let width = (360.0 * scale) as i32;
    let height = (64.0 * scale) as i32;
    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
    let y = info.rcWork.bottom - height - (18.0 * scale) as i32;
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        x,
        y,
        width,
        height,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    // A synchronous WM_DPICHANGED may have resized the popup during SetWindowPos.
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let radius = (28.0 * p.dpi.get() as f64 / 96.0) as i32;
    let region = CreateRoundRectRgn(0, 0, client.right + 1, client.bottom + 1, radius, radius);
    if SetWindowRgn(hwnd, Some(region), true) == 0 {
        let _ = DeleteObject(HGDIOBJ(region.0));
    }
}
unsafe fn text(hdc: HDC, value: &str, rect: RECT, font: HFONT, color: COLORREF) {
    // An empty Vec has a dangling sentinel pointer. Some Windows text paths
    // inspect the buffer even for a zero character count; never pass it to GDI.
    if value.is_empty() {
        return;
    }
    let previous = SelectObject(hdc, HGDIOBJ(font.0));
    let _ = SetTextColor(hdc, color);
    let mut text: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    let count = text.len() - 1;
    let mut rect = rect;
    let _ = DrawTextW(
        hdc,
        &mut text[..count],
        &mut rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
    );
    let _ = SelectObject(hdc, previous);
}
unsafe fn paint(hwnd: HWND, p: &Painter) {
    fonts(hwnd, p);
    let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    let brush = CreateSolidBrush(if view.phase == "error" {
        rgb(59, 40, 49)
    } else {
        rgb(31, 44, 65)
    });
    let _ = FillRect(hdc, &client, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = SetBkMode(hdc, TRANSPARENT);
    let s = p.dpi.get() as f64 / 96.0;
    let px = |n: f64| (n * s).round() as i32;
    let light = rgb(241, 246, 255);
    let muted = rgb(163, 182, 211);
    let blue = rgb(174, 203, 250);
    let pen = CreatePen(PS_SOLID, px(2.0).max(1), blue);
    let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = RoundRect(
        hdc,
        px(18.0),
        px(17.0),
        px(27.0),
        px(34.0),
        px(8.0),
        px(8.0),
    );
    let _ = Arc(
        hdc,
        px(14.0),
        px(20.0),
        px(31.0),
        px(40.0),
        px(14.0),
        px(28.0),
        px(31.0),
        px(28.0),
    );
    let _ = MoveToEx(hdc, px(22.0), px(38.0), None);
    let _ = LineTo(hdc, px(22.0), px(44.0));
    let _ = SelectObject(hdc, old_pen);
    let _ = SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    text(
        hdc,
        &view.message,
        RECT {
            left: px(43.0),
            top: px(12.0),
            right: px(266.0),
            bottom: px(34.0),
        },
        p.font.get(),
        light,
    );
    let detail = match view.phase.as_str() {
        "listening" => format!("{:.1}s · Esc to cancel", view.seconds),
        "processing" => "Processing securely".into(),
        "error" => "Open Hushline for details".into(),
        "preview" => "Review in Hushline".into(),
        _ => "Your words, wherever you work".into(),
    };
    text(
        hdc,
        &detail,
        RECT {
            left: px(43.0),
            top: px(33.0),
            right: px(266.0),
            bottom: px(50.0),
        },
        p.small.get(),
        muted,
    );
    let wave_brush = CreateSolidBrush(blue);
    let previous = SelectObject(hdc, HGDIOBJ(wave_brush.0));
    let previous_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    for i in 0..11 {
        let variation = ((i as f32 * 1.71 + p.tick.get() as f32 * 0.45).sin().abs() * 0.75) + 0.25;
        let activity = if view.phase == "processing" {
            0.65
        } else if view.phase == "listening" {
            view.level
        } else {
            0.1
        };
        let height = 4.0 + activity * 25.0 * variation;
        let x = 277.0 + i as f64 * 3.0;
        let _ = RoundRect(
            hdc,
            px(x),
            px(32.0 - height as f64 / 2.0),
            px(x + 2.0),
            px(32.0 + height as f64 / 2.0),
            px(2.0),
            px(2.0),
        );
    }
    let _ = SelectObject(hdc, previous);
    let _ = SelectObject(hdc, previous_pen);
    let _ = DeleteObject(HGDIOBJ(wave_brush.0));
    let pen = CreatePen(PS_SOLID, px(1.2).max(1), muted);
    let old = SelectObject(hdc, HGDIOBJ(pen.0));
    let _ = MoveToEx(hdc, px(332.0), px(27.0), None);
    let _ = LineTo(hdc, px(340.0), px(35.0));
    let _ = MoveToEx(hdc, px(340.0), px(27.0), None);
    let _ = LineTo(hdc, px(332.0), px(35.0));
    let _ = SelectObject(hdc, old);
    let _ = DeleteObject(HGDIOBJ(pen.0));
    let _ = EndPaint(hwnd, &ps);
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        let data = &*(create.lpCreateParams as *const Painter);
        data.claimed.set(true);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return LRESULT(1);
    }
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Painter;
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    Rc::increment_strong_count(pointer);
    let p = Rc::from_raw(pointer);
    match message {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            paint(hwnd, &p);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            p.dpi.set(0);
            position(hwnd, &p);
            LRESULT(0)
        }
        UPDATE => {
            let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
            if view.phase == "idle" {
                let _ = ShowWindow(hwnd, SW_HIDE);
                let _ = KillTimer(Some(hwnd), 1);
                let _ = KillTimer(Some(hwnd), 2);
            } else {
                if !IsWindowVisible(hwnd).as_bool() {
                    position(hwnd, &p);
                    if p.animated {
                        p.alpha.set(90);
                        let _ =
                            SetLayeredWindowAttributes(hwnd, COLORREF(0), p.alpha.get(), LWA_ALPHA);
                        SetTimer(Some(hwnd), 2, 16, None);
                    }
                }
                if view.phase == "processing" && p.animated {
                    SetTimer(Some(hwnd), 1, 100, None);
                } else {
                    let _ = KillTimer(Some(hwnd), 1);
                }
                let title: Vec<u16> = view.message.encode_utf16().chain(Some(0)).collect();
                let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title.as_ptr()));
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == 2 {
                p.alpha.set(p.alpha.get().saturating_add(25).min(245));
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), p.alpha.get(), LWA_ALPHA);
                if p.alpha.get() == 245 {
                    let _ = KillTimer(Some(hwnd), 2);
                }
            } else {
                p.tick.set(p.tick.get().wrapping_add(1));
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = lparam.0 as u16 as i32;
            let scale = p.dpi.get() as f64 / 96.0;
            if x >= (322.0 * scale) as i32 {
                let _ = p.tx.send(Action::Cancel);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = p.tx.send(Action::Cancel);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Rc::from_raw(pointer));
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Shows the real native overlay briefly; requires a Windows desktop without concurrent focus changes."]
    fn native_overlay_does_not_activate() {
        let before = unsafe { GetForegroundWindow() };
        let (tx, _) = mpsc::channel();
        start(tx).unwrap();
        let initial = BRIDGE.get().unwrap();
        let initial_hwnd = HWND(initial.hwnd.load(Ordering::Relaxed) as *mut _);
        // Exercise the empty initial status that can be painted before first use.
        unsafe {
            let _ = SendMessageW(initial_hwnd, WM_PAINT, None, None);
        }
        update(&View {
            phase: "listening".into(),
            message: "Native overlay verification".into(),
            level: 0.6,
            seconds: 1.2,
            ..View::default()
        });
        let bridge = BRIDGE.get().unwrap();
        let hwnd = HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _);
        let wait = std::time::Instant::now();
        while !unsafe { IsWindowVisible(hwnd) }.as_bool() && wait.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(unsafe { IsWindowVisible(hwnd) }.as_bool());
        assert_eq!(
            unsafe { GetForegroundWindow() },
            before,
            "Overlay changed keyboard focus"
        );
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_ne!(ex & WS_EX_NOACTIVATE.0, 0);
        assert_ne!(ex & WS_EX_TOOLWINDOW.0, 0);
        update(&View {
            phase: "idle".into(),
            ..View::default()
        });
        let wait = std::time::Instant::now();
        while unsafe { IsWindowVisible(hwnd) }.as_bool() && wait.elapsed() < Duration::from_secs(2)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
    }
}
