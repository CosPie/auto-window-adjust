//! Win32 owns the message loop. Hooks and window callbacks run on this thread;
//! no Rust state borrow is kept across a call that can dispatch more messages.
use crate::geometry::{fit, Rect};
use std::{
    cell::Cell,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    ptr::{null, null_mut},
};
use windows_sys::{
    core::w,
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{LibraryLoader::GetModuleHandleW, Registry::*, Threading::CreateMutexW},
        UI::{
            Accessibility::*, HiDpi::*, Input::KeyboardAndMouse::*, Shell::*,
            WindowsAndMessaging::*,
        },
    },
};

const TRAY_MESSAGE: u32 = WM_APP + 1;
const SETTLE_TIMER: usize = 1;
const SMOKE_TIMER: usize = 2;
const TOGGLE: usize = 100;
const STARTUP: usize = 101;
const HELP: usize = 102;
const EXIT: usize = 103;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const RUN_KEY: *const u16 = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_NAME: *const u16 = w!("AutoWindowAdjust");

#[derive(Clone, Copy)]
struct Drag {
    window: HWND,
    monitor: HMONITOR,
    bounds: Rect,
    cursor: POINT,
}
#[derive(Clone, Copy)]
struct Pending {
    window: HWND,
    monitor: HMONITOR,
    process: u32,
}

thread_local! {
    static APP_WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
    static ENABLED: Cell<bool> = const { Cell::new(true) };
    static DRAG: Cell<Option<Drag>> = const { Cell::new(None) };
    static PENDING: Cell<Option<Pending>> = const { Cell::new(None) };
    static STATUS: Cell<&'static str> = const { Cell::new("等待跨屏拖动") };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

unsafe fn message(text: &str) {
    MessageBoxW(
        APP_WINDOW.get(),
        wide(text).as_ptr(),
        w!("跨屏收窗"),
        MB_OK | MB_ICONINFORMATION,
    );
}

unsafe fn key_down(key: u16) -> bool {
    GetAsyncKeyState(i32::from(key)) < 0
}

unsafe fn eligible(window: HWND) -> bool {
    !window.is_null()
        && IsWindow(window) != 0
        && IsWindowVisible(window) != 0
        && IsIconic(window) == 0
        && IsZoomed(window) == 0
        && GetAncestor(window, GA_ROOT) == window
        && GetWindowLongW(window, GWL_STYLE) as u32 & WS_CHILD == 0
        && GetWindowLongW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW == 0
}

fn rect(r: RECT) -> Rect {
    Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top)
}

unsafe fn bounds(window: HWND) -> Option<RECT> {
    let mut r = zeroed();
    (GetWindowRect(window, &mut r) != 0).then_some(r)
}

unsafe fn visible(window: HWND, fallback: RECT) -> Rect {
    let mut frame: RECT = zeroed();
    if DwmGetWindowAttribute(
        window,
        DWMWA_EXTENDED_FRAME_BOUNDS as u32,
        &mut frame as *mut _ as *mut _,
        size_of::<RECT>() as u32,
    ) == 0
        && frame.right > frame.left
        && frame.bottom > frame.top
    {
        rect(frame)
    } else {
        rect(fallback)
    }
}

/// Adjust a live window. Caller must supply valid window and monitor handles.
/// Returns whether a change was requested, or the Win32 failure code.
///
/// # Safety
/// Handles must refer to a window and monitor on the current Windows desktop.
pub unsafe fn fit_window(window: HWND, monitor: HMONITOR) -> Result<bool, u32> {
    let outer = bounds(window).ok_or_else(|| GetLastError())?;
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return Err(GetLastError());
    }
    let frame = visible(window, outer);
    let resizable = GetWindowLongW(window, GWL_STYLE) as u32 & WS_THICKFRAME != 0;
    let fitted = fit(frame, rect(info.rcWork), resizable);
    if fitted == frame {
        return Ok(false);
    }
    let left = frame.x - outer.left;
    let top = frame.y - outer.top;
    let right = outer.right - frame.x - frame.width;
    let bottom = outer.bottom - frame.y - frame.height;
    if SetWindowPos(
        window,
        null_mut(),
        fitted.x - left,
        fitted.y - top,
        fitted.width + left + right,
        fitted.height + top + bottom,
        SWP_NOACTIVATE | SWP_NOZORDER,
    ) == 0
    {
        return Err(GetLastError());
    }
    // A window may enforce a minimum size. Re-read actual bounds and fix position only.
    if let Some(actual) = bounds(window) {
        let frame = visible(window, actual);
        let corrected = fit(frame, rect(info.rcWork), false);
        if (corrected.x, corrected.y) != (frame.x, frame.y)
            && SetWindowPos(
                window,
                null_mut(),
                actual.left + corrected.x - frame.x,
                actual.top + corrected.y - frame.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
            ) == 0
        {
            return Err(GetLastError());
        }
    }
    Ok(true)
}

