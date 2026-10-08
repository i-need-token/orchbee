//! Typed key vocabulary shared by the legacy input decoder, the prefix binding
//! registry, and help/status label formatting. This module is pure data plus
//! normalization; matching and display both consume the same canonical form.

/// The key a host event resolves to. `Enter`, `Tab`, and `Space` are authoring
/// aliases for canonical control chords (`Ctrl+M`, `Ctrl+I`, and literal
/// space); `canonical` folds them so matching never forks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyCode {
    Char(char),
    Esc,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    F(u8),
    Enter,
    Tab,
    Space,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Modifiers {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) super_: bool,
}

impl Modifiers {
    pub(crate) const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        super_: false,
    };
    pub(crate) const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };

    pub(crate) const fn with_ctrl(self) -> Self {
        Self { ctrl: true, ..self }
    }

    pub(crate) const fn with_alt(self) -> Self {
        Self { alt: true, ..self }
    }

    pub(crate) const fn with_shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }

    pub(crate) const fn with_super_(self) -> Self {
        Self {
            super_: true,
            ..self
        }
    }

    const fn without_ctrl(self) -> Self {
        Self {
            ctrl: false,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyKind {
    Press,
    Repeat,
    Release,
}

/// One decoded host key event. `kind` is `Press` unless the host protocol
/// reports repeat/release (kitty `CSI u`); legacy decoders only emit `Press`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct KeyEvent {
    pub(crate) code: KeyCode,
    pub(crate) mods: Modifiers,
    pub(crate) kind: KeyKind,
}

impl KeyEvent {
    pub(crate) const fn press(code: KeyCode, mods: Modifiers) -> Self {
        Self {
            code,
            mods,
            kind: KeyKind::Press,
        }
    }

    /// The canonical `(code, modifiers)` pair used by binding matching. An
    /// uppercase `Char` folds to lowercase plus Shift; `Enter`/`Tab`/`Space`
    /// fold to their canonical chords. This is the single classification —
    /// modifier supersets stay distinct, so `Ctrl+Shift+X` never matches a
    /// `Ctrl+X` binding.
    pub(crate) const fn normalized(self) -> (KeyCode, Modifiers) {
        canonical(self.code, self.mods)
    }
}

/// A normalized key-plus-complete-modifier-set binding. Two bindings match only
/// when their canonical pairs are identical.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Binding {
    pub(crate) code: KeyCode,
    pub(crate) mods: Modifiers,
}

impl Binding {
    pub(crate) const fn new(code: KeyCode, mods: Modifiers) -> Self {
        let (code, mods) = canonical(code, mods);
        Self { code, mods }
    }

    pub(crate) const fn matches(&self, event: KeyEvent) -> bool {
        let (code, mods) = event.normalized();
        code_equals(self.code, code) && mods_equal(self.mods, mods)
    }
}

const fn code_equals(left: KeyCode, right: KeyCode) -> bool {
    match (left, right) {
        (KeyCode::Char(left), KeyCode::Char(right)) => left == right,
        (KeyCode::F(left), KeyCode::F(right)) => left == right,
        (KeyCode::Esc, KeyCode::Esc)
        | (KeyCode::Backspace, KeyCode::Backspace)
        | (KeyCode::Delete, KeyCode::Delete)
        | (KeyCode::Insert, KeyCode::Insert)
        | (KeyCode::Home, KeyCode::Home)
        | (KeyCode::End, KeyCode::End)
        | (KeyCode::PageUp, KeyCode::PageUp)
        | (KeyCode::PageDown, KeyCode::PageDown)
        | (KeyCode::Up, KeyCode::Up)
        | (KeyCode::Down, KeyCode::Down)
        | (KeyCode::Left, KeyCode::Left)
        | (KeyCode::Right, KeyCode::Right)
        | (KeyCode::Enter, KeyCode::Enter)
        | (KeyCode::Tab, KeyCode::Tab)
        | (KeyCode::Space, KeyCode::Space) => true,
        _ => false,
    }
}

const fn mods_equal(left: Modifiers, right: Modifiers) -> bool {
    left.ctrl == right.ctrl
        && left.alt == right.alt
        && left.shift == right.shift
        && left.super_ == right.super_
}

