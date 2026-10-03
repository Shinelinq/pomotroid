use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Geometry {
    /// Window origin in physical desktop coordinates (negative coordinates are valid).
    pub x: i32,
    pub y: i32,
    /// Client size in logical pixels; restored using the destination monitor's scale.
    pub width: f64,
    pub height: f64,
    pub monitor: Option<String>,
    pub monitor_x: i32,
    pub monitor_y: i32,
}

#[derive(Clone, Debug)]
pub struct Area {
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

#[derive(Clone, Copy)]
pub struct Spec {
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

pub fn same_monitor(saved: &Geometry, area: &Area) -> bool {
    match (&saved.monitor, &area.name) {
        (Some(a), Some(b)) => a == b,
        _ => saved.monitor_x == area.x && saved.monitor_y == area.y,
    }
}

pub fn fits(saved: &Geometry, area: &Area, frame: (f64, f64)) -> bool {
    let width = ((saved.width + frame.0) * area.scale).round();
    let height = ((saved.height + frame.1) * area.scale).round();
    saved.width.is_finite()
        && saved.height.is_finite()
        && saved.width > 0.0
        && saved.height > 0.0
        && f64::from(saved.x) >= f64::from(area.x)
        && f64::from(saved.y) >= f64::from(area.y)
        && f64::from(saved.x) + width <= f64::from(area.x) + f64::from(area.width)
        && f64::from(saved.y) + height <= f64::from(area.y) + f64::from(area.height)
}

pub fn restore(
    spec: Spec,
    saved: Option<&Geometry>,
    areas: &[Area],
    preferred: &Area,
    frame: (f64, f64),
) -> Placement {
    let previous_monitor = saved.and_then(|g| areas.iter().find(|area| same_monitor(g, area)));
    let area = previous_monitor.unwrap_or(preferred);
    let available_width = (f64::from(area.width) / area.scale - frame.0).max(1.0);
    let available_height = (f64::from(area.height) / area.scale - frame.1).max(1.0);
    let min_width = spec.min_width.min(available_width);
    let min_height = spec.min_height.min(available_height);
    let valid_size = saved.filter(|g| {
        g.width.is_finite()
            && g.height.is_finite()
            && g.width >= min_width
            && g.height >= min_height
    });
    let (width, height) = valid_size
        .map(|g| (g.width, g.height))
        .unwrap_or((spec.width, spec.height));
    let width = width.clamp(min_width, available_width);
    let height = height.clamp(min_height, available_height);
    let preserved = valid_size.filter(|g| {
        previous_monitor.is_some() && fits(g, area, frame) && g.width == width && g.height == height
    });
    let (x, y) = preserved.map(|g| (g.x, g.y)).unwrap_or_else(|| {
        let outer_width = ((width + frame.0) * area.scale).round();
        let outer_height = ((height + frame.1) * area.scale).round();
        (
            (f64::from(area.x) + (f64::from(area.width) - outer_width) / 2.0).floor() as i32,
            (f64::from(area.y) + (f64::from(area.height) - outer_height) / 2.0).floor() as i32,
        )
    });
    Placement {
        x,
        y,
        width,
        height,
        min_width,
        min_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const STATS: Spec = Spec {
        width: 900.0,
        height: 600.0,
        min_width: 720.0,
        min_height: 480.0,
    };
    fn area(scale: f64) -> Area {
        Area {
            name: Some("left".into()),
            x: -1920,
            y: -100,
            width: 1920,
            height: 1040,
            scale,
        }
    }
    fn saved() -> Geometry {
        Geometry {
            x: -1800,
            y: 0,
            width: 900.0,
            height: 600.0,
            monitor: Some("left".into()),
            monitor_x: -1920,
            monitor_y: -100,
        }
    }

    #[test]
    fn first_open_centers_in_work_area_not_full_screen() {
        let a = area(1.0);
        let p = restore(STATS, None, &[a.clone()], &a, (0.0, 0.0));
        assert_eq!((p.x, p.y, p.width, p.height), (-1410, 120, 900.0, 600.0));
    }

    #[test]
    fn restores_negative_origin_and_logical_size_at_each_dpi() {
        for scale in [1.0, 1.25, 1.5] {
            let a = area(scale);
            let g = saved();
            let p = restore(STATS, Some(&g), &[a.clone()], &a, (0.0, 0.0));
            assert_eq!((p.x, p.y, p.width, p.height), (-1800, 0, 900.0, 600.0));
        }
    }

    #[test]
    fn rejects_sliver_overlap_and_disconnected_monitor() {
        let a = area(1.0);
        let mut g = saved();
        g.x = -1;
        let p = restore(STATS, Some(&g), &[a.clone()], &a, (0.0, 0.0));
        assert_eq!((p.x, p.y), (-1410, 120));
        g.monitor = Some("disconnected".into());
        g.x = -1800;
        assert_eq!(
            restore(STATS, Some(&g), &[a.clone()], &a, (0.0, 0.0)).x,
            -1410
        );
    }

    #[test]
    fn small_work_area_overrides_regular_minimum_including_frame() {
        let a = Area {
            width: 800,
            height: 600,
            scale: 1.5,
            ..area(1.0)
        };
        let p = restore(STATS, None, &[a.clone()], &a, (0.0, 20.0));
        assert!(p.width < 720.0 && p.height < 480.0);
        assert_eq!(p.min_width, p.width);
        assert_eq!(p.min_height, p.height);
        assert!((p.height + 20.0) * a.scale <= 600.0);
    }

    #[test]
    fn invalid_sizes_use_defaults_without_rejecting_valid_negative_positions() {
        let a = area(1.0);
        for width in [0.0, -1.0, f64::NAN] {
            let mut g = saved();
            g.width = width;
            let p = restore(STATS, Some(&g), &[a.clone()], &a, (0.0, 0.0));
            assert_eq!((p.width, p.height), (900.0, 600.0));
        }
    }
}
