#![cfg(windows)]
use auto_window_adjust::platform::fit_window;
use std::{
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::{
    core::w,
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
};

unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: isize) -> i32 {
    (&mut *(data as *mut Vec<HMONITOR>)).push(monitor);
    1
}

struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

unsafe fn visible(window: HWND) -> RECT {
    let mut frame: RECT = zeroed();
    assert_eq!(
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut frame as *mut _ as *mut _,
            size_of::<RECT>() as u32
        ),
        0
    );
    frame
}

fn coordinates(r: RECT) -> (i32, i32, i32, i32) {
    (r.left, r.top, r.right, r.bottom)
}

#[test]
fn real_windows_fit_each_monitor_and_remain_stable() {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mut monitors = Vec::<HMONITOR>::new();
        assert_ne!(
            EnumDisplayMonitors(
                null_mut(),
                null(),
                Some(collect),
                &mut monitors as *mut _ as isize
            ),
            0
        );
        assert!(!monitors.is_empty());
        for monitor in monitors {
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            assert_ne!(GetMonitorInfoW(monitor, &mut info), 0);
            let work = info.rcWork;
            let window = Window(CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("AutoWindowAdjust native test"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                work.left - 300,
                work.top + 20,
                work.right - work.left + 500,
                work.bottom - work.top + 300,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            ));
            assert!(!window.0.is_null());
            assert!(fit_window(window.0, monitor).unwrap());
            let frame = visible(window.0);
            assert!(
                frame.left >= work.left
                    && frame.top >= work.top
                    && frame.right <= work.right
                    && frame.bottom <= work.bottom,
                "window {:?} outside {:?}",
                coordinates(frame),
                coordinates(work)
            );
            assert!(!fit_window(window.0, monitor).unwrap());
            assert_eq!(coordinates(visible(window.0)), coordinates(frame));
            println!(
                "PASS native monitor work area {:?}: {:?}",
                coordinates(work),
                coordinates(frame)
            );

            // A fixed-size window must move without being shrunk.
            SetWindowLongW(
                window.0,
                GWL_STYLE,
                (WS_OVERLAPPED | WS_CAPTION | WS_VISIBLE) as i32,
            );
            SetWindowPos(
                window.0,
                null_mut(),
                work.left - 200,
                work.top + 30,
                work.right - work.left + 500,
                work.bottom - work.top + 300,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
            let before = visible(window.0);
            fit_window(window.0, monitor).unwrap();
            let after = visible(window.0);
            assert_eq!(
                (after.right - after.left, after.bottom - after.top),
                (before.right - before.left, before.bottom - before.top)
            );
            assert_eq!((after.left, after.top), (work.left, work.top));
        }
    }
}

#[test]
fn tray_hook_pause_resume_and_clean_exit() {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_AutoWindowAdjust"))
        .arg("--smoke-test")
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("tray smoke test timed out");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
