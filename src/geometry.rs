#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Fit visible bounds into the monitor work area, using physical pixels.
/// Fixed or minimum-size windows keep an accessible top-left corner.
pub fn fit(window: Rect, work: Rect, resizable: bool) -> Rect {
    if window.width <= 0 || window.height <= 0 || work.width <= 0 || work.height <= 0 {
        return window;
    }
    let (mut width, mut height) = (window.width, window.height);
    if resizable {
        let scale = 1.0_f64
            .min(f64::from(work.width) / f64::from(width))
            .min(f64::from(work.height) / f64::from(height));
        width = ((f64::from(width) * scale).floor() as i32).max(1);
        height = ((f64::from(height) * scale).floor() as i32).max(1);
    }
    let x = i64::from(window.x).clamp(
        i64::from(work.x),
        (i64::from(work.x) + i64::from(work.width) - i64::from(width)).max(i64::from(work.x)),
    );
    let y = i64::from(window.y).clamp(
        i64::from(work.y),
        (i64::from(work.y) + i64::from(work.height) - i64::from(height)).max(i64::from(work.y)),
    );
    Rect::new(x as i32, y as i32, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_cases() {
        let cases = [
            (
                "already fits",
                Rect::new(100, 100, 800, 600),
                Rect::new(0, 0, 1920, 1040),
                true,
                Rect::new(100, 100, 800, 600),
            ),
            (
                "left edge",
                Rect::new(-200, 100, 800, 600),
                Rect::new(0, 0, 1920, 1040),
                true,
                Rect::new(0, 100, 800, 600),
            ),
            (
                "right edge",
                Rect::new(1700, 100, 800, 600),
                Rect::new(0, 0, 1920, 1040),
                true,
                Rect::new(1120, 100, 800, 600),
            ),
            (
                "taskbar",
                Rect::new(100, 800, 800, 600),
                Rect::new(0, 0, 1920, 1040),
                true,
                Rect::new(100, 440, 800, 600),
            ),
            (
                "landscape to portrait",
                Rect::new(-800, 100, 1600, 900),
                Rect::new(-1080, 0, 1080, 1880),
                true,
                Rect::new(-1080, 100, 1080, 607),
            ),
            (
                "portrait to landscape",
                Rect::new(100, 0, 900, 1800),
                Rect::new(0, 0, 1920, 1040),
                true,
                Rect::new(100, 0, 520, 1040),
            ),
            (
                "monitor above",
                Rect::new(100, -50, 800, 600),
                Rect::new(0, -1080, 1920, 1040),
                true,
                Rect::new(100, -640, 800, 600),
            ),
            (
                "left taskbar",
                Rect::new(0, 10, 800, 600),
                Rect::new(60, 0, 1860, 1080),
                true,
                Rect::new(60, 10, 800, 600),
            ),
            (
                "fixed oversized",
                Rect::new(-500, 50, 2000, 1200),
                Rect::new(0, 0, 1920, 1040),
                false,
                Rect::new(0, 0, 2000, 1200),
            ),
            (
                "never enlarge",
                Rect::new(10, 10, 100, 100),
                Rect::new(0, 0, 3840, 2160),
                true,
                Rect::new(10, 10, 100, 100),
            ),
            (
                "invalid area",
                Rect::new(10, 10, 100, 100),
                Rect::new(0, 0, 0, 0),
                true,
                Rect::new(10, 10, 100, 100),
            ),
        ];
        for (name, window, work, resizable, expected) in cases {
            assert_eq!(fit(window, work, resizable), expected, "{name}");
            assert_eq!(
                fit(expected, work, resizable),
                expected,
                "idempotence: {name}"
            );
        }
    }
}
