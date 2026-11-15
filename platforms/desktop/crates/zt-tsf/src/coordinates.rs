//! The caret rectangle the broker gets: physical pixels turned into logical pixels around the origin of the monitor
//! the caret is on, `origin + (physical - origin) / scale`, so a mixed-DPI desktop keeps every monitor where it is.
//! The request then says `scale: 1`.

/// `[x, y, width, height]` in logical pixels for the physical rectangle `left, top, right, bottom`.
pub(crate) fn logical_rect(physical: [i32; 4], origin: (i32, i32), scale: f64) -> [f64; 4] {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let [left, top, right, bottom] = physical.map(f64::from);
    let (x, y) = (f64::from(origin.0), f64::from(origin.1));
    [x + (left - x) / scale, y + (top - y) / scale, (right - left) / scale, (bottom - top) / scale]
}

#[cfg(test)]
mod tests {
    use super::logical_rect;

    #[test]
    fn primary_monitor() {
        assert_eq!(logical_rect([300, 200, 302, 230], (0, 0), 1.5), [200.0, 200.0 / 1.5, 2.0 / 1.5, 20.0]);
    }

    #[test]
    fn monitor_left_of_the_primary_keeps_its_origin() {
        assert_eq!(logical_rect([-1900, 100, -1898, 120], (-1920, 0), 2.0), [-1910.0, 50.0, 1.0, 10.0]);
    }

    #[test]
    fn bad_scale_is_one() {
        assert_eq!(logical_rect([1, 2, 3, 4], (0, 0), f64::NAN), [1.0, 2.0, 2.0, 2.0]);
        assert_eq!(logical_rect([1, 2, 3, 4], (0, 0), 0.0), [1.0, 2.0, 2.0, 2.0]);
    }
}
