//! Fish's adapter keeps literal words in lists and never evaluates input.
pub(super) fn render() -> String {
    let mut script = String::from(
        "# Static Orchbee completion: no runtime or filesystem probing.\nfunction __orchbee_meta\n  switch $argv[1]\n",
    );
    for (key, values) in super::metadata::rows() {
        if values.is_empty() {
            continue;
        }
        script.push_str(&format!(
            "    case {}\n      printf '%s\\n' {}\n",
            super::metadata::quote(&key),
            values
                .iter()
                .map(|value| super::metadata::quote(value))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    script.push_str("  end\nend\n");
    script.push_str(include_str!("fish.fish"));
    script
}
