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
        let margin = (12.0 * self.scale).round();
        let size = (SIZE * self.scale).ceil();
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
            assert!(f64::from(clamped.x) + SIZE * scale <= -12.0 * scale);
            assert!(f64::from(clamped.y) + SIZE * scale <= 1040.0 - 12.0 * scale);
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
}
