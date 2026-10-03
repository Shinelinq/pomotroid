use super::TrayColors;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Compact built-in vector numerals. No font assets or font rasterizer needed.
pub fn render(
    colors: &TrayColors,
    label: &str,
    paused: bool,
    round: &str,
    size: u32,
) -> Result<Vec<u8>, String> {
    let mut pixmap = Pixmap::new(size, size).ok_or("tray bitmap allocation failed")?;
    let color = match round {
        "short-break" => colors.short_round,
        "long-break" => colors.long_round,
        _ => colors.focus_round,
    };
    let mut paint = Paint {
        anti_alias: true,
        ..Default::default()
    };
    paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
    let compact = label == "99+";
    let width = if compact {
        17.0
    } else if label.len() == 1 {
        28.0
    } else {
        22.0
    };
    let gap = if compact { 4.0 } else { 6.0 };
    let stroke = if compact { 5.0 } else { 6.0 };
    let height = if paused {
        38.0
    } else if compact {
        42.0
    } else {
        48.0
    };
    let y = if paused { 6.0 } else { (64.0 - height) / 2.0 };
    let total = if compact {
        width * 2.0 + gap * 2.0 + 10.0
    } else {
        width * label.len() as f32 + gap * (label.len() - 1) as f32
    };
    let mut x = (64.0 - total) / 2.0;
    let transform = Transform::from_scale(size as f32 / 64.0, size as f32 / 64.0);
    let style = Stroke {
        width: stroke,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    for digit in label.chars() {
        let mut path = PathBuilder::new();
        if digit == '+' {
            path.move_to(x, y + height / 2.0);
            path.line_to(x + 10.0, y + height / 2.0);
            path.move_to(x + 5.0, y + height / 2.0 - 6.0);
            path.line_to(x + 5.0, y + height / 2.0 + 6.0);
        } else {
            let mask = [
                0b1111110, 0b0110000, 0b1101101, 0b1111001, 0b0110011, 0b1011011, 0b1011111,
                0b1110000, 0b1111111, 0b1111011,
            ][digit.to_digit(10).ok_or("invalid tray digit")? as usize];
            let segments = [
                ((0.0, 0.0), (width, 0.0)),
                ((width, 0.0), (width, height / 2.0)),
                ((width, height / 2.0), (width, height)),
                ((0.0, height), (width, height)),
                ((0.0, height / 2.0), (0.0, height)),
                ((0.0, 0.0), (0.0, height / 2.0)),
                ((0.0, height / 2.0), (width, height / 2.0)),
            ];
            // A single 1 is centered rather than sitting at the right of an empty glyph cell.
            let origin = if digit == '1' && label.len() == 1 {
                x - width / 2.0
            } else {
                x
            };
            for (index, ((ax, ay), (bx, by))) in segments.iter().enumerate() {
                if mask & (1 << (6 - index)) != 0 {
                    path.move_to(origin + ax, y + ay);
                    path.line_to(origin + bx, y + by);
                }
            }
        }
        let path = path.finish().ok_or("tray digit path failed")?;
        pixmap.stroke_path(&path, &paint, &style, transform, None);
        x += width + gap;
    }
    if paused {
        for x in [25.0, 35.0] {
            let rect =
                tiny_skia::Rect::from_xywh(x, 53.0, 4.0, 9.0).ok_or("tray pause marker failed")?;
            pixmap.fill_path(
                &PathBuilder::from_rect(rect),
                &paint,
                tiny_skia::FillRule::Winding,
                transform,
                None,
            );
        }
    }
    Ok(pixmap.take())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_layout_renders_at_windows_dpi_sizes_with_separate_pause_marker() {
        for size in [16, 20, 24, 32, 64] {
            for label in ["0", "1", "8", "12", "90", "99", "99+"] {
                let running = render(&TrayColors::default(), label, false, "work", size).unwrap();
                let paused = render(&TrayColors::default(), label, true, "work", size).unwrap();
                assert_eq!(running.len(), (size * size * 4) as usize);
                assert_ne!(running, paused);
                assert!(paused.chunks_exact(4).any(|p| p[3] > 0));
            }
        }
        let paused = render(&TrayColors::default(), "99+", true, "work", 64).unwrap();
        // Four transparent rows separate the compact digits from the pause bars.
        for y in 48..52 {
            assert!(paused[y * 64 * 4..(y + 1) * 64 * 4]
                .chunks_exact(4)
                .all(|p| p[3] == 0));
        }
    }
}
