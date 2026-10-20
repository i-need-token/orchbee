//! Lossless compact identities. Membership changes never alter printed handles.
pub(super) fn stable(kind: &str, ids: &[&str]) -> Vec<String> {
    ids.iter()
        .map(|id| format!("@{kind}:0x{}", id.trim_start_matches('0')))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_independent_of_peers_and_preserve_all_significant_digits() {
        let ids = [
            format!("{:032x}", 1),
            format!("{:032x}", 0x91),
            format!("{:032x}", u128::MAX),
        ];
        for kind in ["project", "workspace", "tab", "panel", "terminal"] {
            let together = stable(kind, &ids.iter().map(String::as_str).collect::<Vec<_>>());
            for (index, id) in ids.iter().enumerate() {
                assert_eq!(together[index], stable(kind, &[id])[0]);
            }
            assert_eq!(together[0], format!("@{kind}:0x1"));
            assert_eq!(together[1], format!("@{kind}:0x91"));
            assert_eq!(together[2], format!("@{kind}:0x{}", "f".repeat(32)));
        }
    }
}
