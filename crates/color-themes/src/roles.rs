//! Resolves a theme's palette into every semantic color role: OKLab surface steps and interaction
//! mixes, frame and text steps, ANSI accents lightness-corrected to their contrast floors on
//! their painted surfaces, and the Tab-group accents. Agent glyph colors are fixed for each
//! polarity. It does not quantize or choose modifiers; the README's resolver table is the authority
//! for each rule.

use crate::{Polarity, Rgb, Theme, contrast, oklab};

/// The needs-attention, completed and failed glyph colors of every theme of `polarity`. Dark themes
/// take the macOS window-control traffic light; light themes take its hues deepened to 3:1 on
/// light canvas, chrome and selected rows, where the traffic light reads at 1.3–2.9:1.
#[must_use]
pub const fn agent_colors(polarity: Polarity) -> [Rgb; 3] {
    match polarity {
        Polarity::Dark => [
            Rgb::new(0xfe, 0xbc, 0x2e),
            Rgb::new(0x28, 0xc8, 0x40),
            Rgb::new(0xff, 0x5f, 0x57),
        ],
        Polarity::Light => [
            Rgb::new(0xad, 0x6f, 0x00),
            Rgb::new(0x00, 0x91, 0x00),
            Rgb::new(0xe1, 0x42, 0x3e),
        ],
    }
}

const BODY_FLOOR: f64 = 4.5;
const NON_TEXT_FLOOR: f64 = 3.0;
/// The GUI's primary-action hover and press keep these shares of the accent, the rest mixed
/// toward the label's contrast pole (`client_files.rs` writes the same proportions).
const ACTION_HOVER_KEEP_PERCENT: u8 = 88;
const ACTION_PRESSED_KEEP_PERCENT: u8 = 76;

/// The resolved color of every role a theme supplies, named as in the visual-foundations role
/// inventory. Text and semantic accents meet their floors on shared painted surfaces; muted rests on chrome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Roles {
    /// The theme background.
    pub surface_canvas: Rgb,
    /// Panels and bars around the canvas: one lightness step from it.
    pub surface_chrome: Rgb,
    /// Tab and status strips; the same value as chrome.
    pub surface_strip: Rgb,
    /// Menus, dialogs, and popovers.
    pub surface_overlay: Rgb,
    /// Text inputs on an overlay.
    pub surface_field: Rgb,
    /// Rows related to the current one.
    pub surface_related: Rgb,
    /// A hovered row or control.
    pub surface_hover: Rgb,
    /// The current row.
    pub surface_current: Rgb,
    /// A pressed control.
    pub surface_pressed: Rgb,
    /// Selected text and cells.
    pub surface_selection: Rgb,
    /// An inactive pane's mask; the same value as `surface_canvas`.
    pub surface_inactive: Rgb,
    /// The primary action's fill: the accent itself, under `text_inverse`.
    pub surface_action_primary: Rgb,
    /// The primary action's fill under the pointer: the accent mixed toward the pole that gives
    /// `text_inverse` more contrast.
    pub surface_action_primary_hover: Rgb,
    /// The primary action's fill while pressed: the same mix, with a larger share of the pole.
    pub surface_action_primary_pressed: Rgb,
    /// The ordinary action's fill; the same value as `surface_hover`, under `text_primary`.
    pub surface_action_secondary: Rgb,
    /// The ordinary action's fill under the pointer; the same value as `surface_current`.
    pub surface_action_secondary_hover: Rgb,
    /// The ordinary action's fill while pressed; the same value as `surface_pressed`.
    pub surface_action_secondary_pressed: Rgb,
    /// The focused pane or overlay frame; the same value as `accent_focus`.
    pub frame_focused: Rgb,
    /// Indent guides and the quietest rules.
    pub frame_guide: Rgb,
    /// Dividers between regions.
    pub frame_divider: Rgb,
    /// Frames of unfocused panes.
    pub frame_unfocused: Rgb,
    /// Body text: the theme foreground, corrected for contrast and secondary hierarchy.
    pub text_primary: Rgb,
    /// At least 4.5:1 on chrome.
    pub text_secondary: Rgb,
    /// The quietest step from secondary toward chrome that still holds 4.5:1 on chrome.
    pub text_muted: Rgb,
    /// Unavailable content: muted mixed toward chrome; exempt from the floors.
    pub text_disabled: Rgb,
    /// Copy-mode text on focus: canvas corrected to the body-text floor.
    pub text_inverse: Rgb,
    /// Text on `surface_selection`: the theme selection foreground, else the foreground.
    pub text_selection: Rgb,
    /// At least 3:1 on chrome.
    pub accent_action: Rgb,
    /// At least 3:1 on chrome.
    pub accent_focus: Rgb,
    /// At least 4.5:1 on chrome.
    pub accent_link: Rgb,
    /// At least 4.5:1 on chrome; a separate role from `status_error`.
    pub accent_destructive: Rgb,
    /// At least 4.5:1 on chrome.
    pub status_success: Rgb,
    /// At least 4.5:1 on chrome.
    pub status_warning: Rgb,
    /// At least 4.5:1 on chrome.
    pub status_error: Rgb,
    /// At least 4.5:1 on chrome.
    pub status_info: Rgb,
    /// Unread markers; the same value as `accent_action`.
    pub status_unread: Rgb,
    /// A pending operation; the same value as `text_secondary`.
    pub status_pending: Rgb,
    /// A working Agent; the same value as `text_secondary`.
    pub status_working: Rgb,
    /// An idle Agent; the same value as `text_muted`.
    pub status_idle: Rgb,
    /// The needs-attention glyph color fixed for the theme's polarity.
    pub agent_attention: Rgb,
    /// The completed glyph color fixed for the theme's polarity.
    pub agent_done: Rgb,
    /// The failed glyph color fixed for the theme's polarity.
    pub agent_error: Rgb,
    /// One accent per shared Tab-group color.
    pub tab_groups: TabGroups,
    /// Semantic hues and Tab-group accents lightness-corrected onto `surface_selection`.
    pub selection: SelectionAccents,
}