unsafe extern "system" fn win_event(
    _: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if event == EVENT_SYSTEM_MOVESIZESTART {
        KillTimer(APP_WINDOW.get(), SETTLE_TIMER);
        PENDING.set(None);
        DRAG.set(None);
        if !ENABLED.get() || !eligible(window) || !key_down(VK_LBUTTON) || key_down(VK_MENU) {
            return;
        }
        let mut cursor: POINT = zeroed();
        if GetCursorPos(&mut cursor) == 0 {
            return;
        }
        let Some(bounds) = bounds(window) else {
            return;
        };
        let packed =
            ((cursor.y as u32 & 0xffff) << 16 | (cursor.x as u32 & 0xffff)) as i32 as isize;
        let mut hit = 0;
        if SendMessageTimeoutW(
            window,
            WM_NCHITTEST,
            0,
            packed,
            SMTO_ABORTIFHUNG,
            50,
            &mut hit,
        ) != 0
            && (HTLEFT as usize..=HTBOTTOMRIGHT as usize).contains(&hit)
        {
            return;
        }
        DRAG.set(Some(Drag {
            window,
            monitor: MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST),
            bounds: rect(bounds),
            cursor,
        }));
    } else if event == EVENT_SYSTEM_MOVESIZEEND {
        let Some(start) = DRAG.get().filter(|drag| drag.window == window) else {
            return;
        };
        DRAG.set(None);
        if !ENABLED.get() || key_down(VK_MENU) || !eligible(window) {
            return;
        }
        let mut cursor: POINT = zeroed();
        if GetCursorPos(&mut cursor) == 0 {
            return;
        }
        let Some(end) = bounds(window) else {
            return;
        };
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONULL);
        if monitor.is_null()
            || monitor == start.monitor
            || (end.left, end.top) == (start.bounds.x, start.bounds.y)
            || (i64::from(cursor.x).abs_diff(i64::from(start.cursor.x)) < 4
                && i64::from(cursor.y).abs_diff(i64::from(start.cursor.y)) < 4)
        {
            return;
        }
        let mut process = 0;
        GetWindowThreadProcessId(window, &mut process);
        PENDING.set(Some(Pending {
            window,
            monitor,
            process,
        }));
        if SetTimer(APP_WINDOW.get(), SETTLE_TIMER, 180, None) == 0 {
            PENDING.set(None);
            STATUS.set("无法创建延迟调整计时器");
        }
    }
}

unsafe fn apply_pending() {
    KillTimer(APP_WINDOW.get(), SETTLE_TIMER);
    let Some(item) = PENDING.take() else {
        return;
    };
    if !ENABLED.get() || key_down(VK_MENU) || key_down(VK_LBUTTON) || !eligible(item.window) {
        return;
    }
    let mut process = 0;
    GetWindowThreadProcessId(item.window, &mut process);
    if process != item.process {
        return;
    }
    match fit_window(item.window, item.monitor) {
        Ok(true) => STATUS.set("已将窗口收进目标屏幕"),
        Ok(false) => {}
        Err(_) => STATUS.set("调整失败：应用可能以管理员身份运行"),
    }
}

fn startup_command() -> std::io::Result<Vec<u16>> {
    let path = std::env::current_exe()?;
    Ok(std::iter::once(b'"' as u16)
        .chain(path.as_os_str().encode_wide())
        .chain([b'"' as u16, 0])
        .collect())
}

