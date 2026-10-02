//! Automatic translation only reads real UI Automation selections. Unlike the
//! manual selection shortcut, it must never synthesize Copy or read a clipboard.

use tauri::Manager;

const DEFAULT_DELAY_MS: u64 = 200;

fn positive_delay(value: &serde_json::Value) -> Result<u64, String> {
    value.as_u64().filter(|delay| *delay > 0).ok_or_else(|| {
        "The selection translation delay must be a positive integer in milliseconds".into()
    })
}

#[tauri::command]
pub fn set_auto_selection_translate(enabled: bool) -> Result<(), String> {
    apply_enabled(enabled)?;
    if crate::config::get("auto_selection_translate").and_then(|value| value.as_bool())
        != Some(enabled)
    {
        crate::config::set("auto_selection_translate", enabled);
    }
    crate::APP
        .get()
        .unwrap()
        .emit_all("auto_selection_translate_changed", enabled)
        .map_err(|error| error.to_string())?;
    crate::tray::sync_auto_selection_item();
    Ok(())
}

#[tauri::command]
pub fn set_auto_selection_delay(delay_ms: serde_json::Value) -> Result<(), String> {
    let delay = positive_delay(&delay_ms)?;
    crate::config::set("auto_selection_translate_delay_ms", delay);
    #[cfg(target_os = "windows")]
    platform::set_delay(delay);
    crate::APP
        .get()
        .unwrap()
        .emit_all("auto_selection_translate_delay_ms_changed", delay)
        .map_err(|error| error.to_string())
}

fn apply_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return platform::set_enabled(enabled);
    #[cfg(not(target_os = "windows"))]
    if enabled {
        Err("Automatic selection translation is only supported on Windows".into())
    } else {
        Ok(())
    }
}

pub fn sync_from_config() {
    let delay = crate::config::get("auto_selection_translate_delay_ms")
        .as_ref()
        .and_then(|value| positive_delay(value).ok())
        .unwrap_or(DEFAULT_DELAY_MS);
    #[cfg(target_os = "windows")]
    platform::set_delay(delay);
    let enabled = crate::config::get("auto_selection_translate")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if let Err(error) = apply_enabled(enabled) {
        log::warn!("Automatic selection translation: {}", error);
    }
    crate::tray::sync_auto_selection_item();
    let _ = crate::APP
        .get()
        .unwrap()
        .emit_all("auto_selection_translate_changed", enabled);
    let _ = crate::APP
        .get()
        .unwrap()
        .emit_all("auto_selection_translate_delay_ms_changed", delay);
}

#[cfg(any(target_os = "windows", test))]
fn moved(start: (i32, i32), end: (i32, i32)) -> bool {
    let dx = i64::from(start.0) - i64::from(end.0);
    let dy = i64::from(start.1) - i64::from(end.1);
    dx.abs() > 4 || dy.abs() > 4 || dx * dx + dy * dy > 16
}

#[cfg(any(target_os = "windows", test))]
struct Press {
    point: (i32, i32),
    window: isize,
    dragged: bool,
    double_click: bool,
}

#[cfg(any(target_os = "windows", test))]
#[derive(Default)]
struct Gesture {
    press: Option<Press>,
    last_click: Option<(std::time::Instant, (i32, i32), isize)>,
}

#[cfg(any(target_os = "windows", test))]
impl Gesture {
    fn begin(&mut self, point: (i32, i32), window: isize, double_click_time: std::time::Duration) {
        let double_click = self
            .last_click
            .as_ref()
            .map_or(false, |(time, previous, target)| {
                *target == window && time.elapsed() <= double_click_time && !moved(*previous, point)
            });
        self.press = Some(Press {
            point,
            window,
            dragged: false,
            double_click,
        });
    }

    fn update(&mut self, point: (i32, i32)) {
        if let Some(press) = self.press.as_mut() {
            press.dragged |= moved(press.point, point);
        }
    }

