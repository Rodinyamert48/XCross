//! Windows için yerel (Win32) crosshair overlay'i.
//!
//! # Neden egui penceresi değil?
//!
//! winit, tıklama geçirgenliği (`set_cursor_hittest(false)`) için pencereye
//! `WS_EX_LAYERED | WS_EX_TRANSPARENT` ekler ama ardından
//! `SetLayeredWindowAttributes` veya `UpdateLayeredWindow` çağırmaz. Microsoft
//! belgelerine göre böyle katmanlı bir pencere ekrana hiç çizilmez; sonuçta
//! crosshair Windows'ta görünmez olur.
//!
//! Bu modül crosshair overlay'lerinin standart yöntemini kullanır: yalnızca
//! crosshair kadar büyük, katmanlı, tıklama geçirgen, odak almayan ve en üstte
//! duran bir pencere. İçerik CPU'da çizilir (bkz. `raster`) ve piksel bazında
//! saydamlıkla `UpdateLayeredWindow` ile basılır. Oyunlar z-sırasını
//! değiştirebildiği için pencere hem ön plandaki pencere değiştiği anda
//! (alt-tab) hem de düzenli aralıklarla yeniden en üste alınır.

use crate::config::{Crosshair, OverlayPlacement};
use crate::raster;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{
    GetLastError, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
    GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, ReleaseDC,
    SelectObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EVENT_SYSTEM_FOREGROUND,
    GetMessageW, HTTRANSPARENT, HWND_TOPMOST, MA_NOACTIVATE, MSG, PostMessageW, RegisterClassExW,
    SW_HIDE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetTimer, SetWindowPos,
    ShowWindow, TranslateMessage, ULW_ALPHA, UpdateLayeredWindow, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, WM_APP, WM_DISPLAYCHANGE, WM_MOUSEACTIVATE, WM_NCHITTEST, WM_TIMER,
    WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

/// Yeni bir kare çizilmesi gerektiğini bildirir.
const WM_APP_UPDATE: u32 = WM_APP + 1;
/// Overlay iş parçacığını sonlandırır.
const WM_APP_QUIT: u32 = WM_APP + 2;
/// Pencereyi hemen yeniden en üste al.
const WM_APP_RAISE: u32 = WM_APP + 3;
/// En üstte kalma zamanlayıcısı.
const TOPMOST_TIMER_ID: usize = 1;
/// Oyunlar z-sırasını değiştirebildiği için pencere bu aralıkla yeniden
/// en üste alınır (ms).
const TOPMOST_INTERVAL_MS: u32 = 250;

/// Overlay'in göstermesi gereken her şey.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub crosshair: Crosshair,
    pub visible: bool,
    pub placement: OverlayPlacement,
}

#[derive(Default)]
struct Shared {
    pending: Option<Frame>,
    status: String,
}

/// Ayrı bir iş parçacığında çalışan yerel overlay penceresinin tutamacı.
pub struct NativeOverlay {
    hwnd: Arc<AtomicIsize>,
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<()>>,
    last_sent: Option<Frame>,
}

impl NativeOverlay {
    /// Overlay penceresini oluşturur. Pencere hazır olana kadar bekler.
    pub fn spawn() -> Result<Self, String> {
        let hwnd = Arc::new(AtomicIsize::new(0));
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let thread = {
            let hwnd = hwnd.clone();
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("xcross-overlay".into())
                .spawn(move || unsafe { run(hwnd, shared, ready_tx) })
                .map_err(|e| format!("overlay iş parçacığı başlatılamadı: {e}"))?
        };

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                hwnd,
                shared,
                thread: Some(thread),
                last_sent: None,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => Err("overlay iş parçacığı beklenmedik şekilde sonlandı".into()),
        }
    }

    /// Yeni kareyi gönderir; önceki kareyle aynıysa hiçbir şey yapmaz.
    pub fn update(&mut self, frame: Frame) {
        if self.last_sent.as_ref() == Some(&frame) {
            return;
        }
        self.last_sent = Some(frame.clone());
        if let Ok(mut s) = self.shared.lock() {
            s.pending = Some(frame);
        }
        let hwnd = self.hwnd.load(Ordering::Acquire) as HWND;
        if !hwnd.is_null() {
            unsafe {
                PostMessageW(hwnd, WM_APP_UPDATE, 0, 0);
            }
        }
    }

    /// Son çizimin durumu (hata varsa açıklaması).
    pub fn status(&self) -> String {
        self.shared
            .lock()
            .map(|s| s.status.clone())
            .unwrap_or_default()
    }
}

