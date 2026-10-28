//! Opt-in whole-screen evidence from the same `TestBackend` buffer tests assert.

use ratatui::{Terminal, backend::TestBackend};

use crate::{localization::Locale, tui_theme::Theme};

/// Write one complete styled terminal frame when an external review owns a capture directory.
///
/// Ordinary test runs have no filesystem side effect. The invoking review creates and later
/// releases `ORCHBEE_TUI_CAPTURE_DIR`; this helper never invents a repository snapshot store.
pub fn capture_screen(
    terminal: &mut Terminal<TestBackend>,
    name: &str,
    locale: Locale,
    theme: &Theme,
) {
    let Some(directory) = std::env::var_os("ORCHBEE_TUI_CAPTURE_DIR") else {
        return;
    };
    let cursor_visible = terminal.backend().cursor_visible();
    let cursor = terminal.get_cursor_position().unwrap();
    let buffer = terminal.backend().buffer();
    let cells: Vec<_> = buffer
        .content()
        .iter()
        .map(|cell| {
            serde_json::json!({
                "symbol": cell.symbol(),
                "fg": format!("{:?}", cell.fg),
                "bg": format!("{:?}", cell.bg),
                "modifier": format!("{:?}", cell.modifier),
                "diff_option": format!("{:?}", cell.diff_option),
            })
        })
        .collect();
    let path = std::path::Path::new(&directory).join(format!("{name}.json"));
    std::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "schema": "orchbee.tui.test-backend.capture.v1",
            "source": "ratatui-test-backend",
            "name": name,
            "width": buffer.area.width,
            "height": buffer.area.height,
            "cell_order": "row-major",
            "locale": crate::localization::locale_name(locale),
            "resolved_theme": {
                "name": theme.name,
                "polarity": format!("{:?}", theme.polarity),
                "corners": format!("{:?}", theme.corners),
                "glyph_set": if theme.glyphs == crate::tui_theme::GlyphSet::ASCII {
                    "ascii"
                } else {
                    "utf-8"
                },
                // Theme's test-only Debug value includes the resolved color level, host-color
                // decision, and every semantic role style without exposing production accessors.
                "value": format!("{theme:?}"),
            },
            "cursor": {
                "visible": cursor_visible,
                "x": cursor.x,
                "y": cursor.y,
            },
            "cells": cells,
        }))
        .unwrap(),
    )
    .unwrap();
}
