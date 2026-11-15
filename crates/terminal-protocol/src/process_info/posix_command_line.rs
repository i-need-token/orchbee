//! Decodes the `commandLine` a POSIX Terminal Host reports: its argument vector with each argument
//! quoted as Python's `shlex.quote` quotes it, joined by one space. The decoder splits words as a
//! POSIX shell does, limited to quoting that expands nothing, so text the Host's quoting cannot
//! produce, such as an escape, a variable or an operator, is refused rather than guessed at.

/// The arguments a POSIX Host joined into `line`, or `None` when `line` is not in that form.
///
/// Words are separated by exactly one space. A word joins bare characters from the quoting's safe
/// set (ASCII letters, digits and `_@%+=:,./-`), single-quoted text, and double-quoted text without
/// `"`, `\`, `$` or `` ` ``, so the `'"'"'` that writes a quote inside a quoted argument decodes and
/// `''` is one empty argument. An empty line, a leading, trailing or doubled space, an unclosed
/// quote, and any other character outside quotes decode to `None`.
#[must_use]
pub fn split_posix_command_line(line: &str) -> Option<Vec<String>> {
    let mut arguments = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        match character {
            ' ' if in_word => {
                arguments.push(std::mem::take(&mut word));
                in_word = false;
            }
            '\'' => {
                in_word = true;
                loop {
                    match characters.next()? {
                        '\'' => break,
                        quoted => word.push(quoted),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match characters.next()? {
                        '"' => break,
                        '\\' | '$' | '`' => return None,
                        quoted => word.push(quoted),
                    }
                }
            }
            bare if is_safe(bare) => {
                in_word = true;
                word.push(bare);
            }
            _ => return None,
        }
    }
    in_word.then(|| {
        arguments.push(word);
        arguments
    })
}

/// The characters `shlex.quote` leaves bare.
const fn is_safe(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-'
        )
}
