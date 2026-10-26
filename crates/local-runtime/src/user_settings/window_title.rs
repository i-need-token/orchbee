//! Windows title-template grammar and bounded rendering shared by config and Server.
pub const DEFAULT_WINDOW_TITLE: &str = "{hostname}: {workspace}";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowTitleTemplate(Vec<Part>);
#[derive(Clone, Debug, Eq, PartialEq)]
enum Part {
    Literal(String),
    Token(usize),
}
impl WindowTitleTemplate {
    pub fn parse(value: &str) -> Result<Option<Self>, &'static str> {
        if value.len() > 4096 {
            return Err("value-too-long");
        }
        if value.is_empty() {
            return Ok(None);
        }
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut chars = value.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '{' | '}' if chars.peek() == Some(&ch) => {
                    chars.next();
                    literal.push(ch);
                }
                '{' => {
                    let mut name = String::new();
                    let mut closed = false;
                    for ch in chars.by_ref() {
                        if ch == '}' {
                            closed = true;
                            break;
                        }
                        name.push(ch);
                    }
                    if !closed {
                        return Err("unclosed-brace");
                    }
                    let token = match name.trim() {
                        "hostname" => 0,
                        "workspace" => 1,
                        "tab" => 2,
                        "pane" => 3,
                        "terminal_title" => 4,
                        _ => return Err("unknown-token"),
                    };
                    if !literal.is_empty() {
                        parts.push(Part::Literal(std::mem::take(&mut literal)));
                    }
                    parts.push(Part::Token(token));
                }
                '}' => return Err("unmatched-brace"),
                ch => literal.push(ch),
            }
        }
        if !literal.is_empty() {
            parts.push(Part::Literal(literal));
        }
        Ok(Some(Self(parts)))
    }
    /// Values are hostname, Workspace, Tab, manual Pane name and stripped terminal title.
    pub fn render(&self, values: [&str; 5]) -> Option<String> {
        let rendered: String = self
            .0
            .iter()
            .flat_map(|part| {
                match part {
                    Part::Literal(value) => value.as_str(),
                    Part::Token(index) => values[*index],
                }
                .chars()
            })
            .filter(|ch| !ch.is_control())
            .take(200)
            .collect();
        let rendered = rendered.trim();
        (!rendered.is_empty()).then(|| rendered.into())
    }
}
#[cfg(test)]
mod tests;
