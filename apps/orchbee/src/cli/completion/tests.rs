#[cfg(windows)]
pub(in crate::cli::completion) mod cases;
use super::render;
use crate::invocation::CompletionShell;

/// fish's `test` misreads a leading-dash operand inside a compound
/// `-a`/`-o` expression as an operator (witnessed on fish 4.2.1:
/// `test "$value" = format -a "$word" != text` fails once `$word` is
/// `-c`). `$word` is the only variable that can hold a typed word, so it
/// must never appear inside a compound `test`; `-a`/`-o` compounds over
/// fixed 0/1 `seen_*` operands are valid and remain.
#[test]
fn fish_compound_tests_have_no_user_word_operands() {
    let script = render(CompletionShell::Fish);
    for line in script.lines() {
        for segment in line.split(';') {
            if !segment.contains("test ") || !(segment.contains(" -a ") || segment.contains(" -o "))
            {
                continue;
            }
            assert!(
                !segment.contains("$word"),
                "compound test carries typed-word operand: {line}"
            );
        }
    }
}
