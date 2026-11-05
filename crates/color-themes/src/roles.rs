//! Resolves a theme's palette into every semantic color role: OKLab surface steps and interaction
//! mixes, frame and text steps, ANSI accents lightness-corrected to their contrast floors on
//! their painted surfaces, and the Tab-group accents. Agent glyph colors are fixed for every theme. It does not
//! quantize or choose modifiers; the README's resolver table is the authority for each rule.

use crate::{Polarity, Rgb, Theme, contrast, oklab};

const AGENT_ATTENTION: Rgb = Rgb::new(0xfe, 0xbc, 0x2e);
const AGENT_DONE: Rgb = Rgb::new(0x28, 0xc8, 0x40);
const AGENT_ERROR: Rgb = Rgb::new(0xff, 0x5f, 0x57);

const BODY_FLOOR: f64 = 4.5;
const NON_TEXT_FLOOR: f64 = 3.0;

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
    /// The fixed needs-attention glyph color.
    pub agent_attention: Rgb,
    /// The fixed completed glyph color.
    pub agent_done: Rgb,
    /// The fixed failed glyph color.
    pub agent_error: Rgb,
    /// One accent per shared Tab-group color.
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
        let surfaces = [
            background,
            chrome,
            overlay,
            oklab::mix(overlay, foreground, field_mix),
            toward_text(0.05),
            toward_text(0.08),
            toward_text(0.11),
            toward_text(0.14),
        ];
        // Pick the most demanding painted surface, preserving hue and all surface fills.
        let on_surfaces = |color: Rgb, floor: f64| {
            surfaces.into_iter().fold(color, |color, surface| {
                contrast::raise(color, surface, floor)
            })
        };
        let text_secondary = on_surfaces(oklab::mix(foreground, chrome, 0.28), BODY_FLOOR);
        let text_muted = quietest_toward(text_secondary, chrome, BODY_FLOOR, &[chrome]);
        let text_primary = surfaces.into_iter().fold(foreground, |color, surface| {
            contrast::raise(
                color,
                surface,
                BODY_FLOOR.max(contrast::ratio(text_secondary, surface)),
            )
        });
        let accent_action = on_surfaces(blue, NON_TEXT_FLOOR);
        let group = |color: Rgb| on_surfaces(color, BODY_FLOOR);
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
            frame_focused: accent_action,
            frame_guide: toward_text(0.10),
            frame_divider: toward_text(0.18),
            frame_unfocused: toward_text(0.24),
            text_primary,
            text_secondary,
            text_muted,
            text_disabled: oklab::mix(text_muted, chrome, 0.45),
            text_inverse: contrast::raise(background, accent_action, BODY_FLOOR),
            text_selection: canvas
                .selection_foreground
                .map_or(foreground, crate::PaletteColor::value),
            accent_action,
            accent_focus: accent_action,
            accent_link: on_surfaces(blue, BODY_FLOOR),
            accent_destructive: on_surfaces(red, BODY_FLOOR),
            status_success: on_surfaces(green, BODY_FLOOR),
            status_warning: on_surfaces(yellow, BODY_FLOOR),
            status_error: on_surfaces(red, BODY_FLOOR),
            status_info: on_surfaces(cyan, BODY_FLOOR),
            status_unread: accent_action,
            status_pending: text_secondary,
            status_working: text_secondary,
            status_idle: on_surfaces(text_muted, NON_TEXT_FLOOR),
            agent_attention: AGENT_ATTENTION,
            agent_done: AGENT_DONE,
            agent_error: AGENT_ERROR,
            tab_groups: TabGroups {
                blue: group(blue),
                cyan: group(cyan),
                green: group(green),
                grey: group(grey),
                orange: group(oklab::mix(red, yellow, 0.5)),
                pink: group(oklab::mix(magenta, red, 0.3)),
                purple: group(oklab::mix(magenta, blue, 0.5)),
                red: group(red),
                yellow: group(yellow),
            },
        }
    }
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