/// Semantic accents and Tab-group accents lightness-corrected onto `surface_selection`. Fixed
/// Agent glyph colors stay unshifted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionAccents {
    /// At least 3:1 on `surface_selection`.
    pub accent_action: Rgb,
    /// At least 3:1 on `surface_selection`.
    pub accent_focus: Rgb,
    /// At least 4.5:1 on `surface_selection`.
    pub accent_link: Rgb,
    /// At least 4.5:1 on `surface_selection`.
    pub accent_destructive: Rgb,
    /// At least 4.5:1 on `surface_selection`.
    pub status_success: Rgb,
    /// At least 4.5:1 on `surface_selection`.
    pub status_warning: Rgb,
    /// At least 4.5:1 on `surface_selection`.
    pub status_error: Rgb,
    /// At least 4.5:1 on `surface_selection`.
    pub status_info: Rgb,
    /// At least 3:1 on `surface_selection`.
    pub status_unread: Rgb,
    /// Each Tab-group accent raised to at least 4.5:1 on `surface_selection`.
    pub tab_groups: TabGroups,
}

/// The Tab-group accents, one per shared group color, each at least 4.5:1 on chrome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabGroups {
    /// ANSI 4.
    pub blue: Rgb,
    /// ANSI 6.
    pub cyan: Rgb,
    /// ANSI 2.
    pub green: Rgb,
    /// ANSI 8.
    pub grey: Rgb,
    /// ANSI 1 mixed halfway toward ANSI 3.
    pub orange: Rgb,
    /// ANSI 5 mixed 30% toward ANSI 1.
    pub pink: Rgb,
    /// ANSI 5 mixed halfway toward ANSI 4.
    pub purple: Rgb,
    /// ANSI 1.
    pub red: Rgb,
    /// ANSI 3.
    pub yellow: Rgb,
}

impl Roles {
    /// GUI status fill mix percentages (`rest`, `hover`) over neutral surfaces.
    #[must_use]
    pub const fn status_surface_tints(polarity: Polarity) -> (u8, u8) {
        match polarity {
            Polarity::Light => (10, 15),
            Polarity::Dark => (15, 20),
        }
    }

