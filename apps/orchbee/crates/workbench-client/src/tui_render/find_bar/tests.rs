use super::*;
use crate::terminal_copy_mode::EpisodeId;
use crate::terminal_find::FindMemory;
use ratatui::{Terminal, backend::TestBackend};
use std::{num::NonZeroU64, time::Instant};

fn visible_row(buffer: &ratatui::buffer::Buffer, width: u16) -> String {
    let mut column = 0;
    let mut text = String::new();
    while column < width {
        let symbol = buffer[(column, 0)].symbol();
        text.push_str(symbol);
        column += u16::try_from(display_width(symbol).max(1)).unwrap();
    }
    text
}

#[test]
fn narrow_panels_drop_controls_but_keep_the_label_draft_and_counter() {
    let memory = FindMemory {
        query: "error".into(),
        match_case: false,
    };
    let bar = FindBar::new(&memory, Instant::now());
    let token = FindToken(EpisodeId::new(NonZeroU64::new(1).unwrap()));
    // Columns, whether `Aa` shows, and how many bracketed controls show.
    for (width, match_case, brackets) in [(40, true, 3), (28, false, 3), (20, false, 1)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let theme = Theme::terminal_native();
            let mut terminal = Terminal::new(TestBackend::new(width, 3)).unwrap();
            let mut hits = HitMapBuilder::new();
            let mut cursor = None;
            terminal
                .draw(|frame| {
                    let area = frame.area();
                    cursor = render(frame, area, area, &bar, token, locale, &theme, &mut hits);
                })
                .unwrap();
            let text = visible_row(terminal.backend().buffer(), width);
            assert!(text.contains(locale.find_bar_label()), "{width}: {text}");
            assert!(text.contains("error"), "{width}: {text}");
            assert!(text.contains(theme.glyphs.ellipsis), "{width}: {text}");
            assert_eq!(text.contains("Aa"), match_case, "{width}: {text}");
            assert_eq!(text.matches('[').count(), brackets, "{width}: {text}");
            assert!(cursor.is_some(), "{width}: the draft keeps the caret");
        }
    }
}
