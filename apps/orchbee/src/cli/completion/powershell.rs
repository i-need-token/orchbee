//! PowerShell literal-AST completion adapter.

#[cfg(all(test, windows))]
mod tests;

pub(super) fn render() -> String {
    include_str!("powershell.ps1")
        .replace("__ORCHBEE_GRAMMAR__", &super::grammar::build().to_string())
        .replace(
            "__ORCHBEE_DEFAULT_CONFIG_PS__",
            if cfg!(windows) {
                ", '--default-config'"
            } else {
                ""
            },
        )
        .replace(
            "__ORCHBEE_DEFAULT_CONFIG_OFFER_PS__",
            if cfg!(windows) {
                "if ($seen.Count -eq 0) { $candidates.Add('--default-config') }"
            } else {
                ""
            },
        )
}
