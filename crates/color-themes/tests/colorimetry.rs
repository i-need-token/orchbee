//! The color science the gates measure with, implemented independently of the crate's resolver and
//! checked once against published reference values.

use orchbee_color_themes::Rgb;

fn linear(color: Rgb) -> [f64; 3] {
    [color.red, color.green, color.blue].map(|channel| {
        let encoded = f64::from(channel) / 255.0;
        if encoded <= 0.040_45 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn encode(linear: [f64; 3]) -> Rgb {
    let [red, green, blue] = linear.map(|channel| {
        let channel = channel.clamp(0.0, 1.0);
        let encoded = if channel <= 0.003_130_8 {
            channel * 12.92
        } else {
            1.055 * channel.powf(1.0 / 2.4) - 0.055
        };
        // Clamped to 0.0..=255.0 before the cast.
        (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
    });
    Rgb::new(red, green, blue)
}

fn luminance(color: Rgb) -> f64 {
    let [red, green, blue] = linear(color);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/// WCAG 2 contrast ratio.
pub(crate) fn contrast(left: Rgb, right: Rgb) -> f64 {
    let (left, right) = (luminance(left), luminance(right));
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

/// APCA 0.0.98G-4g lightness contrast `Lc`: positive for dark text on a light background.
pub(crate) fn apca(text: Rgb, background: Rgb) -> f64 {
    let screen_luminance = |color: Rgb| {
        let [red, green, blue] = [color.red, color.green, color.blue]
            .map(|channel| (f64::from(channel) / 255.0).powf(2.4));
        0.212_672_9 * red + 0.715_152_2 * green + 0.072_175 * blue
    };
    let soft_clamp = |y: f64| {
        if y > 0.022 {
            y
        } else {
            y + (0.022 - y).powf(1.414)
        }
    };
    let text = soft_clamp(screen_luminance(text));
    let background = soft_clamp(screen_luminance(background));
    if (background - text).abs() < 0.0005 {
        return 0.0;
    }
    let output = if background > text {
        let sapc = (background.powf(0.56) - text.powf(0.57)) * 1.14;
        if sapc < 0.1 { 0.0 } else { sapc - 0.027 }
    } else {
        let sapc = (background.powf(0.65) - text.powf(0.62)) * 1.14;
        if sapc > -0.1 { 0.0 } else { sapc + 0.027 }
    };
    output * 100.0
}

/// CIELAB under D65.
fn lab(color: Rgb) -> [f64; 3] {
    let [red, green, blue] = linear(color);
    let x = (0.412_456_4 * red + 0.357_576_1 * green + 0.180_437_5 * blue) / 0.950_47;
    let y = 0.212_672_9 * red + 0.715_152_2 * green + 0.072_175 * blue;
    let z = (0.019_333_9 * red + 0.119_192 * green + 0.950_304_1 * blue) / 1.088_83;
    let epsilon = (6.0_f64 / 29.0).powi(3);
    let f = |t: f64| {
        if t > epsilon {
            t.cbrt()
        } else {
            t / (3.0 * (6.0_f64 / 29.0).powi(2)) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 color difference between two CIELAB colors (Sharma, Wu and Dalal 2005).
fn ciede2000([l1, a1, b1]: [f64; 3], [l2, a2, b2]: [f64; 3]) -> f64 {
    let chroma_mean = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let seventh = |c: f64| c.powi(7) / (c.powi(7) + 25.0_f64.powi(7));
    let g = 0.5 * (1.0 - seventh(chroma_mean).sqrt());
    let (a1, a2) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1, c2) = (a1.hypot(b1), a2.hypot(b2));
    let hue = |a: f64, b: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1, h2) = (hue(a1, b1), hue(a2, b2));
    let delta_l = l2 - l1;
    let delta_c = c2 - c1;
    let delta_h_angle = if c1 * c2 == 0.0 {
        0.0
    } else if (h2 - h1).abs() <= 180.0 {
        h2 - h1
    } else if h2 - h1 > 180.0 {
        h2 - h1 - 360.0
    } else {
        h2 - h1 + 360.0
    };
    let delta_h = 2.0 * (c1 * c2).sqrt() * (delta_h_angle / 2.0).to_radians().sin();
    let l_mean = (l1 + l2) / 2.0;
    let c_mean = (c1 + c2) / 2.0;
    let h_mean = if c1 * c2 == 0.0 {
        h1 + h2
    } else if (h1 - h2).abs() <= 180.0 {
        (h1 + h2) / 2.0
    } else if h1 + h2 < 360.0 {
        (h1 + h2 + 360.0) / 2.0
    } else {
        (h1 + h2 - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (h_mean - 30.0).to_radians().cos()
        + 0.24 * (2.0 * h_mean).to_radians().cos()
        + 0.32 * (3.0 * h_mean + 6.0).to_radians().cos()
        - 0.20 * (4.0 * h_mean - 63.0).to_radians().cos();
    let delta_theta = 30.0 * (-((h_mean - 275.0) / 25.0).powi(2)).exp();
    let rotation = -2.0 * seventh(c_mean).sqrt() * (2.0 * delta_theta).to_radians().sin();
    let s_l = 1.0 + 0.015 * (l_mean - 50.0).powi(2) / (20.0 + (l_mean - 50.0).powi(2)).sqrt();
    let s_c = 1.0 + 0.045 * c_mean;
    let s_h = 1.0 + 0.015 * c_mean * t;
    ((delta_l / s_l).powi(2)
        + (delta_c / s_c).powi(2)
        + (delta_h / s_h).powi(2)
        + rotation * (delta_c / s_c) * (delta_h / s_h))
        .sqrt()
}

/// CIEDE2000 between two sRGB colors.
pub(crate) fn delta_e(left: Rgb, right: Rgb) -> f64 {
    ciede2000(lab(left), lab(right))
}

/// A dichromacy the red-and-green gate simulates.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Deficiency {
    Protanopia,
    Deuteranopia,
}

/// The color as the deficiency renders it: Machado, Oliveira and Fernandes 2009 at severity 1.0,
/// applied to linear-light sRGB.
pub(crate) fn simulate(color: Rgb, deficiency: Deficiency) -> Rgb {
    let matrix = match deficiency {
        Deficiency::Protanopia => [
            [0.152_286, 1.052_583, -0.204_868],
            [0.114_503, 0.786_281, 0.099_216],
            [-0.003_882, -0.048_116, 1.051_998],
        ],
        Deficiency::Deuteranopia => [
            [0.367_322, 0.860_646, -0.227_968],
            [0.280_085, 0.672_501, 0.047_413],
            [-0.011_82, 0.042_94, 0.968_881],
        ],
    };
    let [red, green, blue] = linear(color);
    encode(matrix.map(|row| row[0] * red + row[1] * green + row[2] * blue))
}

/// OKLab perceptual lightness, `0.0` for black through `1.0` for white.
pub(crate) fn oklab_lightness(color: Rgb) -> f64 {
    let [red, green, blue] = linear(color);
    let l = (0.412_221_470_8 * red + 0.536_332_536_3 * green + 0.051_445_992_9 * blue).cbrt();
    let m = (0.211_903_498_2 * red + 0.680_699_545_1 * green + 0.107_396_956_6 * blue).cbrt();
    let s = (0.088_302_461_9 * red + 0.281_718_837_6 * green + 0.629_978_700_5 * blue).cbrt();
    0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} differs from the published {expected}"
    );
}

#[test]
fn color_science_matches_published_reference_values() {
    let gray = |value: u8| Rgb::new(value, value, value);
    // WCAG 2: the full range, and #767676 as the darkest gray that clears 4.5:1 on white.
    assert_close(contrast(gray(0), gray(255)), 21.0, 1e-9, "black on white");
    assert_close(
        contrast(gray(0x76), gray(255)),
        4.54,
        0.005,
        "#767676 on white",
    );
    // APCA 0.0.98G-4g reference pairs, text then background.
    for (text, background, lc) in [
        (0x88, 0xff, 63.056_469_930_209_42),
        (0xff, 0x88, -68.541_464_366_449_62),
        (0x00, 0xaa, 58.146_262_578_561_33),
        (0xaa, 0x00, -56.241_133_368_397_42),
    ] {
        assert_close(apca(gray(text), gray(background)), lc, 1e-6, "APCA Lc");
    }
    // CIEDE2000 test data from Sharma, Wu and Dalal 2005.
    for (left, right, difference) in [
        ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
        ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
        ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        (
            [60.2574, -34.0099, 36.2677],
            [60.4626, -34.1751, 39.4387],
            1.2644,
        ),
        ([50.0, 2.5, 0.0], [50.0, 0.0, -2.5], 4.3065),
        ([2.0776, 0.0795, -1.135], [0.9033, -0.0636, -0.5514], 0.9082),
    ] {
        assert_close(ciede2000(left, right), difference, 5e-5, "CIEDE2000");
    }
    // Machado 2009 matrix rows sum to one, so neutral grays survive every simulation.
    for deficiency in [Deficiency::Protanopia, Deficiency::Deuteranopia] {
        for value in [0, 0x40, 0x80, 0xc0, 0xff] {
            assert_eq!(simulate(gray(value), deficiency), gray(value));
        }
    }
    // CSS Color 4: sRGB red is oklab(0.62796 0.22486 0.12585); white has lightness 1.
    assert_close(
        oklab_lightness(Rgb::new(255, 0, 0)),
        0.627_96,
        5e-5,
        "OKLab red",
    );
    assert_close(oklab_lightness(gray(255)), 1.0, 1e-6, "OKLab white");
}
