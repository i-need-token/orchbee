//! Exact resize-mode bindings shared by pure input decisions and localized discovery.
use super::{Binding, KeyCode, KeyEvent, KeyKind, LabelStyle, Modifiers, PanelDirection};

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

pub(super) fn labels(fine: bool) -> String {
    let mods = if fine {
        Modifiers::NONE.with_shift()
    } else {
        Modifiers::NONE
    };
    ARROWS
        .into_iter()
        .map(|(code, _)| super::format_binding(&Binding::new(code, mods), LabelStyle::for_host()))
        .collect::<Vec<_>>()
        .join(" / ")
}

pub(super) fn help_label() -> String {
    super::format_binding(&HELP, LabelStyle::for_host())
}
pub(super) fn exit_label() -> String {
    super::format_binding(&EXIT, LabelStyle::for_host())
}
pub(super) fn page_label() -> String {
    PAGES
        .iter()
        .map(|binding| super::format_binding(binding, LabelStyle::for_host()))
        .collect::<Vec<_>>()
        .join(" / ")
}

#[cfg(test)]
mod tests;