    /// Resolves every role of `theme` from its declared polarity, canvas, and corrected ANSI values.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        let canvas = theme.canvas();
        let ansi = theme.ansi().map(crate::PaletteColor::value);
        let [_, red, green, yellow, blue, magenta, cyan, _, grey, ..] = ansi;
        let background = canvas.background;
        let foreground = canvas.foreground;
        let lightness = oklab::to_oklab(background).l;
        let (chrome, overlay, field_mix) = match theme.polarity() {
            Polarity::Dark => (
                // A near-black canvas has no darker step left, so its chrome lifts instead.
                oklab::offset_lightness(background, if lightness < 0.16 { 0.03 } else { -0.025 }),
                oklab::offset_lightness(background, 0.045),
                0.07,
            ),
            Polarity::Light => (
                oklab::offset_lightness(background, -0.02),
                // A near-white canvas has no lighter step left, so overlays stay on it.
                if lightness > 0.98 {
                    background
                } else {
                    oklab::offset_lightness(background, 0.02)
                },
                0.05,
            ),
        };
        let toward_text = |amount: f64| oklab::mix(chrome, foreground, amount);
        let selection = canvas.selection_background.map_or_else(
            || oklab::mix(background, blue, 0.30),
            crate::PaletteColor::value,
        );
        let (tint_rest, tint_hover) = Self::status_surface_tints(theme.polarity());
        let resting = [background, chrome, overlay];
        let subtle_over = |base| srgb_composite(foreground, base, 5);
        let gui_alphas = resting.into_iter().flat_map(|base| {
            let subtle = subtle_over(base);
            [5, 6, 8, 11, 14]
                .into_iter()
                .map(move |percent| srgb_composite(foreground, base, percent))
                .chain(
                    [8, 11, 14]
                        .into_iter()
                        .map(move |percent| srgb_composite(foreground, subtle, percent)),
                )
        });
        let surfaces: Vec<Rgb> = [
            background,
            chrome,
            overlay,
            oklab::mix(overlay, foreground, field_mix),
            toward_text(0.05),
            toward_text(0.08),
            toward_text(0.11),
            toward_text(0.14),
        ]
        .into_iter()
        .chain(gui_alphas)
        .collect();
        // Pick the most demanding painted surface, preserving hue and all surface fills.
        let on_surfaces = |color: Rgb, floor: f64| {
            surfaces.iter().copied().fold(color, |color, surface| {
                contrast::raise(color, surface, floor)
            })
        };
        let on_tinted = |initial: Rgb| -> Rgb {
            let mut color = on_surfaces(initial, BODY_FLOOR);
            // Status fills mix `color` itself over canvas/chrome/overlay; iterate to a fixed point.
            for _ in 0..4 {
                for base in resting {
                    for percent in [tint_rest, tint_hover] {
                        color = contrast::raise(
                            color,
                            srgb_composite(color, base, percent),
                            BODY_FLOOR,
                        );
                    }
                }
            }
            color
        };
        let status_success = on_tinted(green);
        let status_warning = on_tinted(yellow);
        let status_error = on_tinted(red);
        let status_info = on_tinted(cyan);
        let status_fills: Vec<Rgb> = [status_info, status_success, status_warning, status_error]
            .into_iter()
            .flat_map(|fill| {
                resting.into_iter().flat_map(move |base| {
                    [tint_rest, tint_hover]
                        .into_iter()
                        .map(move |percent| srgb_composite(fill, base, percent))
                })
            })
            .collect();
        let on_all_gui = |color: Rgb, floor: f64| {
            surfaces
                .iter()
                .chain(status_fills.iter())
                .copied()
                .fold(color, |color, surface| {
                    contrast::raise(color, surface, floor)
                })
        };
        let text_secondary = on_all_gui(oklab::mix(foreground, chrome, 0.28), BODY_FLOOR);
        let muted_surfaces = [
            background,
            chrome,
            overlay,
            subtle_over(background),
            subtle_over(chrome),
            subtle_over(overlay),
        ];
        let text_muted = quietest_toward(text_secondary, chrome, BODY_FLOOR, &muted_surfaces);
        let text_primary = surfaces.iter().chain(status_fills.iter()).copied().fold(
            foreground,
            |color, surface| {
                contrast::raise(
                    color,
                    surface,
                    BODY_FLOOR.max(contrast::ratio(text_secondary, surface)),
                )
            },
        );
        let accent_action = on_all_gui(blue, NON_TEXT_FLOOR);
        let accent_focus = accent_action;
        let accent_link = on_all_gui(blue, BODY_FLOOR);
        let accent_destructive = on_all_gui(red, BODY_FLOOR);
        let status_success = on_all_gui(status_success, BODY_FLOOR);
        let status_warning = on_all_gui(status_warning, BODY_FLOOR);
        let status_error = on_all_gui(status_error, BODY_FLOOR);
        let status_info = on_all_gui(status_info, BODY_FLOOR);
        let status_unread = accent_action;
        let [agent_attention, agent_done, agent_error] = agent_colors(theme.polarity());
        let group = |color: Rgb| on_surfaces(color, BODY_FLOOR);
        let tab_groups = TabGroups {
            blue: group(blue),
            cyan: group(cyan),
            green: group(green),
            grey: group(grey),
            orange: group(oklab::mix(red, yellow, 0.5)),
            pink: group(oklab::mix(magenta, red, 0.3)),
            purple: group(oklab::mix(magenta, blue, 0.5)),
            red: group(red),
            yellow: group(yellow),
        };
        let on_sel = |color: Rgb, floor: f64| contrast::raise(color, selection, floor);
        let selection_accents = SelectionAccents {
            accent_action: on_sel(accent_action, NON_TEXT_FLOOR),
            accent_focus: on_sel(accent_focus, NON_TEXT_FLOOR),
            accent_link: on_sel(accent_link, BODY_FLOOR),
            accent_destructive: on_sel(accent_destructive, BODY_FLOOR),
            status_success: on_sel(status_success, BODY_FLOOR),
            status_warning: on_sel(status_warning, BODY_FLOOR),
            status_error: on_sel(status_error, BODY_FLOOR),
            status_info: on_sel(status_info, BODY_FLOOR),
            status_unread: on_sel(status_unread, NON_TEXT_FLOOR),
            tab_groups: TabGroups {
                blue: on_sel(tab_groups.blue, BODY_FLOOR),
                cyan: on_sel(tab_groups.cyan, BODY_FLOOR),
                green: on_sel(tab_groups.green, BODY_FLOOR),
                grey: on_sel(tab_groups.grey, BODY_FLOOR),
                orange: on_sel(tab_groups.orange, BODY_FLOOR),
                pink: on_sel(tab_groups.pink, BODY_FLOOR),
                purple: on_sel(tab_groups.purple, BODY_FLOOR),
                red: on_sel(tab_groups.red, BODY_FLOOR),
                yellow: on_sel(tab_groups.yellow, BODY_FLOOR),
            },
        };
        // The primary label is `text_inverse`, so the accent's hover and press move toward the
        // pole that gives that label more contrast, as the GUI's primary-action states do.
        let text_inverse = contrast::raise(background, accent_action, BODY_FLOOR);
        let action_pole = if contrast::ratio(text_inverse, Rgb::new(255, 255, 255))
            < contrast::ratio(text_inverse, Rgb::new(0, 0, 0))
        {
            Rgb::new(0, 0, 0)
        } else {
            Rgb::new(255, 255, 255)
        };
        let action_fill = |keep_percent: u8| {
            oklab::mix(
                accent_action,
                action_pole,
                f64::from(100 - keep_percent) / 100.0,
            )
        };
        Self {
            surface_canvas: background,
            surface_chrome: chrome,
            surface_strip: chrome,
            surface_overlay: overlay,
            surface_field: oklab::mix(overlay, foreground, field_mix),
            surface_related: toward_text(0.05),
            surface_hover: toward_text(0.08),
            surface_current: toward_text(0.11),
            surface_pressed: toward_text(0.14),
            surface_selection: selection,
            surface_inactive: background,
            surface_action_primary: accent_action,
            surface_action_primary_hover: action_fill(ACTION_HOVER_KEEP_PERCENT),
            surface_action_primary_pressed: action_fill(ACTION_PRESSED_KEEP_PERCENT),
            surface_action_secondary: toward_text(0.08),
            surface_action_secondary_hover: toward_text(0.11),
            surface_action_secondary_pressed: toward_text(0.14),
            frame_focused: accent_action,
            frame_guide: toward_text(0.10),
            frame_divider: toward_text(0.18),
            frame_unfocused: toward_text(0.24),
            text_primary,
            text_secondary,
            text_muted,
            text_disabled: oklab::mix(text_muted, chrome, 0.45),
            text_inverse,
            text_selection: canvas
                .selection_foreground
                .map_or(foreground, crate::PaletteColor::value),
            accent_action,
            accent_focus,
            accent_link,
            accent_destructive,
            status_success,
            status_warning,
            status_error,
            status_info,
            status_unread,
            status_pending: text_secondary,
            status_working: text_secondary,
            status_idle: on_surfaces(text_muted, NON_TEXT_FLOOR),
            agent_attention,
            agent_done,
            agent_error,
            tab_groups,
            selection: selection_accents,
        }
    }

    /// Lightness-corrects `color` (preserving hue) onto `self.surface_selection` to `floor`.
    #[must_use]
    pub fn raise_on_selection(&self, color: Rgb, floor: f64) -> Rgb {
        contrast::raise(color, self.surface_selection, floor)
    }
}

fn srgb_composite(foreground: Rgb, background: Rgb, opacity_percent: u8) -> Rgb {
    let p = u16::from(opacity_percent);
    let channel = |fg: u8, bg: u8| {
        let mixed = (u16::from(fg) * p + u16::from(bg) * (100 - p) + 50) / 100;
        u8::try_from(mixed).expect("weighted mean of two channels stays a channel")
    };
    Rgb::new(
        channel(foreground.red, background.red),
        channel(foreground.green, background.green),
        channel(foreground.blue, background.blue),
    )
}

/// The step from `color` toward `surface` closest to the surface that still holds `floor` on it.
fn quietest_toward(color: Rgb, surface: Rgb, floor: f64, surfaces: &[Rgb]) -> Rgb {
    let (mut passing, mut failing) = (0.0, 1.0);
    let mut quietest = color;
    for _ in 0..40 {
        let middle = f64::midpoint(passing, failing);
        let candidate = oklab::mix(color, surface, middle);
        if surfaces
            .iter()
            .all(|&fill| contrast::ratio(candidate, fill) >= floor)
        {
            quietest = candidate;
            passing = middle;
        } else {
            failing = middle;
        }
    }
    quietest
}
