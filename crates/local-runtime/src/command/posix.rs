//! Literal POSIX command lines: text that zsh, bash and `sh` read back as exactly the given words.

/// `program` and `args` as one line that zsh, bash and `sh` read back as exactly those words, for
/// a line typed into an interactive shell. The line runs as typed, so the person's own alias or
/// function of the program's name runs, as it would for a line they typed. Nothing when a word
/// holds a control character, which a typed line cannot carry.
#[must_use]
pub fn posix_command_line(program: &str, args: &[String]) -> Option<String> {
    let mut line = posix_shell_word(program)?;
    for arg in args {
        line.push(' ');
        line.push_str(&posix_shell_word(arg)?);
    }
    Some(line)
}

/// One word as zsh, bash and `sh` read it back literally. A word of only
/// `[A-Za-z0-9._/:,+@%=-]` stays as it is, except a leading `=`, which zsh expands to a command's
/// path; any other word is single-quoted, with each `'` written as `'\''`. Nothing when the word
/// holds a control character.
#[must_use]
pub fn posix_shell_word(word: &str) -> Option<String> {
    if word.chars().any(char::is_control) {
        return None;
    }
    let plain = !word.is_empty()
        && !word.starts_with('=')
        && word.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'.' | b'_' | b'/' | b':' | b',' | b'+' | b'@' | b'%' | b'=' | b'-'
                )
        });
    Some(if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    })
}

#[cfg(test)]
mod tests;
