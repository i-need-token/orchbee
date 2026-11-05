//! The generator of the GUI's tracked color-theme files and their staleness check. One CSS block
//! per named theme overrides every system color token at the theme's polarity boundary, and a
//! TypeScript module carries every shipped theme's terminal colors for `@orchbee/terminal`.
//!
//! The comparison fails when a tracked file differs from what the shipped themes generate;
//! `pnpm lint:styles` checks each generated block against the registry's system color inventory
//! and the colors a named theme never changes.
//! `pnpm generate:color-themes` runs it with `ORCHBEE_GENERATE_COLOR_THEMES` set to rewrite the
//! files instead; `ORCHBEE_COLOR_THEMES_OUT_DIR` redirects them, and the theme files listed in
//! `ORCHBEE_COLOR_THEMES_EXTRA` join the output there for review without shipping.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use orchbee_color_themes::{Polarity, Rgb, Roles, Theme, orchbee, shipped};

use crate::colorimetry::contrast;

const CSS_PATH: &str = "packages/design-tokens/src/color-themes.generated.css";
const TERMINAL_PATH: &str = "packages/terminal/src/color-themes/color-themes.generated.ts";

const CSS_HEADER: &str = "\
/*
 * Generated from crates/color-themes by `pnpm generate:color-themes`; do not edit.
 * Each block overrides every system color of one named theme at its polarity boundary. The
 * Orchbee pair keeps the hand-authored mapping in tokens.css and has no block.
 */
";

const TERMINAL_HEADER: &str = "\
// Generated from crates/color-themes by `pnpm generate:color-themes`; do not edit.
import type { TerminalColorTheme, TerminalColorThemePair } from './terminal-color-theme';
";

const ANSI_KEYS: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
];