    fn release(&mut self, point: (i32, i32), window: isize) -> bool {
        let Some(press) = self.press.take() else {
            return false;
        };
        if window != press.window {
            self.last_click = None;
            return false;
        }
        let dragged = press.dragged || moved(press.point, point);
        self.last_click = if dragged {
            None
        } else {
            Some((std::time::Instant::now(), point, window))
        };
        dragged || press.double_click
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::Gesture;
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::Mutex;
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_ScrollBarControlTypeId,
        UIA_TextPatternId,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime;
    use windows::Win32::UI::WindowsAndMessaging::*;

    static ENABLED: AtomicBool = AtomicBool::new(false);
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    static DELAY_MS: AtomicU64 = AtomicU64::new(super::DEFAULT_DELAY_MS);
    static EVENTS: OnceCell<Sender<SelectionEvent>> = OnceCell::new();
    static HOOK: Lazy<Mutex<Option<HookThread>>> = Lazy::new(|| Mutex::new(None));

    struct HookThread {
        id: u32,
        join: JoinHandle<()>,
    }

    #[derive(Clone, Copy)]
    struct SelectionEvent {
        window: isize,
        process: u32,
        point: (i32, i32),
        generation: u64,
        released_at: Instant,
        delay: Duration,
    }

    thread_local! {
        static GESTURE: RefCell<Gesture> = RefCell::new(Gesture::default());
    }

    pub fn set_delay(delay: u64) {
        if DELAY_MS.swap(delay, Ordering::SeqCst) != delay {
            GENERATION.fetch_add(1, Ordering::SeqCst);
        }
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        let mut hook = HOOK.lock().map_err(|error| error.to_string())?;
        if enabled == hook.is_some() {
            return Ok(());
        }
        // Invalidates delayed reads and language detection immediately.
        ENABLED.store(false, Ordering::SeqCst);
        GENERATION.fetch_add(1, Ordering::SeqCst);
        if let Some(running) = hook.as_ref() {
            unsafe { PostThreadMessageW(running.id, WM_QUIT, WPARAM(0), LPARAM(0)) }
                .map_err(|error| error.to_string())?;
            let running = hook.take().unwrap();
            let _ = running.join.join();
        }
        if !enabled {
            return Ok(());
        }

        EVENTS.get_or_try_init(|| -> Result<_, String> {
            let (sender, receiver) = mpsc::channel();
            let (ready_sender, ready_receiver) = mpsc::channel();
            thread::spawn(move || selection_worker(receiver, ready_sender));
            ready_receiver.recv().map_err(|error| error.to_string())??;
            Ok(sender)
        })?;
        let (ready_sender, ready_receiver) = mpsc::channel();
        let join = thread::spawn(move || unsafe {
            let install = || -> windows::core::Result<_> {
                let module = GetModuleHandleW(None)?;
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HINSTANCE(module.0), 0)
            };
            match install() {
                Ok(handle) => {
                    // Ensure PostThreadMessage can reach this thread before exposing its id.
                    let mut message = MSG::default();
                    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                    let _ = ready_sender.send(Ok(GetCurrentThreadId()));
                    while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    let _ = UnhookWindowsHookEx(handle);
                }
                Err(error) => {
                    let _ = ready_sender.send(Err(error.to_string()));
                }
            }
        });
        match ready_receiver.recv() {
            Ok(Ok(id)) => {
                *hook = Some(HookThread { id, join });
                ENABLED.store(true, Ordering::SeqCst);
                Ok(())
            }
            result => {
                let _ = join.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "Mouse listener failed to start".into(),
                })
            }
        }
    }

    unsafe fn foreground() -> (isize, u32) {
        let window = GetForegroundWindow();
        let mut process = 0;
        GetWindowThreadProcessId(window, Some(&mut process));
        (window.0 as isize, process)
    }

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENABLED.load(Ordering::SeqCst) {
            let data = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            if data.flags & LLMHF_INJECTED == 0 {
                let point = (data.pt.x, data.pt.y);
                let message = wparam.0 as u32;
                GESTURE.with(|state| {
                    let mut gesture = state.borrow_mut();
                    match message {
                        WM_LBUTTONDOWN => {
                            GENERATION.fetch_add(1, Ordering::SeqCst);
                            // Low-level down hooks run before Windows activates the
                            // clicked app. Hit-test instead of using the old foreground.
                            let window = GetAncestor(WindowFromPoint(data.pt), GA_ROOT);
                            let mut process = 0;
                            GetWindowThreadProcessId(window, Some(&mut process));
                            let window = window.0 as isize;
                            if window == 0 || process == std::process::id() {
                                *gesture = Gesture::default();
                                return;
                            }
                            gesture.begin(
                                point,
                                window,
                                Duration::from_millis(GetDoubleClickTime().into()),
                            );
                        }
                        WM_MOUSEMOVE => {
                            gesture.update(point);
                        }
                        WM_LBUTTONUP => {
                            let (window, process) = foreground();
                            if gesture.release(point, window) {
                                if let Some(sender) = EVENTS.get() {
                                    // Never block the system hook on a slow application/provider.
                                    let _ = sender.send(SelectionEvent {
                                        window,
                                        process,
                                        point,
                                        generation: GENERATION.load(Ordering::SeqCst),
                                        released_at: Instant::now(),
                                        delay: Duration::from_millis(
                                            DELAY_MS.load(Ordering::SeqCst),
                                        ),
                                    });
                                }
                            }
                        }
                        WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN | WM_MOUSEWHEEL
                        | WM_MOUSEHWHEEL => {
                            GENERATION.fetch_add(1, Ordering::SeqCst);
                            *gesture = Gesture::default();
                        }
                        _ => {}
                    }
                });
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    fn is_current(event: &SelectionEvent) -> bool {
        ENABLED.load(Ordering::SeqCst)
            && event.generation == GENERATION.load(Ordering::SeqCst)
            && unsafe { foreground() } == (event.window, event.process)
    }

    fn selection_worker(receiver: Receiver<SelectionEvent>, ready: Sender<Result<(), String>>) {
        unsafe {
            if let Err(error) = CoInitializeEx(None, COINIT_MULTITHREADED).ok() {
                let _ = ready.send(Err(error.to_string()));
                return;
            }
        }
        let automation: windows::core::Result<IUIAutomation> =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) };
        if let Ok(automation) = automation {
            if ready.send(Ok(())).is_err() {
                unsafe { CoUninitialize() };
                return;
            }
            let mut previous: Option<(isize, String, Instant)> = None;
            while let Ok(mut event) = receiver.recv() {
                // Each newer gesture replaces the pending read and restarts the delay.
                while let Ok(newer) =
                    receiver.recv_timeout(event.delay.saturating_sub(event.released_at.elapsed()))
                {
                    event = newer;
                }
                if !is_current(&event) {
                    continue;
                }
                let text = unsafe { selected_text(&automation, &event) }.unwrap_or_default();
                if text.is_empty() || !is_current(&event) {
                    continue;
                }
                if previous.as_ref().map_or(false, |(window, old, time)| {
                    *window == event.window
                        && *old == text
                        && time.elapsed() < Duration::from_secs(1)
                }) {
                    continue;
                }
                if crate::lang_detect::should_auto_translate(&text) && is_current(&event) {
                    previous = Some((event.window, text.clone(), Instant::now()));
                    // Window construction must run on the Tauri event loop.
                    let _ = crate::APP.get().unwrap().run_on_main_thread(move || {
                        if is_current(&event) {
                            crate::window::text_translate(text);
                        }
                    });
                }
            }
        } else if let Err(error) = automation {
            let _ = ready.send(Err(error.to_string()));
        }
        unsafe { CoUninitialize() };
    }

    unsafe fn selected_text(
        automation: &IUIAutomation,
        event: &SelectionEvent,
    ) -> windows::core::Result<String> {
        let point = POINT {
            x: event.point.0,
            y: event.point.1,
        };
        let mut element = automation.ElementFromPoint(point)?;
        let walker = automation.ControlViewWalker()?;
        // Text children often expose their selection on an ancestor document.
        for _ in 0..8 {
            if element.CurrentProcessId()? as u32 != event.process
                || element.CurrentIsPassword()?.as_bool()
                || element.CurrentControlType()? == UIA_ScrollBarControlTypeId
            {
                return Ok(String::new());
            }
            if let Ok(pattern) =
                element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
            {
                return read_selection(&pattern);
            }
            element = walker.GetParentElement(&element)?;
        }
        Ok(String::new())
    }

    unsafe fn read_selection(pattern: &IUIAutomationTextPattern) -> windows::core::Result<String> {
        let ranges = pattern.GetSelection()?;
        let mut result = String::new();
        for index in 0..ranges.Length()? {
            // A collapsed selection returns an empty string, never the document text.
            let text = ranges.GetElement(index)?.GetText(-1)?.to_string();
            if !result.is_empty() && !text.is_empty() {
                result.push('\n');
            }
            result.push_str(&text);
        }
        Ok(result.trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{moved, positive_delay, Gesture};
    use std::time::{Duration, Instant};

    #[test]
    fn delay_accepts_only_positive_integer_numbers() {
        for invalid in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("200"),
            serde_json::json!(null),
            serde_json::json!(true),
        ] {
            assert!(positive_delay(&invalid).is_err());
        }
        assert_eq!(positive_delay(&serde_json::json!(1)).unwrap(), 1);
        assert_eq!(positive_delay(&serde_json::json!(200)).unwrap(), 200);
        assert_eq!(positive_delay(&serde_json::json!(5000)).unwrap(), 5000);
    }

    #[test]
    fn tolerates_click_jitter_but_recognizes_drag() {
        assert!(!moved((10, 10), (12, 12)));
        assert!(moved((10, 10), (20, 10)));
        assert!(moved((-1920, 0), (-1900, 0)));
    }

    #[test]
    fn single_click_does_not_check_an_old_selection() {
        let mut gesture = Gesture::default();
        gesture.begin((10, 10), 1, Duration::from_millis(500));
        gesture.update((12, 12));
        assert!(!gesture.release((12, 12), 1));
    }

    #[test]
    fn double_click_checks_selection_but_separate_clicks_do_not() {
        let mut gesture = Gesture::default();
        gesture.begin((10, 10), 1, Duration::from_millis(500));
        assert!(!gesture.release((10, 10), 1));
        gesture.begin((10, 10), 1, Duration::from_millis(500));
        assert!(gesture.release((10, 10), 1));
        gesture.last_click = Some((Instant::now() - Duration::from_secs(1), (10, 10), 1));
        gesture.begin((10, 10), 1, Duration::from_millis(500));
        assert!(!gesture.release((10, 10), 1));
    }

    #[test]
    fn drag_is_recorded_even_when_cursor_returns_to_start() {
        let mut gesture = Gesture::default();
        gesture.begin((-100, 10), 1, Duration::from_millis(500));
        gesture.update((100, 10));
        assert!(gesture.release((-100, 10), 1));
    }

    #[test]
    fn changing_windows_or_resetting_cancels_the_gesture() {
        let mut gesture = Gesture::default();
        gesture.begin((10, 10), 1, Duration::from_millis(500));
        assert!(!gesture.release((100, 10), 2));
        gesture.begin((10, 10), 2, Duration::from_millis(500));
        assert!(!gesture.release((10, 10), 2));
        gesture.begin((10, 10), 2, Duration::from_millis(500));
        gesture = Gesture::default();
        assert!(!gesture.release((100, 10), 2));
    }
}
