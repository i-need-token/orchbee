//! PowerShell literal-AST completion adapter.

#[cfg(all(test, windows))]
mod tests;

/// The script for `name`, the running executable's stem, which it registers with and without
/// `.exe`; `name` uses only the portable command spelling, so single quotes carry it unchanged.
pub(super) fn render(name: &str) -> String {
    // Windows PowerShell 5.1 reads BOM-less scripts using the legacy code page.
    // ASCII JSON escapes preserve every declared Unicode character under either decoder.
    let mut literal = String::new();
    for character in super::grammar::build().to_string().chars() {
        if character.is_ascii() {
            literal.push(character);
        } else {
            for unit in character.encode_utf16(&mut [0; 2]) {
                literal.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    include_str!("powershell.ps1")
        .replace("__ORCHBEE_COMMAND__", name)
        .replace("__ORCHBEE_GRAMMAR__", &literal)
}
