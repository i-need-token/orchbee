//! WCAG 2 contrast, and the lightness-only correction that raises a color to a contrast floor on a
//! surface.

use crate::{Rgb, oklab};

/// The WCAG 2 contrast ratio between two colors, from 1.0 to 21.0.
pub(crate) fn ratio(left: Rgb, right: Rgb) -> f64 {
    let (left, right) = (luminance(left), luminance(right));
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

fn luminance(color: Rgb) -> f64 {
    let [red, green, blue] = color.linear();
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/// `color` unchanged when it already reaches `floor` on `surface`; otherwise the color with the
/// nearest OKLab lightness, moved away from the surface with hue kept, that reaches it. When no
/// lightness in that direction reaches the floor, the result is that direction's extreme.
pub(crate) fn raise(color: Rgb, surface: Rgb, floor: f64) -> Rgb {
    if ratio(color, surface) >= floor {
        return color;
    }
    let mut failing = oklab::to_oklab(color).l;
    let mut passing = if luminance(color) >= luminance(surface) {
        1.0
    } else {
        0.0
    };
    let mut best = oklab::with_lightness(color, passing);
    // Bisect toward the passing extreme; each probe is the rounded color that would ship.
    for _ in 0..40 {
        let middle = f64::midpoint(failing, passing);
        let candidate = oklab::with_lightness(color, middle);
        if ratio(candidate, surface) >= floor {
            best = candidate;
            passing = middle;
        } else {
            failing = middle;
        }
    }
    best
}
