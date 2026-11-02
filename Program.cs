namespace AutoWindowAdjust;

internal static class Program
{
    [STAThread]
    private static void Main()
    {
        using var mutex = new Mutex(true, @"Local\AutoWindowAdjust.Tray", out var first);
        if (!first)
        {
            MessageBox.Show("工具已经在运行，请在任务栏右下角的托盘中查找图标。", "跨屏收窗");
            return;
        }
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        try { Application.Run(new TrayContext()); }
        catch (Exception ex)
        {
            MessageBox.Show($"无法启动跨屏收窗：{ex.Message}", "跨屏收窗", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
    }
}
