//! Exact resize-mode bindings shared by pure input decisions and localized discovery.
use super::{
    Binding, KeyCode, KeyEvent, KeyKind, KeyLabelGlyphs, LabelStyle, Modifiers, PanelDirection,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResizeKey {
    Adjust {
        direction: PanelDirection,
        fine: bool,
    },
    Help,
    Exit,
    Page {
        previous: bool,
    },
}

const ARROWS: [(KeyCode, PanelDirection); 4] = [
    (KeyCode::Left, PanelDirection::Left),
    (KeyCode::Down, PanelDirection::Down),
    (KeyCode::Up, PanelDirection::Up),
    (KeyCode::Right, PanelDirection::Right),
];
const HELP: Binding = Binding::new(KeyCode::Char('?'), Modifiers::NONE);
const EXIT: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);
const PAGES: [Binding; 2] = [
    Binding::new(KeyCode::PageUp, Modifiers::NONE),
    Binding::new(KeyCode::PageDown, Modifiers::NONE),
];

pub(super) fn decide(event: KeyEvent) -> Option<ResizeKey> {
    if event.kind == KeyKind::Release {
        return None;
    }
    if EXIT.matches(event) {
        return Some(ResizeKey::Exit);
    }
    if HELP.matches(event) {
        return Some(ResizeKey::Help);
    }
    for (index, binding) in PAGES.iter().enumerate() {
        if binding.matches(event) {
            return Some(ResizeKey::Page {
                previous: index == 0,
            });
        }
    }
    for (code, direction) in ARROWS {
        for fine in [false, true] {
            let mods = if fine {
                Modifiers::NONE.with_shift()
            } else {
                Modifiers::NONE
            };
            if Binding::new(code, mods).matches(event) {
                return Some(ResizeKey::Adjust { direction, fine });
            }
        }
    }
    None
}

pub(super) fn labels_with_glyphs(fine: bool, glyphs: KeyLabelGlyphs) -> String {
    let mods = if fine {
        Modifiers::NONE.with_shift()
    } else {
        Modifiers::NONE
    };
    ARROWS
        .into_iter()
        .map(|(code, _)| {
            super::format_binding_with_glyphs(
                &Binding::new(code, mods),
                LabelStyle::for_host(),
                glyphs,
            )
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

pub(super) fn help_label_with_glyphs(glyphs: KeyLabelGlyphs) -> String {
    super::format_binding_with_glyphs(&HELP, LabelStyle::for_host(), glyphs)
}
pub(super) fn exit_label_with_glyphs(glyphs: KeyLabelGlyphs) -> String {
    super::format_binding_with_glyphs(&EXIT, LabelStyle::for_host(), glyphs)
}
pub(super) fn page_label_with_glyphs(glyphs: KeyLabelGlyphs) -> String {
    PAGES
        .iter()
        .map(|binding| super::format_binding_with_glyphs(binding, LabelStyle::for_host(), glyphs))
        .collect::<Vec<_>>()
        .join(" / ")
}

#[cfg(test)]
mod tests;
