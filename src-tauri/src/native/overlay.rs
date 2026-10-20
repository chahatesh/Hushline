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
#[cfg(test)]
const QUERY_PAINT: u32 = WM_APP + 18;
struct Bridge {
    hwnd: AtomicIsize,
    view: Arc<Mutex<OverlayView>>,
}
#[derive(Clone, Default)]
struct OverlayView {
    phase: String,
    message: String,
    warning: String,
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
    paint_ok: Cell<bool>,
    layout: Cell<(bool, u8, u64)>,
    pill: Cell<RECT>,
    notice: Cell<RECT>,
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
            lpszClassName: w!("HushNativeOverlay"),
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
            paint_ok: Cell::new(false),
            layout: Cell::new((false, 255, 0)),
            pill: Cell::new(RECT::default()),
            notice: Cell::new(RECT::default()),
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
            w!("HushNativeOverlay"),
            w!("Hush dictation"),
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
                warning: view.warning.clone(),
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
    p.font.set(make(13));
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
        (false, false) => (48.0, 16.0),
        (true, false) => (136.0, 30.0),
        (false, true) => (16.0, 48.0),
        (true, true) => (30.0, 112.0),
    }
}
fn notice_text(view: &OverlayView) -> String {
    if view.phase == "idle" || view.phase == "listening" || view.phase == "processing" {
        return String::new();
    }
    let message = if view.phase == "error" {
        view.message.as_str()
    } else {
        ""
    };
    [message, view.warning.as_str()]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
        .chars()
        .take(1200)
        .collect()
}
fn notice_id(view: &OverlayView) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    notice_text(view).hash(&mut hash);
    hash.finish()
}
unsafe fn notice_height(hwnd: HWND, p: &Painter, value: &str, width: i32) -> i32 {
    if value.is_empty() {
        return 0;
    }
    let dc = GetDC(Some(hwnd));
    let old = SelectObject(dc, HGDIOBJ(p.font.get().0));
    let mut text: Vec<u16> = value.encode_utf16().collect();
    let scale = p.dpi.get() as f64 / 96.0;
    let mut rect = RECT {
        right: width - (28.0 * scale) as i32,
        ..RECT::default()
    };
    DrawTextW(
        dc,
        &mut text,
        &mut rect,
        DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
    );
    let _ = SelectObject(dc, old);
    ReleaseDC(Some(hwnd), dc);
    (rect.bottom + (28.0 * scale) as i32).clamp((60.0 * scale) as i32, (280.0 * scale) as i32)
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
}
unsafe fn position(hwnd: HWND, p: &Painter, animate: bool) {
    let view = p.view.lock().map(|v| v.clone()).unwrap_or_default();
    let active = view.phase != "idle" && !view.phase.is_empty();
    let dock = dock_id(&view.dock);
    if !active && dock == 3 {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = KillTimer(Some(hwnd), 2);
        p.layout.set((active, dock, notice_id(&view)));
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
    let gap = ((if active { 10.0 } else { 7.0 }) * scale) as i32;
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
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    };
    let notice = notice_text(&view);
    let card_width = ((340.0 * scale) as i32).min(info.rcWork.right - info.rcWork.left - 20);
    let card_height = notice_height(hwnd, p, &notice, card_width);
    if card_height > 0 {
        let full_width = width.max(card_width);
        let full_height = height + card_height + (8.0 * scale) as i32;
        rect.left = match dock {
            1 => x,
            2 => x + width - full_width,
            _ => x + (width - full_width) / 2,
        };
        rect.left = rect
            .left
            .clamp(info.rcWork.left, info.rcWork.right - full_width);
        rect.top = (y + height - full_height).max(info.rcWork.top);
        rect.right = rect.left + full_width;
        rect.bottom = rect.top + full_height;
        let pill_x = match dock {
            1 => 0,
            2 => full_width - width,
            _ => (full_width - width) / 2,
        };
        p.pill.set(RECT {
            left: pill_x,
            top: full_height - height,
            right: pill_x + width,
            bottom: full_height,
        });
        p.notice.set(RECT {
            left: 0,
            top: 0,
            right: full_width,
            bottom: card_height,
        });
    } else {
        p.pill.set(RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        });
        p.notice.set(RECT::default());
    }
    let alpha = if active { 255 } else { 165 };
    let mut from = RECT::default();
    let _ = GetWindowRect(hwnd, &mut from);
    let visible = IsWindowVisible(hwnd).as_bool();
    p.from.set(from);
    p.to.set(rect);
    p.fade_from.set(p.alpha.get());
    p.fade_to.set(alpha);
    // Mark the layout before SetWindowPos can synchronously reenter for DPI changes.
    let previous_notice = p.layout.get().2;
    p.layout.set((active, dock, notice_id(&view)));
    if animate && visible && p.animated && previous_notice == notice_id(&view) && card_height == 0 {
        p.transition.set(Some(Instant::now()));
        SetTimer(Some(hwnd), 2, 16, None);
    } else {
        p.transition.set(None);
        let _ = KillTimer(Some(hwnd), 2);
        p.alpha.set(alpha);
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
unsafe fn wrapped_text(hdc: HDC, value: &str, mut rect: RECT, font: HFONT) {
    let previous = SelectObject(hdc, HGDIOBJ(font.0));
    let _ = SetTextColor(hdc, rgb(255, 255, 255));
    let mut text: Vec<u16> = value.encode_utf16().collect();
    if !text.is_empty() {
        DrawTextW(
            hdc,
            &mut text,
            &mut rect,
            DT_LEFT | DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
    }
    let _ = SelectObject(hdc, previous);
}
// Signed distance to a rounded box gives fractional edge coverage at every DPI.
// Unlike a window region, per-pixel alpha retains the antialiased outer pixels.
fn coverage(x: f64, y: f64, width: f64, height: f64, radius: f64) -> f64 {
    let qx = (x - width / 2.0).abs() - (width / 2.0 - radius);
    let qy = (y - height / 2.0).abs() - (height / 2.0 - radius);
    let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}
fn composite(pixel: &mut u32, color: COLORREF, opacity: f64) {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    let inverse = 255 - alpha;
    let r = color.0 & 255;
    let g = (color.0 >> 8) & 255;
    let b = (color.0 >> 16) & 255;
    let mix = |source: u32, shift: u32| {
        (source * alpha + ((*pixel >> shift) & 255) * inverse + 127) / 255
    };
    *pixel = (alpha + ((*pixel >> 24) * inverse + 127) / 255) << 24
        | mix(r, 16) << 16
        | mix(g, 8) << 8
        | mix(b, 0);
}
fn rounded(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    rect: (f64, f64, f64, f64),
    color: COLORREF,
) {
    rounded_radius(pixels, width, height, rect, color, rect.2.min(rect.3) / 2.0);
}
fn rounded_radius(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    rect: (f64, f64, f64, f64),
    color: COLORREF,
    radius: f64,
) {
    let (left, top, w, h) = rect;
    for y in (top.floor() as i32 - 1).max(0)..((top + h).ceil() as i32 + 1).min(height) {
        for x in (left.floor() as i32 - 1).max(0)..((left + w).ceil() as i32 + 1).min(width) {
            let alpha = coverage(x as f64 + 0.5 - left, y as f64 + 0.5 - top, w, h, radius);
            composite(&mut pixels[(y * width + x) as usize], color, alpha);
        }
    }
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
    let (width, height) = (client.right.max(1), client.bottom.max(1));
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let Ok(bitmap) = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
        let _ = EndPaint(hwnd, &ps);
        return;
    };
    let dc = CreateCompatibleDC(Some(hdc));
    let old = SelectObject(dc, HGDIOBJ(bitmap.0));
    let memory = std::slice::from_raw_parts_mut(bits as *mut u32, (width * height) as usize);
    memory.fill(0);
    let scale = p.dpi.get() as f64 / 96.0;
    let pill = p.pill.get();
    let (ox, oy) = (pill.left as f64 / scale, pill.top as f64 / scale);
    let (w, h) = if p.notice.get().bottom > 0 {
        (
            (pill.right - pill.left) as f64 / scale,
            (pill.bottom - pill.top) as f64 / scale,
        )
    } else {
        (width as f64 / scale, height as f64 / scale)
    };
    let light = view.theme == "light";
    let background = if view.phase == "error" {
        rgb(84, 40, 45)
    } else if light {
        rgb(241, 245, 251)
    } else {
        rgb(27, 35, 48)
    };
    let ink = if light {
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
    let mut pixels = vec![0u32; memory.len()];
    rounded(
        &mut pixels,
        width,
        height,
        (
            ox * scale + 0.5,
            oy * scale + 0.5,
            w * scale - 1.0,
            h * scale - 1.0,
        ),
        background,
    );
    let mut shape = |x: f64, y: f64, w: f64, h: f64, color| {
        rounded(
            &mut pixels,
            width,
            height,
            ((x + ox) * scale, (y + oy) * scale, w * scale, h * scale),
            color,
        )
    };
    if !active {
        if side {
            shape((w - 2.0) / 2.0, (h - 18.0) / 2.0, 2.0, 18.0, accent)
        } else {
            shape((w - 20.0) / 2.0, (h - 2.0) / 2.0, 20.0, 2.0, accent)
        }
    } else {
        let level = if view.phase == "processing" {
            0.25
        } else if view.phase == "listening" {
            view.level
        } else {
            0.0
        };
        for i in 0..11 {
            let variation = 0.25 + (i as f32 * 1.31 + p.tick.get() as f32 * 0.4).sin().abs() * 0.75;
            let amplitude = 3.0 + level as f64 * 14.0 * variation as f64;
            if side {
                shape(
                    (w - amplitude) / 2.0,
                    13.0 + i as f64 * 5.0,
                    amplitude,
                    2.0,
                    accent,
                )
            } else {
                shape(
                    13.0 + i as f64 * 4.2,
                    (h - amplitude) / 2.0,
                    2.2,
                    amplitude,
                    accent,
                )
            }
        }
        // The cancel mark is deliberately small; its hit area remains generous.
        let (cx, cy) = if side {
            (w / 2.0, h - 15.0)
        } else {
            (w - 15.0, h / 2.0)
        };
        for i in -4..=4 {
            let v = i as f64 * 0.65;
            shape(cx + v - 0.6, cy + v - 0.6, 1.2, 1.2, ink);
            shape(cx + v - 0.6, cy - v - 0.6, 1.2, 1.2, ink);
        }
        if !side {
            let label = match view.phase.as_str() {
                "listening" if view.message == "Starting microphone" => "…".into(),
                "listening" => format!(
                    "{}:{:02}",
                    view.seconds as u32 / 60,
                    view.seconds as u32 % 60
                ),
                "processing" => "…".into(),
                "success" => "Done".into(),
                "error" => "!".into(),
                _ => "Edit".into(),
            };
            let _ = SetBkMode(dc, TRANSPARENT);
            text(
                dc,
                &label,
                RECT {
                    left: ((ox + 70.0) * scale) as i32,
                    top: (oy * scale) as i32,
                    right: ((ox + w - 29.0) * scale) as i32,
                    bottom: ((oy + h) * scale) as i32,
                },
                p.small.get(),
                rgb(255, 255, 255),
            );
            // GDI writes RGB glyph coverage; composite it into the premultiplied DIB.
            let _ = GdiFlush();
            for (pixel, mask) in pixels.iter_mut().zip(memory.iter()) {
                composite(pixel, ink, (mask & 255) as f64 / 255.0);
            }
        }
    }
    let card = p.notice.get();
    if card.bottom > 0 {
        rounded_radius(
            &mut pixels,
            width,
            height,
            (0.5, 0.5, card.right as f64 - 1.0, card.bottom as f64 - 1.0),
            if light {
                rgb(250, 250, 252)
            } else {
                rgb(32, 36, 44)
            },
            14.0 * scale,
        );
        memory.fill(0);
        let padding = (14.0 * scale) as i32;
        let _ = SetBkMode(dc, TRANSPARENT);
        wrapped_text(
            dc,
            &notice_text(&view),
            RECT {
                left: padding,
                top: padding,
                right: card.right - padding,
                bottom: card.bottom - padding,
            },
            p.font.get(),
        );
        let _ = GdiFlush();
        for (pixel, mask) in pixels.iter_mut().zip(memory.iter()) {
            composite(
                pixel,
                if light {
                    rgb(42, 47, 58)
                } else {
                    rgb(240, 242, 247)
                },
                (mask & 255) as f64 / 255.0,
            );
        }
    }
    memory.copy_from_slice(&pixels);
    let _ = GdiFlush();
    let painted = UpdateLayeredWindow(
        hwnd,
        None,
        None,
        Some(&SIZE {
            cx: width,
            cy: height,
        }),
        Some(dc),
        Some(&POINT { x: 0, y: 0 }),
        COLORREF(0),
        Some(&BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: p.alpha.get(),
            AlphaFormat: AC_SRC_ALPHA as u8,
        }),
        ULW_ALPHA,
    );
    p.paint_ok.set(painted.is_ok());
    #[cfg(test)]
    if active && p.alpha.get() == 255 && p.transition.get().is_none() {
        if let Some(directory) = std::env::var_os("HUSHLINE_OVERLAY_CAPTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            let _ = std::fs::create_dir_all(&directory);
            let data: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
            let _ = std::fs::write(directory.join(format!("bar-{width}x{height}.bgra")), data);
        }
    }
    let _ = SelectObject(dc, old);
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
        #[cfg(test)]
        QUERY_PAINT => LRESULT(p.alpha.get() as isize | if p.paint_ok.get() { 256 } else { 0 }),
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
            if p.layout.get() != (active, dock, notice_id(&view)) {
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
                "Hush · {}",
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
            let pill = p.pill.get();
            let in_pill = x >= pill.left && x < pill.right && y >= pill.top && y < pill.bottom;
            let cancel = in_pill
                && view.phase != "idle"
                && if side {
                    y >= client.bottom - (30.0 * scale) as i32
                } else {
                    x >= pill.right - (28.0 * scale) as i32
                };
            let action = if cancel {
                Action::Cancel
            } else if !in_pill || view.phase == "error" || view.phase == "preview" {
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
    fn pill_edges_have_fractional_alpha_and_correct_premultiplication() {
        assert_eq!(coverage(0.0, 0.0, 48.0, 16.0, 8.0), 0.0);
        assert_eq!(coverage(24.0, 8.0, 48.0, 16.0, 8.0), 1.0);
        let alpha = coverage(3.5, 1.5, 48.0, 16.0, 8.0);
        assert!(alpha > 0.0 && alpha < 1.0);
        let mut pixel = 0;
        composite(&mut pixel, rgb(255, 100, 40), alpha);
        assert_eq!((pixel >> 16) & 255, pixel >> 24);
        assert!((pixel & 255) < pixel >> 24);
    }
    #[test]
    fn notices_include_the_reason_and_recovery_warning_but_not_old_session_text() {
        let mut v = OverlayView {
            phase: "error".into(),
            message: "Microphone unavailable".into(),
            warning: "Choose another device in Settings".into(),
            ..OverlayView::default()
        };
        assert_eq!(
            notice_text(&v),
            "Microphone unavailable\n\nChoose another device in Settings"
        );
        v.phase = "listening".into();
        assert!(notice_text(&v).is_empty());
        v.phase = "success".into();
        assert_eq!(notice_text(&v), "Choose another device in Settings");
    }
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
        assert_eq!(bounds.right - bounds.left, (136.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (30.0 * scale) as i32);
        let state = unsafe {
            SendMessageW(hwnd, WM_PAINT, None, None);
            SendMessageW(hwnd, QUERY_PAINT, None, None).0
        };
        assert_eq!(state & 255, 255, "Active bar must be fully opaque");
        assert_ne!(
            state & 256,
            0,
            "Per-pixel alpha surface was rejected by Windows"
        );
        configure(&Settings {
            dock: "left".into(),
            ..Settings::default()
        });
        std::thread::sleep(Duration::from_millis(250));
        unsafe {
            GetWindowRect(hwnd, &mut bounds).unwrap();
        }
        assert_eq!(bounds.right - bounds.left, (30.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (112.0 * scale) as i32);
        // The real wrapped error surface must remain nonactivating at every dock.
        for dock in ["bottom", "left", "right"] {
            configure(&Settings {
                dock: dock.into(),
                ..Settings::default()
            });
            update(&View { phase:"error".into(),
                message:"Your microphone is unavailable. Reconnect it or choose another microphone in Settings.".into(),
                warning:"Click this message to open Hush. Your clipboard has not changed.".into(),
                ..View::default() });
            std::thread::sleep(Duration::from_millis(220));
            unsafe {
                GetWindowRect(hwnd, &mut bounds).unwrap();
            }
            assert!(bounds.right - bounds.left >= (330.0 * scale) as i32);
            assert!(bounds.bottom - bounds.top > (90.0 * scale) as i32);
            assert_eq!(unsafe { GetForegroundWindow() }, before);
            let painted = unsafe {
                SendMessageW(hwnd, WM_PAINT, None, None);
                SendMessageW(hwnd, QUERY_PAINT, None, None).0
            };
            assert_ne!(painted & 256, 0, "Error card failed to paint");
        }
        configure(&Settings {
            dock: "left".into(),
            ..Settings::default()
        });
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
        assert_eq!(bounds.right - bounds.left, (16.0 * scale) as i32);
        assert_eq!(bounds.bottom - bounds.top, (48.0 * scale) as i32);
        configure(&Settings {
            dock: "hidden".into(),
            ..Settings::default()
        });
        std::thread::sleep(Duration::from_millis(200));
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
    }
}
