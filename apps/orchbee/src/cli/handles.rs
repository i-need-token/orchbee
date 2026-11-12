//! Lossless compact identities. Membership changes never alter printed handles.
pub(super) fn stable(kind: &str, ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| compact(kind, id)).collect()
}

/// The handle a person types for the Panel with canonical ID `id`.
pub(crate) fn panel(id: &str) -> String {
    compact("panel", id)
}

/// The handle a person types for the Tab with canonical ID `id`.
pub(crate) fn tab(id: &str) -> String {
    compact("tab", id)
}

/// The handle a person types for the Workspace with canonical ID `id`.
pub(crate) fn workspace(id: &str) -> String {
    compact("workspace", id)
}

/// The handle a person types for the Project with canonical ID `id`.
pub(crate) fn project(id: &str) -> String {
    compact("project", id)
}

/// The handle a person types for the terminal with canonical ID `id`.
pub(crate) fn terminal(id: &str) -> String {
    compact("terminal", id)
}

fn compact(kind: &str, id: &str) -> String {
    format!("@{kind}:0x{}", id.trim_start_matches('0'))
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
