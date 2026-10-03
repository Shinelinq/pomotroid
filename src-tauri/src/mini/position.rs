use serde::{Deserialize, Serialize};

pub const SIZE: f64 = 112.0;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl WorkArea {
    pub fn contains_point(&self, point: Point) -> bool {
        f64::from(point.x) >= f64::from(self.x)
            && f64::from(point.y) >= f64::from(self.y)
            && f64::from(point.x) < f64::from(self.x) + f64::from(self.width)
            && f64::from(point.y) < f64::from(self.y) + f64::from(self.height)
    }

    pub fn clamp(&self, point: Point) -> Point {
        let margin = (8.0 * self.scale).round();
        let size = (SIZE * self.scale).ceil();
        if f64::from(point.x) >= f64::from(self.x)
            && f64::from(point.y) >= f64::from(self.y)
            && f64::from(point.x) + size <= f64::from(self.x) + f64::from(self.width)
            && f64::from(point.y) + size <= f64::from(self.y) + f64::from(self.height)
        {
            return point;
        }
        let left = f64::from(self.x) + margin;
        let top = f64::from(self.y) + margin;
        let right = (f64::from(self.x) + f64::from(self.width) - size - margin).max(left);
        let bottom = (f64::from(self.y) + f64::from(self.height) - size - margin).max(top);
        Point {
            x: f64::from(point.x).clamp(left, right).round() as i32,
            y: f64::from(point.y).clamp(top, bottom).round() as i32,
        }
    }
}

/// Physical outer rectangle; thresholds and inset are logical pixels at the
/// monitor selected by the drop's center. No position changes happen mid-drag.
pub fn settle(area: WorkArea, point: Point, width: u32, height: u32, snap: bool) -> Point {
    let left = f64::from(area.x);
    let top = f64::from(area.y);
    let right = left + f64::from(area.width) - f64::from(width);
    let bottom = top + f64::from(area.height) - f64::from(height);
    let threshold = 12.0 * area.scale;
    let inset = 8.0 * area.scale;
    fn axis(value: f64, low: f64, high: f64, threshold: f64, inset: f64, snap: bool) -> i32 {
        let mut value = value;
        if snap {
            let near_low = (value - low).abs() <= threshold;
            let near_high = (high - value).abs() <= threshold;
            if near_low && (!near_high || (value - low).abs() <= (high - value).abs()) {
                value = low + inset;
            } else if near_high {
                value = high - inset;
            }
        }
        // Visibility correction also applies with snapping disabled or position locked.
        value.clamp(low, high.max(low)).round() as i32
    }
    Point {
        x: axis(f64::from(point.x), left, right, threshold, inset, snap),
        y: axis(f64::from(point.y), top, bottom, threshold, inset, snap),
    }
}

pub fn place(
    areas: &[WorkArea],
    saved: Option<Point>,
    preferred: WorkArea,
    initial: Point,
) -> Point {
    if let Some(point) = saved {
        if let Some(area) = areas.iter().find(|area| area.contains_point(point)) {
            return area.clamp(point);
        }
    }
    preferred.clamp(initial)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_visible_position_and_keeps_entire_window_in_work_area() {
        for scale in [1.0, 1.25, 1.5] {
            let area = WorkArea {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1040,
                scale,
            };
            let valid = Point { x: -600, y: 300 };
            assert_eq!(place(&[area], Some(valid), area, valid), valid);
            let clamped = area.clamp(Point { x: -1, y: 1039 });
            assert!(f64::from(clamped.x) + SIZE * scale <= -8.0 * scale);
            assert!(f64::from(clamped.y) + SIZE * scale <= 1040.0 - 8.0 * scale);
        }
    }

    #[test]
    fn disconnected_monitor_falls_back_to_main_monitor() {
        let area = WorkArea {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
            scale: 1.25,
        };
        let initial = Point { x: 1000, y: 30 };
        assert_eq!(
            place(&[area], Some(Point { x: -1500, y: 500 }), area, initial),
            initial
        );
    }

    #[test]
    fn drop_uses_actual_edges_logical_thresholds_negative_monitors_and_corners() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let area = WorkArea {
                x: -1920,
                y: -200,
                width: 1920,
                height: 1080,
                scale,
            };
            let size = (112.0 * scale).round() as u32;
            let p = Point {
                x: area.x + (10.0 * scale).round() as i32,
                y: area.y + (11.0 * scale).round() as i32,
            };
            let result = settle(area, p, size, size, true);
            assert_eq!(
                result,
                Point {
                    x: area.x + (8.0 * scale).round() as i32,
                    y: area.y + (8.0 * scale).round() as i32
                }
            );
            assert_eq!(settle(area, p, size, size, false), p);
            let far = Point {
                x: area.x + (14.0 * scale).ceil() as i32,
                y: 100,
            };
            assert_eq!(settle(area, far, size, size, true), far);
            let right = Point {
                x: -(size as i32) - (9.0 * scale).round() as i32,
                y: 880 - size as i32 - (9.0 * scale).round() as i32,
            };
            let end = settle(area, right, size, size, true);
            assert_eq!(end.x, -(size as i32) - (8.0 * scale).round() as i32);
            assert_eq!(end.y, 880 - size as i32 - (8.0 * scale).round() as i32);
            assert_eq!(area.clamp(result), result);
        }
    }
}