impl Drop for NativeOverlay {
    fn drop(&mut self) {
        let hwnd = self.hwnd.load(Ordering::Acquire) as HWND;
        if !hwnd.is_null() {
            unsafe {
                PostMessageW(hwnd, WM_APP_QUIT, 0, 0);
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Ön plan olayı geri çağrısının overlay penceresine ulaşması için.
/// Süreç başına tek overlay olduğundan genel bir değişken yeterli.
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);

/// Başka bir pencere ön plana geçtiğinde (ör. alt-tab ile oyuna dönüş)
/// çağrılır. Oyun kendini öne aldıysa overlay'i beklemeden tekrar öne al.
unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let hwnd = OVERLAY_HWND.load(Ordering::Acquire) as HWND;
    if !hwnd.is_null() {
        unsafe { PostMessageW(hwnd, WM_APP_RAISE, 0, 0) };
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Fare olaylarını her koşulda alttaki pencereye (oyuna) bırak.
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        // Tıklansa bile odağı oyundan çalma.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        // Çözünürlük/monitör değişti: konumu yeniden hesapla.
        WM_DISPLAYCHANGE => {
            unsafe { PostMessageW(hwnd, WM_APP_UPDATE, 1, 0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Overlay iş parçacığının gövdesi: pencereyi oluşturur ve mesaj döngüsünü
/// çalıştırır.
unsafe fn run(
    hwnd_out: Arc<AtomicIsize>,
    shared: Arc<Mutex<Shared>>,
    ready: mpsc::Sender<Result<(), String>>,
) {
    unsafe {
        // Koordinatlar fiziksel piksel olsun; ekran ölçeklemesi (%125, %150)
        // crosshair'i bulanıklaştırmasın ve merkezi kaydırmasın.
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let hinstance = GetModuleHandleW(null());
        let class_name = wide("XCrossOverlay");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: null_mut(),
        };
        // Sınıf zaten kayıtlıysa (overlay yeniden başlatıldıysa) hata önemsiz.
        RegisterClassExW(&wc);

        let title = wide("XCross Overlay");
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );
        if hwnd.is_null() {
            let _ = ready.send(Err(format!(
                "overlay penceresi oluşturulamadı (hata {})",
                GetLastError()
            )));
            return;
        }
        hwnd_out.store(hwnd as isize, Ordering::Release);
        OVERLAY_HWND.store(hwnd as isize, Ordering::Release);
        SetTimer(hwnd, TOPMOST_TIMER_ID, TOPMOST_INTERVAL_MS, None);
        // Bu iş parçacığının mesaj döngüsü olduğu için süreç dışı kanca
        // burada güvenle çalışır.
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        let _ = ready.send(Ok(()));

        let mut current: Option<Frame> = None;
        let mut shown = false;
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.hwnd == hwnd {
                match msg.message {
                    WM_APP_QUIT => {
                        DestroyWindow(hwnd);
                        break;
                    }
                    WM_APP_UPDATE => {
                        let pending = shared.lock().ok().and_then(|mut s| s.pending.take());
                        // wparam == 1: ekran değişti, mevcut kareyi yeniden çiz.
                        if let Some(frame) = pending.or_else(|| current.clone()) {
                            let status = present(hwnd, &frame, &mut shown);
                            if let Ok(mut s) = shared.lock() {
                                s.status = status;
                            }
                            current = Some(frame);
                        }
                        continue;
                    }
                    WM_APP_RAISE | WM_TIMER
                        if msg.message == WM_APP_RAISE || msg.wParam == TOPMOST_TIMER_ID =>
                    {
                        if shown {
                            SetWindowPos(
                                hwnd,
                                HWND_TOPMOST,
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if !hook.is_null() {
            UnhookWinEvent(hook);
        }
        OVERLAY_HWND.store(0, Ordering::Release);
        hwnd_out.store(0, Ordering::Release);
    }
}

/// Overlay'in hedeflediği ekran dikdörtgeni: (sol, üst, genişlik, yükseklik).
unsafe fn target_rect(p: &OverlayPlacement) -> (i32, i32, i32, i32) {
    if p.manual {
        return (
            p.x.round() as i32,
            p.y.round() as i32,
            p.width.round().max(1.0) as i32,
            p.height.round().max(1.0) as i32,
        );
    }
    unsafe {
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut info) != 0 {
            let RECT {
                left,
                top,
                right,
                bottom,
            } = info.rcMonitor;
            return (left, top, right - left, bottom - top);
        }
    }
    (0, 0, 1920, 1080)
}

/// Kareyi çizer ve pencereyi günceller. Durum metnini döndürür.
unsafe fn present(hwnd: HWND, frame: &Frame, shown: &mut bool) -> String {
    unsafe {
        let hide = |shown: &mut bool| {
            ShowWindow(hwnd, SW_HIDE);
            *shown = false;
        };
        if !frame.visible {
            hide(shown);
            return "gizli".into();
        }
        let Some(r) = raster::render(&frame.crosshair) else {
            hide(shown);
            return "çizilecek bir şey yok (opaklık 0?)".into();
        };

        let (mx, my, mw, mh) = target_rect(&frame.placement);
        let cx = mx + mw / 2 + frame.crosshair.offset[0].round() as i32;
        let cy = my + mh / 2 + frame.crosshair.offset[1].round() as i32;
        let (w, h) = (r.width() as i32, r.height() as i32);
        let pos = POINT {
            x: cx - r.origin.0,
            y: cy - r.origin.1,
        };
        let size = SIZE { cx: w, cy: h };
        let src = POINT { x: 0, y: 0 };

        let screen = GetDC(null_mut());
        let mem = CreateCompatibleDC(screen);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // yukarıdan aşağı satır düzeni
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits: *mut c_void = null_mut();
        let dib = CreateDIBSection(screen, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(mem);
            ReleaseDC(null_mut(), screen);
            return format!("bitmap oluşturulamadı (hata {})", GetLastError());
        }
        let pixels = r.bgra_premultiplied();
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        let old = SelectObject(mem, dib);

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let ok = UpdateLayeredWindow(hwnd, screen, &pos, &size, mem, &src, 0, &blend, ULW_ALPHA);
        let err = GetLastError();

        SelectObject(mem, old);
        DeleteObject(dib);
        DeleteDC(mem);
        ReleaseDC(null_mut(), screen);

        if ok == 0 {
            return format!("UpdateLayeredWindow başarısız (hata {err})");
        }
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        *shown = true;
        format!("çalışıyor — merkez ({cx}, {cy}), ekran {mw}×{mh}, pencere {w}×{h}")
    }
}