unsafe fn startup_enabled() -> bool {
    let Ok(command) = startup_command() else {
        return false;
    };
    let mut bytes = 0;
    if RegGetValueW(
        HKEY_CURRENT_USER,
        RUN_KEY,
        RUN_NAME,
        RRF_RT_REG_SZ,
        null_mut(),
        null_mut(),
        &mut bytes,
    ) != ERROR_SUCCESS
        || !(2..=65536).contains(&bytes)
    {
        return false;
    }
    let mut value = vec![0u16; bytes.div_ceil(2) as usize];
    if RegGetValueW(
        HKEY_CURRENT_USER,
        RUN_KEY,
        RUN_NAME,
        RRF_RT_REG_SZ,
        null_mut(),
        value.as_mut_ptr().cast(),
        &mut bytes,
    ) != ERROR_SUCCESS
    {
        return false;
    }
    let end = value.iter().position(|&ch| ch == 0).unwrap_or(value.len());
    value[..end] == command[..command.len() - 1]
}

unsafe fn set_startup() {
    let remove = startup_enabled();
    let mut key = null_mut();
    let result = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        RUN_KEY,
        0,
        null(),
        REG_OPTION_NON_VOLATILE,
        KEY_SET_VALUE,
        null(),
        &mut key,
        null_mut(),
    );
    if result != ERROR_SUCCESS {
        message(&format!("无法更改开机启动：Windows 错误 {result}"));
        return;
    }
    let result = if remove {
        RegDeleteValueW(key, RUN_NAME)
    } else {
        match startup_command() {
            Ok(command) => RegSetValueExW(
                key,
                RUN_NAME,
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            ),
            Err(_) => ERROR_PATH_NOT_FOUND,
        }
    };
    RegCloseKey(key);
    if result != ERROR_SUCCESS {
        message(&format!("无法更改开机启动：Windows 错误 {result}"));
    }
}

fn copy_wide<const N: usize>(dest: &mut [u16; N], text: &str) {
    for (slot, value) in dest.iter_mut().take(N - 1).zip(text.encode_utf16()) {
        *slot = value;
    }
}

unsafe fn tray_data() -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = APP_WINDOW.get();
    data.uID = 1;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    data.uCallbackMessage = TRAY_MESSAGE;
    data.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
    copy_wide(
        &mut data.szTip,
        if ENABLED.get() {
            "跨屏收窗 · 已开启（双击暂停）"
        } else {
            "跨屏收窗 · 已暂停（双击恢复）"
        },
    );
    data
}

unsafe fn add_tray(balloon: bool) -> bool {
    let mut data = tray_data();
    if balloon {
        data.uFlags |= NIF_INFO;
        data.dwInfoFlags = NIIF_INFO | NIIF_RESPECT_QUIET_TIME;
        copy_wide(&mut data.szInfoTitle, "跨屏收窗已启动");
        copy_wide(
            &mut data.szInfo,
            "拖动窗口到另一块屏幕，松手后自动收好。按住 Alt 可跳过。右键托盘图标可暂停或退出。",
        );
    }
    if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
        return false;
    }
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    Shell_NotifyIconW(NIM_SETVERSION, &data);
    true
}

unsafe fn toggle() {
    ENABLED.set(!ENABLED.get());
    DRAG.set(None);
    PENDING.set(None);
    KillTimer(APP_WINDOW.get(), SETTLE_TIMER);
    Shell_NotifyIconW(NIM_MODIFY, &tray_data());
}

unsafe fn show_help() {
    message("把普通窗口拖到另一块屏幕，松手后自动收进该屏幕。\n\n• 放得下：保留大小，只调整位置。\n• 放不下：按比例缩小，避开任务栏。\n• 按住 Alt 松手：跳过本次调整。\n• 同屏拖动、边框缩放、最大化和最小化窗口不处理。\n\n右键托盘图标可暂停、开机启动或退出；双击切换暂停。\n\n部分应用有最小尺寸限制；普通权限无法调整管理员应用。\n移动程序后，请重新勾选开机启动以更新路径。\n\nAutoWindowAdjust 0.2.0 · Rust / Win32 · MIT");
}

