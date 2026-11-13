use super::*;
#[test]
fn slugs_and_repository_join_keep_destination_inside_root() {
    for (branch, expected) in [
        ("worktree/brave-river", "worktree-brave-river"),
        ("issue/137 Worktree Spaces", "issue-137-worktree-spaces"),
        ("///", "worktree"),
        ("../A__B/", "a-b"),
        ("中文", "worktree"),
    ] {
        assert_eq!(slug(branch), expected);
    }
    // The destination joins with the host's path grammar.
    #[cfg(windows)]
    let (root, repository, expected) = (
        r"C:\configured",
        r"D:\Projects\repo",
        r"C:\configured\repo\feature-one",
    );
    #[cfg(not(windows))]
    let (root, repository, expected) = (
        "/configured",
        "/projects/repo",
        "/configured/repo/feature-one",
    );
    let root = WorkbenchDirectoryPath::parse(root).unwrap();
    let repository = WorkbenchDirectoryPath::parse(repository).unwrap();
    assert_eq!(
        Path::new(
            destination(&root, &repository, "Feature/One")
                .unwrap()
                .as_str()
        ),
        Path::new(expected)
    );
}
