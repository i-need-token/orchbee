use super::*;
#[test]
fn pinned_herdr_slugs_and_repository_join_keep_destination_inside_root() {
    for (branch, expected) in [
        ("worktree/brave-river", "worktree-brave-river"),
        ("issue/137 Worktree Spaces", "issue-137-worktree-spaces"),
        ("///", "worktree"),
        ("../A__B/", "a-b"),
        ("中文", "worktree"),
    ] {
        assert_eq!(slug(branch), expected);
    }
    let root = WorkbenchDirectoryPath::parse(r"C:\configured").unwrap();
    let repository = WorkbenchDirectoryPath::parse(r"D:\Projects\repo").unwrap();
    assert_eq!(
        Path::new(
            destination(&root, &repository, "Feature/One")
                .unwrap()
                .as_str()
        ),
        Path::new(r"C:\configured\repo\feature-one")
    );
}
