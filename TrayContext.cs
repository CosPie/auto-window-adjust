using Microsoft.Win32;

namespace AutoWindowAdjust;

internal sealed class TrayContext : ApplicationContext
{
    private const string RunKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string StartupName = "AutoWindowAdjust";
    private readonly NotifyIcon tray;
    private readonly WindowWatcher watcher;
    private readonly ContextMenuStrip menu = new();
    private readonly ToolStripMenuItem enabled = new("自动收窗：已开启") { Checked = true };
    private readonly ToolStripMenuItem startup = new("开机启动");
    private readonly ToolStripMenuItem status = new("等待跨屏拖动") { Enabled = false };

    internal TrayContext()
    {
        watcher = new WindowWatcher();
        watcher.Report = message => status.Text = message;
        enabled.Click += (_, _) => Toggle();
        startup.Checked = IsStartupEnabled();
        startup.Click += (_, _) => SetStartup();
        menu.Items.AddRange([enabled, startup, new ToolStripSeparator(), status,
            new ToolStripMenuItem("使用说明", null, (_, _) => ShowHelp()),
            new ToolStripSeparator(), new ToolStripMenuItem("退出", null, (_, _) => ExitThread())]);
        tray = new NotifyIcon { Text = "跨屏收窗 · 已开启（双击暂停）", Icon = SystemIcons.Application,
            ContextMenuStrip = menu, Visible = true };
        tray.DoubleClick += (_, _) => Toggle();
        tray.ShowBalloonTip(5000, "跨屏收窗已启动", "拖动窗口到另一块屏幕，松手后自动收好。按住 Alt 可跳过本次调整。右键托盘图标可暂停或退出。", ToolTipIcon.Info);
    }

    private void Toggle()
    {
        watcher.Enabled = !watcher.Enabled;
        enabled.Checked = watcher.Enabled;
        enabled.Text = watcher.Enabled ? "自动收窗：已开启" : "自动收窗：已暂停";
        tray.Text = watcher.Enabled ? "跨屏收窗 · 已开启（双击暂停）" : "跨屏收窗 · 已暂停（双击恢复）";
    }
    private static bool IsStartupEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKey);
        return string.Equals(key?.GetValue(StartupName) as string, $"\"{Application.ExecutablePath}\"", StringComparison.OrdinalIgnoreCase);
    }
    private void SetStartup()
    {
        try
        {
            using var key = Registry.CurrentUser.CreateSubKey(RunKey);
            if (startup.Checked) key.DeleteValue(StartupName, false);
            else key.SetValue(StartupName, $"\"{Application.ExecutablePath}\"");
            startup.Checked = IsStartupEnabled();
        }
        catch (Exception ex) { MessageBox.Show($"无法更改开机启动：{ex.Message}", "跨屏收窗"); }
    }
    private static void ShowHelp() => MessageBox.Show(
        "把普通窗口拖到另一块屏幕，松开鼠标后自动收进该屏幕。\n\n" +
        "• 放得下：保留大小，只调整位置。\n• 放不下：按比例缩小，避开任务栏。\n" +
        "• 按住 Alt 松手：跳过本次调整。\n• 同屏拖动、边缘缩放、最大化和最小化窗口不处理。\n\n" +
        "右键托盘图标可暂停、开机启动或退出；双击可切换暂停。\n" +
        "部分应用有最小尺寸限制，无法完整收进更小的屏幕。普通权限无法调整管理员应用。\n" +
        "移动程序位置后，请重新勾选开机启动以更新路径。", "跨屏收窗 · 使用说明");

    protected override void ExitThreadCore()
    {
        watcher.Dispose(); tray.Visible = false; tray.Dispose(); menu.Dispose();
        base.ExitThreadCore();
    }
}
