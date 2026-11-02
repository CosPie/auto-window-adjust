using AutoWindowAdjust;
using System.Runtime.InteropServices;

internal static class Program
{
    [STAThread]
    private static int Main()
    {
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        var cases = new (string Name, Rectangle Window, Rectangle Work, bool Resize, Rectangle Expected)[]
        {
            ("already fits", new(100,100,800,600), new(0,0,1920,1040), true, new(100,100,800,600)),
            ("straddles left edge", new(-200,100,800,600), new(0,0,1920,1040), true, new(0,100,800,600)),
            ("straddles right edge", new(1700,100,800,600), new(0,0,1920,1040), true, new(1120,100,800,600)),
            ("taskbar", new(100,800,800,600), new(0,0,1920,1040), true, new(100,440,800,600)),
            ("landscape to portrait", new(-800,100,1600,900), new(-1080,0,1080,1880), true, new(-1080,100,1080,607)),
            ("portrait to landscape", new(100,0,900,1800), new(0,0,1920,1040), true, new(100,0,520,1040)),
            ("monitor above", new(100,-50,800,600), new(0,-1080,1920,1040), true, new(100,-640,800,600)),
            ("left taskbar", new(0,10,800,600), new(60,0,1860,1080), true, new(60,10,800,600)),
            ("fixed oversized", new(-500,50,2000,1200), new(0,0,1920,1040), false, new(0,0,2000,1200)),
            ("never enlarge", new(10,10,100,100), new(0,0,3840,2160), true, new(10,10,100,100)),
            ("invalid area", new(10,10,100,100), Rectangle.Empty, true, new(10,10,100,100)),
        };
        try
        {
            foreach (var c in cases)
            {
                var actual = WindowGeometry.Fit(c.Window, c.Work, c.Resize);
                if (actual != c.Expected) throw new Exception($"{c.Name}: expected {c.Expected}, got {actual}");
                Console.WriteLine($"PASS {c.Name}");
            }
            using var watcher = new WindowWatcher(); // Also validates hook registration on this desktop.
            foreach (var screen in Screen.AllScreens)
            {
                using var form = new TestForm { Text = "跨屏收窗自动验证", StartPosition = FormStartPosition.Manual,
                    Bounds = new Rectangle(screen.Bounds.X + 10, screen.Bounds.Y + 10, 1800, 1000) };
                form.Show(); Application.DoEvents();
                var point = new Native.Point { X = screen.Bounds.X + screen.Bounds.Width / 2, Y = screen.Bounds.Y + screen.Bounds.Height / 2 };
                var monitor = Native.MonitorFromPoint(point, 0);
                var info = new Native.MonitorInfo { Size = Marshal.SizeOf<Native.MonitorInfo>() };
                if (!Native.GetMonitorInfo(monitor, ref info)) throw new Exception("GetMonitorInfo failed");
                Native.SetWindowPos(form.Handle, 0, screen.Bounds.X - 300, screen.Bounds.Y + 30,
                    screen.Bounds.Width + 500, screen.Bounds.Height + 300, 0x0014);
                Application.DoEvents();
                watcher.FitWindow(form.Handle, monitor); Application.DoEvents();
                if (Native.DwmGetWindowAttribute(form.Handle, 9, out var frame, Marshal.SizeOf<Native.Rect>()) != 0)
                    throw new Exception("Cannot inspect visible frame");
                if (!info.Work.Rectangle.Contains(frame.Rectangle))
                    throw new Exception($"Native fit on {screen.DeviceName}: {frame.Rectangle} outside {info.Work.Rectangle}");
                var before = frame.Rectangle;
                watcher.FitWindow(form.Handle, monitor); Application.DoEvents();
                Native.DwmGetWindowAttribute(form.Handle, 9, out var after, Marshal.SizeOf<Native.Rect>());
                if (after.Rectangle != before) throw new Exception("Fit is not idempotent");
                Console.WriteLine($"PASS native fit + idempotence on {screen.DeviceName}: {before}");
                form.Close();
            }
            Console.WriteLine($"PASS {cases.Length} geometry cases and native checks on {Screen.AllScreens.Length} monitors");
            return 0;
        }
        catch (Exception ex) { Console.Error.WriteLine(ex); return 1; }
    }

    private sealed class TestForm : Form
    {
        protected override bool ShowWithoutActivation => true;
    }
}