#[test]
fn generated_client_files_match_the_shipped_themes() {
    if env::var_os("ORCHBEE_GENERATE_COLOR_THEMES").is_some() {
        generate();
        return;
    }
    let themes: Vec<&Theme> = shipped().iter().collect();
    let stale: Vec<&str> = client_files(&themes)
        .into_iter()
        .filter(|(path, text)| {
            fs::read_to_string(repository().join(path)).ok().as_deref() != Some(text.as_str())
        })
        .map(|(path, _)| path)
        .collect();
    assert!(
        stale.is_empty(),
        "stale generated color-theme files: {}; run `pnpm generate:color-themes`",
        stale.join(", ")
    );
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn client_files(themes: &[&Theme]) -> [(&'static str, String); 2] {
    [
        (CSS_PATH, css(themes)),
        (TERMINAL_PATH, terminal_module(themes)),
    ]
}

fn generate() {
    let extra: Vec<Theme> = env::var_os("ORCHBEE_COLOR_THEMES_EXTRA")
        .map(|paths| {
            env::split_paths(&paths)
                .map(|path| {
                    let text = fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                    Theme::parse(&text)
                        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
                })
                .collect()
        })
        .unwrap_or_default();
    let out_dir = env::var_os("ORCHBEE_COLOR_THEMES_OUT_DIR").map(PathBuf::from);
    assert!(
        extra.is_empty() || out_dir.is_some(),
        "extra theme files render only into an out dir"
    );
    let mut themes: Vec<&Theme> = shipped().iter().collect();
    for theme in &extra {
        assert!(
            themes.iter().all(|shipped| shipped.id() != theme.id()),
            "{} is already shipped",
            theme.id()
        );
        themes.push(theme);
    }
    for (path, text) in client_files(&themes) {
        let target = out_dir.as_ref().map_or_else(
            || repository().join(path),
            |directory| directory.join(Path::new(path).file_name().expect("file name")),
        );
        fs::write(&target, text).unwrap_or_else(|error| panic!("{}: {error}", target.display()));
    }
}

fn is_default(theme: &Theme) -> bool {
    theme.id() == orchbee(theme.polarity()).id()
}

const fn polarity_name(polarity: Polarity) -> &'static str {
    match polarity {
        Polarity::Light => "light",
        Polarity::Dark => "dark",
    }
}

fn css(themes: &[&Theme]) -> String {
    let mut text = String::from(CSS_HEADER);
    for theme in themes.iter().filter(|theme| !is_default(theme)) {
        text.push_str(&format!(
            "\n[data-theme='{}'][data-color-theme='{}'] {{\n",
            polarity_name(theme.polarity()),
            theme.id()
        ));
        for (name, value) in system_colors(theme) {
            declaration(&mut text, &name, &value);
        }
        text.push_str("}\n");
    }
    text
}

/// Writes one declaration as the repository formatter lays it out: a tab counts two columns, and
/// a `color-mix()` value past column 100 puts one argument on each line.
fn declaration(text: &mut String, name: &str, value: &str) {
    let line = format!("--ob-sys-color-{name}: {value};");
    if line.len() + 2 > 100
        && let Some(arguments) = value
            .strip_prefix("color-mix(")
            .and_then(|rest| rest.strip_suffix(')'))
    {
        text.push_str(&format!("\t--ob-sys-color-{name}: color-mix(\n"));
        let arguments: Vec<&str> = arguments.split(", ").collect();
        for (index, argument) in arguments.iter().enumerate() {
            let separator = if index + 1 < arguments.len() { "," } else { "" };
            text.push_str(&format!("\t\t{argument}{separator}\n"));
        }
        text.push_str("\t);\n");
    } else {
        text.push_str(&format!("\t{line}\n"));
    }
}

/// Every system color of a named theme, in the registry's order. Surfaces, text, frames, accents,
/// and statuses come from the resolver; states mix those roles as the hand-authored mapping does,
/// with hover lighter and pressed darker in both polarities. The brand honey, the scrim, and every
/// Agent status glyph color keep their polarity's hand-authored reference.
fn system_colors(theme: &Theme) -> Vec<(String, String)> {
    let roles = Roles::resolve(theme);
    let dark = theme.polarity() == Polarity::Dark;
    let fixed = |light: &str, dark_value: &str| {
        format!(
            "var(--ob-ref-color-{})",
            if dark { dark_value } else { light }
        )
    };
    let alpha = |percent: u8| {
        format!(
            "color-mix(in srgb, {} {percent}%, transparent)",
            roles.text_primary
        )
    };
    let toward = |color: Rgb, percent: u8, other: &str| {
        format!("color-mix(in oklab, {color} {percent}%, {other})")
    };
    let lighter = |color: Rgb| toward(color, 85, "white");
    let darker = |color: Rgb| toward(color, 85, "black");
    let chrome = roles.surface_chrome.to_string();
    let canvas = roles.surface_canvas.to_string();
    let primary = roles.text_primary.to_string();
    let secondary = roles.text_secondary.to_string();
    let muted = roles.text_muted.to_string();
    let placeholder = toward(roles.text_muted, 70, &chrome);
    let disabled = toward(roles.text_muted, 55, &chrome);
    let guide = roles.frame_guide.to_string();
    let focus = roles.accent_focus.to_string();
    let link = roles.accent_link;
    let action = roles.accent_action;
    let on_action =
        if contrast(roles.surface_canvas, action) >= contrast(roles.text_primary, action) {
            canvas.clone()
        } else {
            primary.clone()
        };
    let (thumb, thumb_hover, thumb_pressed) = if dark { (28, 40, 52) } else { (40, 52, 64) };
    let (tint, tint_hover) = if dark { (15, 20) } else { (10, 15) };
    let mut colors: Vec<(String, String)> = [
        ("brand-accent", fixed("amber-400", "amber-300")),
        ("brand-accent-hover", fixed("amber-300", "amber-200")),
        ("brand-on-accent", fixed("neutral-950", "neutral-950")),
        ("surface-canvas", canvas.clone()),
        ("surface-inactive", alpha(6)),
        ("surface-subtle", alpha(5)),
        ("surface-raised", chrome.clone()),
        ("surface-overlay", roles.surface_overlay.to_string()),
        ("surface-inverse", primary.clone()),
        ("surface-hovered", alpha(8)),
        ("surface-pressed", alpha(14)),
        ("surface-selected", alpha(11)),
        ("surface-selected-hover", alpha(14)),
        ("surface-disabled", alpha(8)),
        (
            "surface-scrim",
            format!(
                "color-mix(in oklch, var(--ob-ref-color-neutral-1000) {}%, transparent)",
                if dark { 70 } else { 55 }
            ),
        ),
        ("text-primary", primary.clone()),
        ("text-heading", primary.clone()),
        ("text-secondary", secondary.clone()),
        ("text-label", secondary.clone()),
        ("text-muted", muted.clone()),
        ("text-placeholder", placeholder),
        ("text-disabled", disabled.clone()),
        ("text-inverse", canvas.clone()),
        ("text-link", link.to_string()),
        ("text-link-hover", lighter(link)),
        ("text-link-focus", lighter(link)),
        ("text-link-pressed", darker(link)),
        ("icon-primary", primary.clone()),
        ("icon-secondary", secondary),
        ("icon-muted", muted),
        ("icon-inverse", canvas),
        ("icon-hover", primary.clone()),
        ("icon-disabled", disabled.clone()),
        ("border-subtle", guide.clone()),
        ("border-default", roles.frame_divider.to_string()),
        ("border-strong", roles.frame_unfocused.to_string()),
        ("border-focus", focus.clone()),
        ("border-selected", focus),
        ("border-disabled", guide.clone()),
        ("scrollbar-track", "transparent".to_owned()),
        ("scrollbar-thumb", alpha(thumb)),
        ("scrollbar-thumb-hover", alpha(thumb_hover)),
        ("scrollbar-thumb-pressed", alpha(thumb_pressed)),
        ("action-primary", action.to_string()),
        ("action-primary-hover", lighter(action)),
        ("action-primary-pressed", darker(action)),
        ("action-primary-text", on_action),
        ("action-secondary", roles.surface_hover.to_string()),
        ("action-secondary-hover", roles.surface_current.to_string()),
        (
            "action-secondary-pressed",
            roles.surface_pressed.to_string(),
        ),
        ("action-secondary-text", primary),
        ("action-disabled", roles.surface_related.to_string()),
        ("action-disabled-text", disabled),
        ("action-disabled-border", guide),
        (
            "status-attention",
            fixed("traffic-light-yellow", "traffic-light-yellow"),
        ),
        (
            "status-working",
            fixed("agent-working-light", "agent-working-dark"),
        ),
        ("status-agent-idle", fixed("gray-600", "groknight-gray")),
        (
            "status-agent-done",
            fixed("traffic-light-green", "traffic-light-green"),
        ),
        (
            "status-agent-error",
            fixed("traffic-light-red", "traffic-light-red"),
        ),
        ("status-unread", roles.status_unread.to_string()),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect();
    for (status, color) in [
        ("info", roles.status_info),
        ("success", roles.status_success),
        ("warning", roles.status_warning),
        ("danger", roles.status_error),
    ] {
        let tinted = |percent: u8| format!("color-mix(in oklch, {color} {percent}%, transparent)");
        for (variant, value) in [
            ("", color.to_string()),
            ("-hover", lighter(color)),
            ("-pressed", darker(color)),
            ("-surface", tinted(tint)),
            ("-surface-hover", tinted(tint_hover)),
            ("-border", toward(color, 50, &chrome)),
            ("-border-hover", toward(color, 65, &chrome)),
            ("-text", color.to_string()),
            ("-text-hover", lighter(color)),
            ("-text-pressed", darker(color)),
        ] {
            colors.push((format!("status-{status}{variant}"), value));
        }
    }
    let magenta = theme.ansi()[5].value();
    for (index, color) in [
        roles.accent_action,
        roles.status_success,
        roles.status_warning,
        magenta,
        roles.status_error,
    ]
    .into_iter()
    .enumerate()
    {
        colors.push((format!("data-{}", index + 1), color.to_string()));
    }
    colors
}

fn terminal_module(themes: &[&Theme]) -> String {
    let mut text = String::from(TERMINAL_HEADER);
    text.push_str("\n/** The Orchbee pair, the default theme of each polarity. */\n");
    text.push_str("export const orchbeeTerminalColorThemes: TerminalColorThemePair = {\n");
    for polarity in [Polarity::Dark, Polarity::Light] {
        text.push_str(&format!("\t{}: ", polarity_name(polarity)));
        terminal_theme(&mut text, orchbee(polarity), 1);
        text.push_str(",\n");
    }
    text.push_str("};\n");
    text.push_str("\n/** Every other shipped theme, in the crate's stable order. */\n");
    text.push_str("export const namedTerminalColorThemes: readonly TerminalColorTheme[] = [");
    let named: Vec<&&Theme> = themes.iter().filter(|theme| !is_default(theme)).collect();
    if named.is_empty() {
        text.push_str("];\n");
    } else {
        text.push('\n');
        for theme in named {
            text.push('\t');
            terminal_theme(&mut text, theme, 1);
            text.push_str(",\n");
        }
        text.push_str("];\n");
    }
    text
}

fn terminal_theme(text: &mut String, theme: &Theme, depth: usize) {
    let indent = "\t".repeat(depth + 1);
    let roles = Roles::resolve(theme);
    let canvas = theme.canvas();
    let selection_foreground = canvas
        .selection_foreground
        .map_or(canvas.foreground, |color| color.value());
    text.push_str("{\n");
    for (key, value) in [
        ("id", format!("'{}'", theme.id())),
        ("family", format!("'{}'", theme.family())),
        ("polarity", format!("'{}'", polarity_name(theme.polarity()))),
        ("background", hex_literal(canvas.background)),
        ("foreground", hex_literal(canvas.foreground)),
        ("cursor", hex_literal(canvas.cursor.value())),
        ("cursorText", hex_literal(canvas.cursor_text)),
        ("selectionBackground", hex_literal(roles.surface_selection)),
        ("selectionForeground", hex_literal(selection_foreground)),
    ] {
        text.push_str(&format!("{indent}{key}: {value},\n"));
    }
    text.push_str(&format!("{indent}ansi: {{\n"));
    for (key, color) in ANSI_KEYS.iter().zip(theme.ansi()) {
        text.push_str(&format!(
            "{indent}\t{key}: {},\n",
            hex_literal(color.value())
        ));
    }
    text.push_str(&format!("{indent}}},\n"));
    text.push_str(&"\t".repeat(depth));
    text.push('}');
}

fn hex_literal(color: Rgb) -> String {
    format!("0x{:02x}_{:02x}_{:02x}", color.red, color.green, color.blue)
}
