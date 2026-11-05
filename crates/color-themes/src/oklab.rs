//! OKLab for 8-bit sRGB: conversion, mixing, and lightness changes that keep hue. Results round to
//! the nearest 8-bit color; a mix or offset clips out-of-gamut channels, while a lightness change
//! reduces chroma until the color fits the sRGB gamut.

use crate::Rgb;

/// A color in OKLab: perceptual lightness `l` in `0.0..=1.0` and the opponent axes `a` and `b`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Oklab {
    pub(crate) l: f64,
    pub(crate) a: f64,
    pub(crate) b: f64,
}

/// The OKLab coordinates of an 8-bit sRGB color; neutral grays have exactly zero `a` and `b`.
pub(crate) fn to_oklab(color: Rgb) -> Oklab {
    let [red, green, blue] = color.linear();
    let l = (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
    let m = (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
    let s = (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();
    let lightness = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
    // The published matrices leave a residue of about 1e-8 on the opponent axes of a neutral
    // gray, enough to tint mixes of grays by one channel step after rounding.
    if color.red == color.green && color.green == color.blue {
        return Oklab {
            l: lightness,
            a: 0.0,
            b: 0.0,
        };
    }
    Oklab {
        l: lightness,
        a: 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    }
}

/// The linear-light sRGB channels of `color`, possibly outside `0.0..=1.0`.
fn to_linear(color: Oklab) -> [f64; 3] {
    let l = (color.l + 0.396_337_777_4 * color.a + 0.215_803_757_3 * color.b).powi(3);
    let m = (color.l - 0.105_561_345_8 * color.a - 0.063_854_172_8 * color.b).powi(3);
    let s = (color.l - 0.089_484_177_5 * color.a - 1.291_485_548_0 * color.b).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
    ]
}

/// The nearest 8-bit color, clipping each out-of-gamut channel.
fn clipped(color: Oklab) -> Rgb {
    Rgb::from_linear(to_linear(color))
}

/// `from` moved `amount` (`0.0..=1.0`) of the way toward `toward` in OKLab.
pub(crate) fn mix(from: Rgb, toward: Rgb, amount: f64) -> Rgb {
    let (from, toward) = (to_oklab(from), to_oklab(toward));
    clipped(Oklab {
        l: from.l + (toward.l - from.l) * amount,
        a: from.a + (toward.a - from.a) * amount,
        b: from.b + (toward.b - from.b) * amount,
    })
}

/// `color` with its OKLab lightness moved by `offset`, keeping both opponent axes.
pub(crate) fn offset_lightness(color: Rgb, offset: f64) -> Rgb {
    let color = to_oklab(color);
    clipped(Oklab {
        l: (color.l + offset).clamp(0.0, 1.0),
        ..color
    })
}

/// `color` at OKLab lightness `l`, keeping its hue and reducing its chroma only as far as the
/// sRGB gamut requires.
pub(crate) fn with_lightness(color: Rgb, l: f64) -> Rgb {
    let color = to_oklab(color);
    let at = |scale: f64| Oklab {
        l,
        a: color.a * scale,
        b: color.b * scale,
    };
    let in_gamut = |candidate: Oklab| {
        to_linear(candidate)
            .iter()
            .all(|channel| (-1e-9..=1.0 + 1e-9).contains(channel))
    };
    if in_gamut(at(1.0)) {
        return clipped(at(1.0));
    }
    // Bisect the largest in-gamut chroma scale; zero chroma is always in gamut.
    let (mut inside, mut outside) = (0.0, 1.0);
    for _ in 0..40 {
        let middle = f64::midpoint(inside, outside);
        if in_gamut(at(middle)) {
            inside = middle;
        } else {
            outside = middle;
        }
    }
    clipped(at(inside))
}
