//! Plain-text help documents and their shared display-cell layout. Authored copy
//! carries meaning, while this projection owns spacing, columns and wrapping;
//! rendering never queries a terminal or reads configuration or environment.

use unicode_segmentation::UnicodeSegmentation;

pub(crate) const WIDTH: usize = 78;
const MAX_LABEL_WIDTH: usize = 24;

/// One help page, independent of terminal state and output transport.
pub(crate) struct Document {
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) sections: Vec<Section>,
    pub(crate) footer: Vec<String>,
}

/// Semantic sections share one layout instead of embedding alignment in copy.
pub(crate) enum Section {
    Syntax {
        heading: String,
        lines: Vec<String>,
    },
    Rows {
        heading: String,
        rows: Vec<Row>,
    },
    Paragraphs {
        heading: String,
        paragraphs: Vec<String>,
    },
    Examples {
        heading: String,
        examples: Vec<Row>,
    },
}

/// A command, option or example paired with its localized purpose.
pub(crate) struct Row {
    pub(crate) label: String,
    pub(crate) description: String,
}

/// Render a stable offline document within the shared 78-cell layout.
pub(crate) fn render(document: &Document) -> String {
    render_at_width(document, WIDTH)
}

fn render_at_width(document: &Document, width: usize) -> String {
    let mut output = String::new();
    flow(&mut output, &document.title, "", "", width);
    if !document.summary.is_empty() {
        flow(&mut output, &document.summary, "", "", width);
    }
    let label_width = document
        .sections
        .iter()
        .filter_map(|section| match section {
            Section::Rows { rows, .. } => Some(rows),
            _ => None,
        })
        .flatten()
        .map(|row| cells(&row.label))
        .max()
        .unwrap_or(0)
        .min(MAX_LABEL_WIDTH)
        .min(width.saturating_sub(4) / 2);
    for section in &document.sections {
        output.push('\n');
        match section {
            Section::Syntax { heading, lines } => {
                heading_line(&mut output, heading, width);
                for line in lines {
                    flow(&mut output, line, "  ", "    ", width);
                }
            }
            Section::Rows { heading, rows } => {
                heading_line(&mut output, heading, width);
                for row in rows {
                    if cells(&row.label) > label_width {
                        flow(&mut output, &row.label, "  ", "    ", width);
                        flow(&mut output, &row.description, "    ", "    ", width);
                    } else {
                        let prefix = format!(
                            "  {}{}",
                            row.label,
                            " ".repeat(label_width - cells(&row.label) + 2)
                        );
                        flow(
                            &mut output,
                            &row.description,
                            &prefix,
                            &" ".repeat(label_width + 4),
                            width,
                        );
                    }
                }
            }
            Section::Paragraphs {
                heading,
                paragraphs,
            } => {
                heading_line(&mut output, heading, width);
                for (index, paragraph) in paragraphs.iter().enumerate() {
                    if index > 0 {
                        output.push('\n');
                    }
                    flow(&mut output, paragraph, "  ", "  ", width);
                }
            }
            Section::Examples { heading, examples } => {
                heading_line(&mut output, heading, width);
                for example in examples {
                    for (index, line) in example.label.lines().enumerate() {
                        let indent = if index == 0 { "  " } else { "    " };
                        flow(&mut output, line, indent, "    ", width);
                    }
                    flow(&mut output, &example.description, "    ", "    ", width);
                }
            }
        }
    }
    if !document.footer.is_empty() {
        output.push('\n');
        for paragraph in &document.footer {
            flow(&mut output, paragraph, "", "", width);
        }
    }
    output
}

fn heading_line(output: &mut String, heading: &str, width: usize) {
    if !heading.is_empty() {
        flow(output, &format!("{heading}:"), "", "", width);
    }
}

fn cells(text: &str) -> usize {
    crate::terminal_text::display_width(text)
}

struct Token {
    text: String,
    space: bool,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut word = String::new();
    let mut word_space = false;
    let mut pending_space = false;
    for grapheme in text.graphemes(true) {
        let whitespace = grapheme.chars().all(char::is_whitespace);
        let wide = cells(grapheme) > 1;
        if whitespace || wide {
            if !word.is_empty() {
                tokens.push(Token {
                    text: std::mem::take(&mut word),
                    space: word_space,
                });
            }
            if whitespace {
                pending_space = true;
            } else if !pending_space
                && "，。！？：；、）》】”’".contains(grapheme)
                && let Some(previous) = tokens.last_mut()
            {
                previous.text.push_str(grapheme);
            } else {
                tokens.push(Token {
                    text: grapheme.to_owned(),
                    space: pending_space,
                });
                pending_space = false;
            }
        } else {
            if word.is_empty() {
                word_space = pending_space;
                pending_space = false;
            }
            word.push_str(grapheme);
        }
    }
    if !word.is_empty() {
        tokens.push(Token {
            text: word,
            space: word_space,
        });
    }
    tokens
}

fn flow(output: &mut String, text: &str, first: &str, continuation: &str, width: usize) {
    let mut line = first.to_owned();
    let mut line_width = cells(first);
    let mut content = false;
    for token in tokens(text) {
        let token_width = cells(&token.text);
        let space = content && token.space;
        if line_width + usize::from(space) + token_width > width && content {
            output.push_str(line.trim_end());
            output.push('\n');
            line = continuation.to_owned();
            line_width = cells(continuation);
            content = false;
        }
        if content && token.space {
            line.push(' ');
            line_width += 1;
        }
        for grapheme in token.text.graphemes(true) {
            let grapheme_width = cells(grapheme);
            if line_width + grapheme_width > width && content {
                output.push_str(line.trim_end());
                output.push('\n');
                line = continuation.to_owned();
                line_width = cells(continuation);
            }
            line.push_str(grapheme);
            line_width += grapheme_width;
            content = true;
        }
    }
    if content || !first.trim().is_empty() {
        output.push_str(line.trim_end());
        output.push('\n');
    }
}

#[cfg(test)]
mod tests;
