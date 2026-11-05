//! Every shipped theme meets the gates in the crate README, and its resolved roles meet the
//! resolver's contrast floors. A failure within 5% of its threshold is labeled borderline; it
//! still fails.

use std::collections::BTreeSet;

use orchbee_color_themes::{PaletteColor, Polarity, Rgb, Roles, Theme, orchbee, shipped};

use crate::colorimetry::{Deficiency, apca, contrast, delta_e, oklab_lightness, simulate};

const PERMISSIVE_LICENSES: [&str; 6] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "public-domain",
];

#[test]
fn every_shipped_theme_meets_the_gates() {
    let reference = Roles::resolve(orchbee(Polarity::Light));
    let failures: Vec<String> = shipped()
        .iter()
        .flat_map(|theme| gate(theme, &reference))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn shipped_ids_are_unique_and_the_default_pair_has_both_polarities() {
    let mut ids = BTreeSet::new();
    for theme in shipped() {
        assert!(ids.insert(theme.id()), "{} is shipped twice", theme.id());
    }
    for polarity in [Polarity::Light, Polarity::Dark] {
        let theme = orchbee(polarity);
        assert_eq!(theme.polarity(), polarity);
        assert_eq!(theme.family(), "orchbee");
        assert!(ids.contains(theme.id()));
    }
}

/// Collects one theme's gate failures, each naming the theme and the measured value.
struct Findings<'a> {
    theme: &'a str,
    failures: Vec<String>,
}

impl Findings<'_> {
    fn fail(&mut self, message: String) {
        self.failures.push(format!("{}: {message}", self.theme));
    }

    fn at_least(&mut self, what: &str, value: f64, floor: f64) {
        if value < floor {
            let borderline = if value >= floor * 0.95 {
                ", borderline"
            } else {
                ""
            };
            self.fail(format!("{what} is {value:.3}, below {floor}{borderline}"));
        }
    }

    fn at_most(&mut self, what: &str, value: f64, ceiling: f64) {
        if value > ceiling {
            let borderline = if value <= ceiling * 1.05 {
                ", borderline"
            } else {
                ""
            };
            self.fail(format!("{what} is {value:.3}, above {ceiling}{borderline}"));
        }
    }
}

