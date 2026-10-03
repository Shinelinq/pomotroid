//! Mini-only native boundary notifications. Keep Tauri/Tao's original drag path;
//! WM_EXITSIZEMOVE, never an idle Moved timeout, acknowledges the actual release.
use std::ffi::c_void;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewWindow};

type Hwnd = *mut c_void;
type Subclass = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;
#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(hwnd: Hwnd, callback: Subclass, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: Hwnd, callback: Subclass, id: usize) -> i32;
    fn DefSubclassProc(hwnd: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
}
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(key: i32) -> i16;
}
struct Context {
    app: AppHandle,
    token: u64,
}

unsafe extern "system" fn callback(
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    data: usize,
) -> isize {
    // SAFETY: the allocation is installed on this HWND and freed at WM_NCDESTROY.
    let context = unsafe { &*(data as *const Context) };
    // Clone before dispatch: nested native messages may destroy the HWND/context.
    let app = context.app.clone();
    let token = context.token;
    let state = app.state::<super::MiniState>();
    if message == 0x00A1 && wparam == 2 {
        // WM_NCLBUTTONDOWN / HTCAPTION
        if state.position_locked.load(Ordering::Acquire) || unsafe { GetAsyncKeyState(1) } >= 0 {
            super::drag_released(&app);
            return 0;
        }
    }
    if message == 0x0231 {
        state.drag_entered.store(true, Ordering::Release);
    } // WM_ENTERSIZEMOVE
    if message == 0x0112
        && wparam & 0xfff0 == 0xf010
        && state.position_locked.load(Ordering::Acquire)
    {
        return 0;
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == 0x0232 {
        super::drag_released(&app);
    } // WM_EXITSIZEMOVE
    if message == 0x007E || message == 0x001A {
        // WM_DISPLAYCHANGE / WM_SETTINGCHANGE
        super::schedule_position(&app, token);
    }
    if message == 0x0082 {
        // WM_NCDESTROY
        super::drag_released(&app);
        unsafe {
            RemoveWindowSubclass(hwnd, callback, id);
            drop(Box::from_raw(data as *mut Context));
        }
    }
    result
}

pub fn install(window: &WebviewWindow, token: u64) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as usize;
    let app = window.app_handle().clone();
    window
        .run_on_main_thread(move || {
            let owner = app.clone();
            let context = Box::into_raw(Box::new(Context { app, token }));
            // SAFETY: executes on the owning UI thread; the callback retains its context.
            let installed =
                unsafe { SetWindowSubclass(hwnd as Hwnd, callback, 0x504f4d4f, context as usize) }
                    != 0;
            owner
                .state::<super::MiniState>()
                .native_ready
                .store(installed, Ordering::Release);
            if !installed {
                unsafe {
                    drop(Box::from_raw(context));
                }
                log::error!("[mini] native drag boundary hook could not be installed");
            }
        })
        .map_err(|e| e.to_string())
}
