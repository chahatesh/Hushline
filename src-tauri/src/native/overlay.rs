// SPDX-License-Identifier: MIT
//! Nonactivating native overlay. No WebView, idle animation, or audio dependency.
use crate::{
    model::{Settings, View},
    shortcuts::Action,
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{
        atomic::{AtomicIsize, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
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
    dock: String,
    theme: String,
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
    layout: Cell<(bool, u8)>,
    from: Cell<RECT>,
    to: Cell<RECT>,
    fade_from: Cell<u8>,
    fade_to: Cell<u8>,
    transition: Cell<Option<Instant>>,
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
            alpha: Cell::new(165),
            layout: Cell::new((false, 255)),
            from: Cell::new(RECT::default()),
            to: Cell::new(RECT::default()),
            fade_from: Cell::new(165),
            fade_to: Cell::new(165),
            transition: Cell::new(None),
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
                dock: v.dock.clone(),
                theme: v.theme.clone(),
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
pub fn configure(settings: &Settings) {
    if let Some(bridge) = BRIDGE.get() {
        if let Ok(mut v) = bridge.view.lock() {
            v.dock = settings.dock.clone();
            v.theme = settings.theme.clone();
        }
        unsafe {
            let _ = PostMessageW(
                Some(HWND(bridge.hwnd.load(Ordering::Relaxed) as *mut _)),
                UPDATE,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}
fn dock_id(value: &str) -> u8 {
    match value {
        "left" => 1,
        "right" => 2,
        "hidden" => 3,
        _ => 0,
    }
}
fn dimensions(active: bool, side: bool) -> (f64, f64) {
    match (active, side) {
        (false, false) => (72.0, 24.0),
        (true, false) => (216.0, 40.0),
        (false, true) => (24.0, 64.0),
        (true, true) => (40.0, 156.0),
    }
}
unsafe fn geometry(hwnd: HWND, _p: &Painter, rect: RECT) {
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    let height = rect.bottom - rect.top;
    let width = rect.right - rect.left;
    let radius = height.min(width);
    let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
    if SetWindowRgn(hwnd, Some(region), true) == 0 {
        let _ = DeleteObject(HGDIOBJ(region.0));
    }
}
unsafe fn position(hwnd: HWND, p: &Painter, animate: bool) {
    let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
    let active = view.phase != "idle" && !view.phase.is_empty();
    let dock = dock_id(&view.dock);
    if !active && dock == 3 {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = KillTimer(Some(hwnd), 2);
        p.layout.set((active, dock));
        return;
    }
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
    let (width, height) = dimensions(active, dock == 1 || dock == 2);
    let width = (width * scale) as i32;
    let height = (height * scale) as i32;
    let gap = ((if active { 14.0 } else { 8.0 }) * scale) as i32;
    let (x, y) = match dock {
        1 => (
            info.rcWork.left + gap,
            info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 2,
        ),
        2 => (
            info.rcWork.right - width - gap,
            info.rcWork.top + (info.rcWork.bottom - info.rcWork.top - height) / 2,
        ),
        _ => (
            info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2,
            info.rcWork.bottom - height - gap,
        ),
    };
    let rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    let alpha = if active { 255 } else { 165 };
    let mut from = RECT::default();
    let _ = GetWindowRect(hwnd, &mut from);
    let visible = IsWindowVisible(hwnd).as_bool();
    p.from.set(from);
    p.to.set(rect);
    p.fade_from.set(p.alpha.get());
    p.fade_to.set(alpha);
    // Mark the layout before SetWindowPos can synchronously reenter for DPI changes.
    p.layout.set((active, dock));
    if animate && visible && p.animated {
        p.transition.set(Some(Instant::now()));
        SetTimer(Some(hwnd), 2, 16, None);
    } else {
        p.transition.set(None);
        let _ = KillTimer(Some(hwnd), 2);
        p.alpha.set(alpha);
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        geometry(hwnd, p, rect);
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
    let active = view.phase != "idle" && !view.phase.is_empty();
    let side = matches!(dock_id(&view.dock), 1 | 2);
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut client = RECT::default();
    let _ = GetClientRect(hwnd, &mut client);
    // Double buffer the GDI surface so voice updates do not flicker.
    let dc = CreateCompatibleDC(Some(hdc));
    let bitmap = CreateCompatibleBitmap(hdc, client.right.max(1), client.bottom.max(1));
    let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
    let light_theme = view.theme == "light";
    let background = if view.phase == "error" {
        rgb(84, 40, 45)
    } else if light_theme {
        rgb(241, 245, 251)
    } else {
        rgb(27, 35, 48)
    };
    let brush = CreateSolidBrush(background);
    let _ = FillRect(dc, &client, brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
    let _ = SetBkMode(dc, TRANSPARENT);
    let s = p.dpi.get() as f64 / 96.0;
    let px = |n: f64| (n * s).round() as i32;
    let width = client.right as f64 / s;
    let height = client.bottom as f64 / s;
    let color = if light_theme {
        rgb(43, 68, 102)
    } else {
        rgb(218, 231, 249)
    };
    let accent = if view.phase == "error" {
        rgb(255, 184, 174)
    } else if view.phase == "success" {
        rgb(142, 220, 180)
    } else {
        rgb(158, 190, 237)
    };
    if !active {
        let brush = CreateSolidBrush(accent);
        let previous = SelectObject(dc, HGDIOBJ(brush.0));
        let pen = SelectObject(dc, GetStockObject(NULL_PEN));
        let (w, h) = if side { (3.0, 22.0) } else { (26.0, 3.0) };
        let _ = RoundRect(
            dc,
            px((width - w) / 2.0),
            px((height - h) / 2.0),
            px((width + w) / 2.0),
            px((height + h) / 2.0),
            px(3.0),
            px(3.0),
        );
        let _ = SelectObject(dc, previous);
        let _ = SelectObject(dc, pen);
        let _ = DeleteObject(HGDIOBJ(brush.0));
    } else {
        let cx = if side { width / 2.0 } else { 23.0 };
        let cy = if side { 24.0 } else { height / 2.0 };
        let pen = CreatePen(PS_SOLID, px(1.5).max(1), color);
        let previous = SelectObject(dc, HGDIOBJ(pen.0));
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        let _ = RoundRect(
            dc,
            px(cx - 3.5),
            px(cy - 8.0),
            px(cx + 3.5),
            px(cy + 3.0),
            px(7.0),
            px(7.0),
        );
        let _ = Arc(
            dc,
            px(cx - 7.0),
            px(cy - 5.0),
            px(cx + 7.0),
            px(cy + 7.0),
            px(cx - 7.0),
            px(cy),
            px(cx + 7.0),
            px(cy),
        );
        let _ = MoveToEx(dc, px(cx), px(cy + 7.0), None);
        let _ = LineTo(dc, px(cx), px(cy + 10.0));
        let _ = SelectObject(dc, previous);
        let _ = SelectObject(dc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
        let wave_brush = CreateSolidBrush(accent);
        let previous = SelectObject(dc, HGDIOBJ(wave_brush.0));
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        let level = if view.phase == "processing" {
            0.35
        } else if view.phase == "listening" {
            view.level
        } else {
            0.0
        };
        for i in 0..13 {
            let variation = 0.25 + (i as f32 * 1.31 + p.tick.get() as f32 * 0.4).sin().abs() * 0.75;
            let amplitude = 3.0 + level as f64 * 19.0 * variation as f64;
            if side {
                let y = 48.0 + i as f64 * 4.0;
                let _ = RoundRect(
                    dc,
                    px((width - amplitude) / 2.0),
                    px(y),
                    px((width + amplitude) / 2.0),
                    px(y + 2.0),
                    px(2.0),
                    px(2.0),
                );
            } else {
                let x = 48.0 + i as f64 * 4.0;
                let _ = RoundRect(
                    dc,
                    px(x),
                    px((height - amplitude) / 2.0),
                    px(x + 2.0),
                    px((height + amplitude) / 2.0),
                    px(2.0),
                    px(2.0),
                );
            }
        }
        let _ = SelectObject(dc, previous);
        let _ = SelectObject(dc, old_pen);
        let _ = DeleteObject(HGDIOBJ(wave_brush.0));
        if !side {
            let label = match view.phase.as_str() {
                "listening" if view.message == "Starting microphone" => "Starting".into(),
                "listening" => format!(
                    "{}:{:02}",
                    view.seconds as u32 / 60,
                    view.seconds as u32 % 60
                ),
                "processing" => "Working".into(),
                "success" => "Done".into(),
                "error" => "Open app".into(),
                _ => "Review".into(),
            };
            text(
                dc,
                &label,
                RECT {
                    left: px(113.0),
                    top: 0,
                    right: px(width - 35.0),
                    bottom: client.bottom,
                },
                p.small.get(),
                color,
            );
        }
        let (x, y) = if side {
            (width / 2.0, height - 24.0)
        } else {
            (width - 21.0, height / 2.0)
        };
        let pen = CreatePen(PS_SOLID, px(1.2).max(1), color);
        let old = SelectObject(dc, HGDIOBJ(pen.0));
        let _ = MoveToEx(dc, px(x - 3.0), px(y - 3.0), None);
        let _ = LineTo(dc, px(x + 3.0), px(y + 3.0));
        let _ = MoveToEx(dc, px(x + 3.0), px(y - 3.0), None);
        let _ = LineTo(dc, px(x - 3.0), px(y + 3.0));
        let _ = SelectObject(dc, old);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }
    let _ = BitBlt(
        hdc,
        0,
        0,
        client.right,
        client.bottom,
        Some(dc),
        0,
        0,
        SRCCOPY,
    );
    let _ = SelectObject(dc, old_bitmap);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(dc);
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
            position(hwnd, &p, false);
            LRESULT(0)
        }
        UPDATE => {
            let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
            let active = view.phase != "idle" && !view.phase.is_empty();
            let dock = dock_id(&view.dock);
            if p.layout.get() != (active, dock) {
                // Snap between docks/monitors; only the small activation lift animates.
                let same_dock = p.layout.get().1 == dock;
                let same_monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
                    == MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST);
                position(hwnd, &p, same_dock && same_monitor);
            }
            if view.phase == "processing" && p.animated {
                SetTimer(Some(hwnd), 1, 100, None);
            } else {
                let _ = KillTimer(Some(hwnd), 1);
            }
            let title: Vec<u16> = format!(
                "Hushline · {}",
                if active {
                    view.message.as_str()
                } else {
                    "Ready — click to dictate; right-click for settings"
                }
            )
            .encode_utf16()
            .chain(Some(0))
            .collect();
            let _ = SetWindowTextW(hwnd, windows::core::PCWSTR(title.as_ptr()));
            let _ = InvalidateRect(Some(hwnd), None, false);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == 2 {
                if let Some(started) = p.transition.get() {
                    let t = (started.elapsed().as_secs_f64() / 0.16).min(1.0);
                    let eased = 1.0 - (1.0 - t).powi(3);
                    let mix = |a: i32, b: i32| (a as f64 + (b - a) as f64 * eased).round() as i32;
                    let a = p.from.get();
                    let b = p.to.get();
                    p.alpha
                        .set(mix(p.fade_from.get() as i32, p.fade_to.get() as i32) as u8);
                    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), p.alpha.get(), LWA_ALPHA);
                    geometry(
                        hwnd,
                        &p,
                        RECT {
                            left: mix(a.left, b.left),
                            top: mix(a.top, b.top),
                            right: mix(a.right, b.right),
                            bottom: mix(a.bottom, b.bottom),
                        },
                    );
                    let _ = InvalidateRect(Some(hwnd), None, false);
                    if t >= 1.0 {
                        p.transition.set(None);
                        let _ = KillTimer(Some(hwnd), 2);
                    }
                }
            } else {
                p.tick.set(p.tick.get().wrapping_add(1));
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            position(hwnd, &p, false);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
            let mut client = RECT::default();
            let _ = GetClientRect(hwnd, &mut client);
            let x = lparam.0 as u16 as i32;
            let y = (lparam.0 >> 16) as u16 as i32;
            let side = matches!(dock_id(&view.dock), 1 | 2);
            let scale = p.dpi.get() as f64 / 96.0;
            let cancel = view.phase != "idle"
                && if side {
                    y >= client.bottom - (42.0 * scale) as i32
                } else {
                    x >= client.right - (38.0 * scale) as i32
                };
            let action = if cancel {
                Action::Cancel
            } else if view.phase == "error" || view.phase == "preview" {
                Action::Open
            } else {
                Action::HandsFree
            };
            let _ = p.tx.send(action);
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            let _ = p.tx.send(Action::Open);
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
        std::thread::sleep(Duration::from_millis(250));
        let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
        let mut bounds = RECT::default();
        unsafe {
            GetWindowRect(hwnd, &mut bounds).unwrap();
        }
        assert_eq!(bounds.right - bounds.left, (216.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (40.0 * scale) as i32);
        let mut alpha = 0;
        unsafe {
            GetLayeredWindowAttributes(hwnd, None, Some(&mut alpha), None).unwrap();
        }
        assert_eq!(alpha, 255, "Active bar must be fully opaque");
        configure(&Settings {
            dock: "left".into(),
            ..Settings::default()
        });
        std::thread::sleep(Duration::from_millis(250));
        unsafe {
            GetWindowRect(hwnd, &mut bounds).unwrap();
        }
        assert_eq!(bounds.right - bounds.left, (40.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (156.0 * scale) as i32);
        update(&View {
            phase: "idle".into(),
            ..View::default()
        });
        let wait = std::time::Instant::now();
        while wait.elapsed() < Duration::from_millis(250) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(unsafe { IsWindowVisible(hwnd) }.as_bool());
        unsafe {
            GetWindowRect(hwnd, &mut bounds).unwrap();
        }
        assert_eq!(bounds.right - bounds.left, (24.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (64.0 * scale) as i32);
        configure(&Settings {
            dock: "hidden".into(),
            ..Settings::default()
        });
        std::thread::sleep(Duration::from_millis(200));
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
    }
}
