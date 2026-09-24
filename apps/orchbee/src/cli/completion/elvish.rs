//! Elvish literal-word completion adapter. The editor owns matching and quoting.

#[cfg(test)]
mod tests;

pub(super) fn render() -> String {
    let grammar = super::grammar::build().to_string().replace('\'', "''");
    include_str!("elvish.elv").replace("__ORCHBEE_GRAMMAR__", &grammar)
}