unsafe fn show_menu() {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    AppendMenuW(
        menu,
        MF_STRING | if ENABLED.get() { MF_CHECKED } else { 0 },
        TOGGLE,
        if ENABLED.get() {
            w!("自动收窗：已开启")
        } else {
            w!("自动收窗：已暂停")
        },
    );
    AppendMenuW(
        menu,
        MF_STRING | if startup_enabled() { MF_CHECKED } else { 0 },
        STARTUP,
        w!("开机启动"),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING | MF_DISABLED,
        0,
        wide(STATUS.get()).as_ptr(),
    );
    AppendMenuW(menu, MF_STRING, HELP, w!("使用说明"));
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, EXIT, w!("退出"));
    let mut cursor: POINT = zeroed();
    GetCursorPos(&mut cursor);
    SetForegroundWindow(APP_WINDOW.get());
    let command = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
        cursor.x,
        cursor.y,
        0,
        APP_WINDOW.get(),
        null(),
    ) as usize;
    PostMessageW(APP_WINDOW.get(), WM_NULL, 0, 0);
    DestroyMenu(menu);
    match command {
        TOGGLE => toggle(),
        STARTUP => set_startup(),
        HELP => show_help(),
        EXIT => {
            DestroyWindow(APP_WINDOW.get());
        }
        _ => {}
    }
}

unsafe extern "system" fn window_proc(window: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        TRAY_MESSAGE => match lp as u32 & 0xffff {
            WM_LBUTTONDBLCLK => toggle(),
            WM_CONTEXTMENU | NIN_KEYSELECT => show_menu(),
            _ => {}
        },
        WM_TIMER if wp == SETTLE_TIMER => apply_pending(),
        WM_TIMER if wp == SMOKE_TIMER => {
            toggle();
            let paused = !ENABLED.get();
            toggle();
            if !paused || !ENABLED.get() {
                std::process::exit(1);
            }
            DestroyWindow(window);
        }
        WM_CLOSE => {
            DestroyWindow(window);
        }
        WM_DESTROY => {
            KillTimer(window, SETTLE_TIMER);
            KillTimer(window, SMOKE_TIMER);
            PENDING.set(None);
            DRAG.set(None);
            Shell_NotifyIconW(NIM_DELETE, &tray_data());
            PostQuitMessage(0);
        }
        _ if msg != 0 && msg == TASKBAR_CREATED.get() => {
            // Explorer restarts must not make the app impossible to pause or exit.
            add_tray(false);
        }
        _ => return DefWindowProcW(window, msg, wp, lp),
    }
    0
}

pub fn run() {
    unsafe {
        let smoke = std::env::args().any(|arg| arg == "--smoke-test");
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(
            null(),
            0,
            if smoke {
                w!("Local\\AutoWindowAdjust.SmokeTest")
            } else {
                w!("Local\\AutoWindowAdjust.Tray")
            },
        );
        if mutex.is_null() {
            fatal("无法创建单实例锁。", smoke);
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            message("工具已经在运行，请在任务栏右下角的托盘中查找图标。");
            return;
        }
        let instance = GetModuleHandleW(null());
        let mut class: WNDCLASSW = zeroed();
        class.lpfnWndProc = Some(window_proc);
        class.hInstance = instance;
        class.lpszClassName = w!("AutoWindowAdjust.TrayWindow");
        if RegisterClassW(&class) == 0 {
            CloseHandle(mutex);
            fatal("无法注册托盘窗口。", smoke);
        }
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.lpszClassName,
            w!("跨屏收窗"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if window.is_null() {
            CloseHandle(mutex);
            fatal("无法创建托盘窗口。", smoke);
        }
        APP_WINDOW.set(window);
        TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
        let hook = SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZEEND,
            null_mut(),
            Some(win_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if hook.is_null() || !add_tray(!smoke) {
            if !hook.is_null() {
                UnhookWinEvent(hook);
            }
            DestroyWindow(window);
            CloseHandle(mutex);
            fatal("无法启动事件监听或托盘图标。", smoke);
        }
        if smoke && SetTimer(window, SMOKE_TIMER, 500, None) == 0 {
            UnhookWinEvent(hook);
            DestroyWindow(window);
            CloseHandle(mutex);
            fatal("无法创建验证计时器。", smoke);
        }
        let mut msg: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnhookWinEvent(hook);
        Shell_NotifyIconW(NIM_DELETE, &tray_data());
        APP_WINDOW.set(null_mut());
        CloseHandle(mutex);
    }
}

unsafe fn fatal(text: &str, smoke: bool) -> ! {
    if !smoke {
        message(text);
    }
    std::process::exit(1)
}
