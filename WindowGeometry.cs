namespace AutoWindowAdjust;

public static class WindowGeometry
{
    // Physical pixels throughout. Work areas can have negative desktop coordinates.
    public static Rectangle Fit(Rectangle window, Rectangle workArea, bool resizable)
    {
        if (window.Width <= 0 || window.Height <= 0 || workArea.Width <= 0 || workArea.Height <= 0)
            return window;
        int width = window.Width, height = window.Height;
        if (resizable)
        {
            var scale = Math.Min(1d, Math.Min((double)workArea.Width / width, (double)workArea.Height / height));
            width = Math.Max(1, (int)Math.Floor(width * scale));
            height = Math.Max(1, (int)Math.Floor(height * scale));
        }
        // A fixed/minimum-size window may exceed the screen; keep its top-left accessible.
        int x = Math.Clamp(window.X, workArea.Left, Math.Max(workArea.Left, workArea.Right - width));
        int y = Math.Clamp(window.Y, workArea.Top, Math.Max(workArea.Top, workArea.Bottom - height));
        return new Rectangle(x, y, width, height);
    }
}
