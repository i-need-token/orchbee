use super::wrap_str;
use crate::terminal_text::display_width;

fn lines(text: &str, width: usize) -> Vec<&str> {
    wrap_str(text, width)
        .into_iter()
        .map(|range| &text[range])
        .collect()
}

#[test]
fn chinese_prose_fills_the_line_instead_of_breaking_at_the_digit_space() {
    let text =
        "关闭此标签页将终止 1 个终端中运行的任务，并丢弃保留的输出，影响所有连接到此工作区的人。";
    for width in [48, 60, 72] {
        let wrapped = lines(text, width);
        assert!(wrapped.len() >= 2, "{width}: {wrapped:?}");
        assert!(
            display_width(wrapped[0]) >= width - 2,
            "{width}: {wrapped:?}"
        );
        for line in &wrapped {
            assert_eq!(line.trim(), *line, "{width}: {wrapped:?}");
            assert!(!line.starts_with(['，', '。']), "{width}: {wrapped:?}");
        }
    }
}

#[test]
fn closing_punctuation_never_starts_and_opening_never_ends_a_line() {
    assert_eq!(lines("一二三，四五", 6), ["一二", "三，四", "五"]);
    assert_eq!(lines("一二（三", 6), ["一二", "（三"]);
}

#[test]
fn latin_prose_breaks_only_at_spaces_and_splits_overlong_words() {
    assert_eq!(
        lines("Closing this tab ends 1 running terminal task.", 20),
        ["Closing this tab", "ends 1 running", "terminal task."]
    );
    assert_eq!(lines("abcdefghij", 4), ["abcd", "efgh", "ij"]);
}
