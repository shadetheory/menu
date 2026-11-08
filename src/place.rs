//! Place a panel slightly below center on one output's full logical rectangle.
//!
//! Coordinates are logical pixels. The caller converts physical pixels with
//! [`logical_len`] and [`logical_px`] using that output's scale. The panel is
//! clamped so it stays inside the output when it grows.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputBox {
    pub origin_x: i32,
    pub origin_y: i32,
    pub logical_w: i32,
    pub logical_h: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn logical_px(physical: i32, scale: f64) -> i32 {
    let scale = if scale.is_finite() && scale >= 1.0 {
        scale
    } else {
        1.0
    };
    (physical as f64 / scale).round() as i32
}

pub fn logical_len(physical: i32, scale: f64) -> i32 {
    logical_px(physical, scale).max(1)
}

/// Panel center at 55% of output height, clamped within the focused output.
pub fn place_panel(output: OutputBox, panel_w: u32, panel_h: u32) -> Placement {
    let ow = output.logical_w.max(1);
    let oh = output.logical_h.max(1);
    let width = (panel_w as i32).clamp(1, ow) as u32;
    let height = (panel_h as i32).clamp(1, oh) as u32;
    let x = output.origin_x + (ow - width as i32) / 2;
    let y = output.origin_y
        + ((oh as i64 * 55 / 100) as i32 - height as i32 / 2).clamp(0, oh - height as i32);
    Placement {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dp7() -> OutputBox {
        OutputBox {
            origin_x: 0,
            origin_y: 0,
            logical_w: 2560,
            logical_h: 1440,
        }
    }

    #[test]
    fn focused_full_output_centers_the_idle_pill() {
        let placed = place_panel(dp7(), 680, 74);
        assert_eq!(placed.width, 680);
        assert_eq!(placed.height, 74);
        assert_eq!((placed.x, placed.y), (940, 755));
    }

    #[test]
    fn expansion_recomputes_y_and_stays_inside() {
        let placed = place_panel(dp7(), 680, 431);
        assert_eq!(placed.y, 1440 * 55 / 100 - 431 / 2);
        assert_eq!(placed.x, 940);
        assert!(placed.y >= 0);
        assert!(placed.y + placed.height as i32 <= 1440);
    }

    #[test]
    fn scale_uses_logical_pixels_and_other_output_uses_its_origin() {
        assert_eq!(logical_len(5120, 2.0), 2560);
        assert_eq!(logical_len(2880, 2.0), 1440);
        let logical = OutputBox {
            origin_x: logical_px(0, 2.0),
            origin_y: logical_px(0, 2.0),
            logical_w: logical_len(5120, 2.0),
            logical_h: logical_len(2880, 2.0),
        };
        assert_eq!(place_panel(logical, 680, 74).x, 940);
        let dp8 = OutputBox {
            origin_x: 2560,
            origin_y: 0,
            logical_w: 3840,
            logical_h: 2160,
        };
        let placed = place_panel(dp8, 680, 74);
        assert_eq!(placed.x, 2560 + (3840 - 680) / 2);
        assert_eq!(placed.y, 2160 * 55 / 100 - 74 / 2);
        let huge = place_panel(dp7(), 4000, 2000);
        assert_eq!(
            (huge.x, huge.y, huge.width, huge.height),
            (0, 0, 2560, 1440)
        );
    }
}