const fn canonical(code: KeyCode, mods: Modifiers) -> (KeyCode, Modifiers) {
    match code {
        KeyCode::Enter => (KeyCode::Char('m'), mods.with_ctrl()),
        KeyCode::Tab => (KeyCode::Char('i'), mods.with_ctrl()),
        KeyCode::Space => (KeyCode::Char(' '), mods),
        KeyCode::Char(c) if c.is_ascii_uppercase() => {
            (KeyCode::Char(c.to_ascii_lowercase()), mods.with_shift())
        }
        _ => (code, mods),
    }
}

/// Display style: Apple hosts label modifiers with ⌃⌥⇧⌘ and no separators;
/// other hosts spell `Ctrl+Alt+Shift+Super+` before the key name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LabelStyle {
    Apple,
    Other,
}

impl LabelStyle {
    pub(crate) const fn for_host() -> Self {
        if cfg!(target_os = "macos") {
            Self::Apple
        } else {
            Self::Other
        }
    }
}

/// Renders a binding as the key column of help/status text. The named aliases
/// keep their display names: canonical `Ctrl+M` shows `Enter`, `Ctrl+I` shows
/// `Tab`, and bare space shows `Space`; extra modifiers stay explicit
/// (`Shift+Enter`, `Ctrl+Alt+M`-style classification never forks).
pub(crate) fn format_binding(binding: &Binding, style: LabelStyle) -> String {
    let mut mods = binding.mods;
    let name = match binding.code {
        KeyCode::Char('m') if mods.ctrl => {
            mods = mods.without_ctrl();
            "Enter".to_owned()
        }
        KeyCode::Char('i') if mods.ctrl => {
            mods = mods.without_ctrl();
            "Tab".to_owned()
        }
        KeyCode::Char(' ') if mods == Modifiers::NONE => "Space".to_owned(),
        KeyCode::Char(c) => {
            if mods != Modifiers::NONE && c.is_ascii_alphabetic() {
                c.to_ascii_uppercase().to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::Esc => "Esc".to_owned(),
        KeyCode::Backspace => "Backspace".to_owned(),
        KeyCode::Delete => "Delete".to_owned(),
        KeyCode::Insert => "Insert".to_owned(),
        KeyCode::Home => "Home".to_owned(),
        KeyCode::End => "End".to_owned(),
        KeyCode::PageUp => "PageUp".to_owned(),
        KeyCode::PageDown => "PageDown".to_owned(),
        KeyCode::Up => "↑".to_owned(),
        KeyCode::Down => "↓".to_owned(),
        KeyCode::Left => "←".to_owned(),
        KeyCode::Right => "→".to_owned(),
        KeyCode::F(n) => format!("F{n}"),
        // Aliases are canonicalized away in Binding::new; unreachable in
        // practice but keep the formatter total.
        KeyCode::Enter => "Enter".to_owned(),
        KeyCode::Tab => "Tab".to_owned(),
        KeyCode::Space => "Space".to_owned(),
    };
    match style {
        LabelStyle::Apple => {
            let mut label = String::new();
            if mods.ctrl {
                label.push('⌃');
            }
            if mods.alt {
                label.push('⌥');
            }
            if mods.shift {
                label.push('⇧');
            }
            if mods.super_ {
                label.push('⌘');
            }
            label.push_str(&name);
            label
        }
        LabelStyle::Other => {
            let mut label = String::new();
            for (set, name) in [
                (mods.ctrl, "Ctrl+"),
                (mods.alt, "Alt+"),
                (mods.shift, "Shift+"),
                (mods.super_, "Super+"),
            ] {
                if set {
                    label.push_str(name);
                }
            }
            label.push_str(&name);
            label
        }
    }
}

/// Keep a shared Shift visible once for single-letter alternatives without
/// widening every help row. Mixed modifiers and named keys stay explicit.
pub(crate) fn join_key_labels(labels: &[String], style: LabelStyle, separator: &str) -> String {
    let shift = match style {
        LabelStyle::Apple => "⇧",
        LabelStyle::Other => "Shift+",
    };
    let letters = labels
        .iter()
        .map(|label| {
            label.strip_prefix(shift).filter(|key| {
                key.len() == 1 && key.chars().all(|letter| letter.is_ascii_uppercase())
            })
        })
        .collect::<Option<Vec<_>>>();
    if labels.len() > 1
        && let Some(letters) = letters
    {
        return format!("{shift}{}", letters.join(separator));
    }
    labels.join(separator)
}

#[cfg(test)]
mod tests;
