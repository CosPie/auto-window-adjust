using System.ComponentModel;
using System.Runtime.InteropServices;

namespace AutoWindowAdjust;

internal sealed class WindowWatcher : IDisposable
{
    private readonly Native.WinEventProc callback;
    private readonly nint hook;
    private readonly System.Windows.Forms.Timer settle = new() { Interval = 180 };
    private Drag? drag;
    private Pending? pending;
    internal bool Enabled { get; set; } = true;
    internal Action<string>? Report { get; set; }
    private sealed record Drag(nint Window, nint Monitor, Rectangle Bounds, Native.Point Cursor);
    private sealed record Pending(nint Window, nint Monitor);

    internal WindowWatcher()
    {
        callback = OnEvent;
        // Only move/size events; no polling, no DLL injection, ignore our own UI.
        hook = Native.SetWinEventHook(0x000A, 0x000B, 0, callback, 0, 0, 0x0002);
        if (hook == 0) throw new Win32Exception(Marshal.GetLastWin32Error());
        settle.Tick += (_, _) => ApplyPending();
    }

    private void OnEvent(nint hookHandle, uint eventType, nint hwnd, int objectId, int childId, uint threadId, uint time)
    {
        // Never let managed exceptions cross the unmanaged callback boundary.
        try
        {
            if (eventType == 0x000A)
            {
                settle.Stop(); pending = null; drag = null;
                if (!Enabled || !Native.Eligible(hwnd) || !Native.KeyDown(1) || Native.KeyDown(0x12)
                    || !Native.GetCursorPos(out var cursor) || !Native.GetWindowRect(hwnd, out var rect)) return;
                var packed = (nint)unchecked((int)(((uint)cursor.Y & 0xffff) << 16 | ((uint)cursor.X & 0xffff)));
                // Exclude border resizing; custom title bars usually return HTCAPTION/HTCLIENT.
                if (Native.SendMessageTimeout(hwnd, 0x0084, 0, packed, 0x0002, 50, out var hit) != 0
                    && hit.ToInt64() is >= 10 and <= 17) return;
                drag = new(hwnd, Native.MonitorFromWindow(hwnd, 2), rect.Rectangle, cursor);
            }
            else if (eventType == 0x000B && drag is { } start && start.Window == hwnd)
            {
                drag = null;
                if (!Enabled || Native.KeyDown(0x12) || !Native.Eligible(hwnd)
                    || !Native.GetCursorPos(out var cursor) || !Native.GetWindowRect(hwnd, out var rect)) return;
                var target = Native.MonitorFromPoint(cursor, 0);
                if (target == 0 || target == start.Monitor || rect.Rectangle.Location == start.Bounds.Location
                    || (Math.Abs(cursor.X - start.Cursor.X) < 4 && Math.Abs(cursor.Y - start.Cursor.Y) < 4)) return;
                pending = new(hwnd, target);
                settle.Start(); // Let the target app finish its DPI/snap transition first.
            }
        }
        catch (Exception ex) { Report?.Invoke($"跳过窗口：{ex.Message}"); }
    }

    private void ApplyPending()
    {
        settle.Stop();
        var item = pending; pending = null;
        if (item == null || !Enabled || Native.KeyDown(0x12) || Native.KeyDown(1) || !Native.Eligible(item.Window)) return;
        FitWindow(item.Window, item.Monitor);
    }

    internal void FitWindow(nint window, nint targetMonitor)
    {
        var item = new Pending(window, targetMonitor);
        if (!Native.GetWindowRect(item.Window, out var outer)) return;
        var monitor = new Native.MonitorInfo { Size = Marshal.SizeOf<Native.MonitorInfo>() };
        if (!Native.GetMonitorInfo(item.Monitor, ref monitor)) return;
        var frame = VisibleBounds(item.Window, outer.Rectangle);
        bool resizable = (Native.GetWindowLong(item.Window, -16) & 0x00040000) != 0;
        var fit = WindowGeometry.Fit(frame, monitor.Work.Rectangle, resizable);
        if (fit == frame) return;
        // Convert visible frame coordinates back to GetWindowRect coordinates.
        var left = frame.Left - outer.Left; var top = frame.Top - outer.Top;
        var right = outer.Right - frame.Right; var bottom = outer.Bottom - frame.Bottom;
        if (!Native.SetWindowPos(item.Window, 0, fit.X - left, fit.Y - top,
            fit.Width + left + right, fit.Height + top + bottom, 0x0014))
        {
            Report?.Invoke("调整失败：该应用可能以管理员身份运行。");
            return;
        }
        // Applications can enforce minimum sizes. Read the actual result and correct only position.
        if (Native.GetWindowRect(item.Window, out var actual))
        {
            var actualFrame = VisibleBounds(item.Window, actual.Rectangle);
            var corrected = WindowGeometry.Fit(actualFrame, monitor.Work.Rectangle, false);
            if (corrected.Location != actualFrame.Location)
                Native.SetWindowPos(item.Window, 0, actual.Left + corrected.X - actualFrame.X,
                    actual.Top + corrected.Y - actualFrame.Y, 0, 0, 0x0015);
        }
        Report?.Invoke("已将窗口收进目标屏幕");
    }

    private static Rectangle VisibleBounds(nint hwnd, Rectangle fallback)
    {
        return Native.DwmGetWindowAttribute(hwnd, 9, out var frame, Marshal.SizeOf<Native.Rect>()) == 0
            && frame.Right > frame.Left && frame.Bottom > frame.Top ? frame.Rectangle : fallback;
    }

    public void Dispose()
    {
        settle.Stop(); settle.Dispose(); Native.UnhookWinEvent(hook);
        GC.KeepAlive(callback);
    }
}
