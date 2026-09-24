//! PowerShell literal-AST completion adapter.

#[cfg(all(test, windows))]
mod tests;

pub(super) fn render() -> String {
    include_str!("powershell.ps1")
        .replace("__ORCHBEE_GRAMMAR__", &super::grammar::build().to_string())
}
