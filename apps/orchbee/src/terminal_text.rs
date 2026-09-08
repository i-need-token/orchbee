use std::borrow::Cow;

const REPLACEMENT_CHARACTER: char = '\u{fffd}';

pub(crate) fn is_forbidden_terminal_scalar(character: char) -> bool {
    character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
}

pub(crate) fn sanitize_terminal_text(value: &str) -> Cow<'_, str> {
    if !value.chars().any(is_forbidden_terminal_scalar) {
        return Cow::Borrowed(value);
    }

    Cow::Owned(
        value
            .chars()
            .map(|character| {
                if is_forbidden_terminal_scalar(character) {
                    REPLACEMENT_CHARACTER
                } else {
                    character
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
