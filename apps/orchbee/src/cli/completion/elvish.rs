//! Elvish literal-word completion adapter. The editor owns matching and quoting.

#[cfg(test)]
mod tests;

/// The script for `name`, the running executable's stem, which it registers with and without
/// `.exe`; `name` uses only the portable command spelling, so single quotes carry it unchanged.
pub(super) fn render(name: &str) -> String {
    let grammar = super::grammar::build().to_string().replace('\'', "''");
    include_str!("elvish.elv")
        .replace("__ORCHBEE_COMMAND__", name)
        .replace("__ORCHBEE_GRAMMAR__", &grammar)
}