fn gate(theme: &Theme, reference: &Roles) -> Vec<String> {
    let mut findings = Findings {
        theme: theme.id(),
        failures: Vec::new(),
    };
    let canvas = theme.canvas();
    let roles = Roles::resolve(theme);
    let background = canvas.background;
    let foreground = canvas.foreground;
    let ansi = theme.ansi().map(PaletteColor::value);
    let dark = theme.polarity() == Polarity::Dark;

    let license = theme.source().license.as_str();
    if !PERMISSIVE_LICENSES.contains(&license) {
        findings.fail(format!("license {license} is not admitted"));
    }
    let lightness = oklab_lightness(background);
    if (lightness < 0.6) != dark {
        findings.fail(format!(
            "declares {:?} but its canvas has OKLab lightness {lightness:.3}",
            theme.polarity()
        ));
    }

    findings.at_least("body text contrast", contrast(foreground, background), 4.5);
    findings.at_least("body text |Lc|", apca(foreground, background).abs(), 60.0);
    findings.at_least(
        "selection text contrast",
        contrast(roles.text_selection, roles.surface_selection),
        4.5,
    );
    findings.at_least(
        "selection ΔE from the canvas",
        delta_e(roles.surface_selection, background),
        6.0,
    );
    findings.at_least(
        "cursor contrast",
        contrast(canvas.cursor.value(), background),
        3.0,
    );

    // ANSI 0 is exempt on dark canvases; ANSI 0, 7 and 15 are exempt on light canvases.
    let mut legible = |index: usize, floor: f64| {
        findings.at_least(
            &format!("ANSI {index} contrast"),
            contrast(ansi[index], background),
            floor,
        );
    };
    (1..=6).for_each(|index| legible(index, 3.0));
    (8..=14).for_each(|index| legible(index, 2.0));
    if dark {
        legible(7, 3.0);
        legible(15, 2.0);
    }

    let correctable = [
        ("cursor".to_owned(), Some(canvas.cursor)),
        (
            "selection background".to_owned(),
            canvas.selection_background,
        ),
        (
            "selection foreground".to_owned(),
            canvas.selection_foreground,
        ),
    ]
    .into_iter()
    .chain(
        theme
            .ansi()
            .iter()
            .enumerate()
            .map(|(index, color)| (format!("ANSI {index}"), Some(*color))),
    );
    let mut total = 0.0;
    for (slot, color) in correctable {
        if let Some((original, value)) =
            color.and_then(|color| color.original().map(|original| (original, color.value())))
        {
            let difference = delta_e(original, value);
            findings.at_most(&format!("{slot} correction ΔE"), difference, 10.0);
            total += difference;
        }
    }
    findings.at_most("total correction ΔE", total, 20.0);

    for set in [1..=6, 9..=14] {
        let set: Vec<usize> = set.collect();
        for (position, &left) in set.iter().enumerate() {
            for &right in &set[position + 1..] {
                findings.at_least(
                    &format!("ANSI {left}/{right} ΔE"),
                    delta_e(ansi[left], ansi[right]),
                    10.0,
                );
            }
        }
    }
    for deficiency in [Deficiency::Protanopia, Deficiency::Deuteranopia] {
        for (red, green) in [(1, 2), (9, 10)] {
            findings.at_least(
                &format!("ANSI {red}/{green} ΔE under {deficiency:?}"),
                delta_e(
                    simulate(ansi[red], deficiency),
                    simulate(ansi[green], deficiency),
                ),
                5.0,
            );
        }
    }

    let glyphs = [
        (
            "attention",
            roles.agent_attention,
            reference.agent_attention,
        ),
        ("done", roles.agent_done, reference.agent_done),
        ("error", roles.agent_error, reference.agent_error),
    ];
    let surfaces = [
        ("canvas", roles.surface_canvas, reference.surface_canvas),
        ("chrome", roles.surface_chrome, reference.surface_chrome),
        ("current", roles.surface_current, reference.surface_current),
    ];
    for (glyph, color, accepted) in glyphs {
        if color != accepted {
            findings.fail(format!("the fixed {glyph} glyph is themed as {color}"));
        }
        for (surface, fill, accepted_fill) in surfaces {
            findings.at_least(
                &format!("{glyph} glyph on {surface}, relative to Orchbee Light"),
                contrast(color, fill) / contrast(accepted, accepted_fill),
                0.9,
            );
        }
    }

    let groups = roles.tab_groups;
    let floors: [(&str, Rgb, f64); 19] = [
        ("text-secondary", roles.text_secondary, 4.5),
        ("text-muted", roles.text_muted, 4.5),
        ("accent-action", roles.accent_action, 3.0),
        ("accent-focus", roles.accent_focus, 3.0),
        ("accent-link", roles.accent_link, 4.5),
        ("accent-destructive", roles.accent_destructive, 4.5),
        ("status-success", roles.status_success, 4.5),
        ("status-warning", roles.status_warning, 4.5),
        ("status-error", roles.status_error, 4.5),
        ("status-info", roles.status_info, 4.5),
        ("tab-group blue", groups.blue, 4.5),
        ("tab-group cyan", groups.cyan, 4.5),
        ("tab-group green", groups.green, 4.5),
        ("tab-group grey", groups.grey, 4.5),
        ("tab-group orange", groups.orange, 4.5),
        ("tab-group pink", groups.pink, 4.5),
        ("tab-group purple", groups.purple, 4.5),
        ("tab-group red", groups.red, 4.5),
        ("tab-group yellow", groups.yellow, 4.5),
    ];
    for (role, color, floor) in floors {
        findings.at_least(
            &format!("resolved {role} on chrome"),
            contrast(color, roles.surface_chrome),
            floor,
        );
    }
    // Clients paint these foregrounds on panels, overlay inputs and interaction rows.
    let surfaces = [
        ("canvas", roles.surface_canvas),
        ("chrome", roles.surface_chrome),
        ("overlay", roles.surface_overlay),
        ("field", roles.surface_field),
        ("related", roles.surface_related),
        ("hover", roles.surface_hover),
        ("current", roles.surface_current),
        ("pressed", roles.surface_pressed),
    ];
    for (surface, fill) in surfaces {
        for (role, color, floor) in floors
            .into_iter()
            .filter(|(role, _, _)| *role != "text-muted")
            .chain([
                ("text-primary", roles.text_primary, 4.5),
                ("status-pending", roles.status_pending, 4.5),
                ("status-working", roles.status_working, 4.5),
                ("status-idle", roles.status_idle, 3.0),
                ("status-unread", roles.status_unread, 3.0),
                ("frame-focused", roles.frame_focused, 3.0),
            ])
        {
            findings.at_least(
                &format!("resolved {role} on {surface}"),
                contrast(color, fill),
                floor,
            );
        }
        findings.at_least(
            &format!("primary/secondary hierarchy on {surface}"),
            contrast(roles.text_primary, fill) / contrast(roles.text_secondary, fill),
            1.0,
        );
    }
    findings.at_least(
        "copy cursor inverse on focus",
        contrast(roles.text_inverse, roles.accent_focus),
        4.5,
    );
    findings.failures
}
