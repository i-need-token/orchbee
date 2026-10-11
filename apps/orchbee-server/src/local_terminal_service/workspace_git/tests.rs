use super::*;
fn oid() -> String {
    "a".repeat(40)
}
#[test]
fn branch_counts_are_exact_local_upstream_values_and_absence_is_not_zero() {
    let data = format!(
        "# branch.oid {}\n# branch.head worktree/测试\n# branch.upstream origin/main\n# branch.ab +12 -3\n",
        oid()
    );
    assert_eq!(
        parse_status(data.as_bytes()),
        Ok((
            Some("worktree/测试".into()),
            Some(GitDivergence {
                ahead: 12,
                behind: 3
            })
        ))
    );
    let clean = data.replace("+12 -3", "+0 -0");
    assert_eq!(
        parse_status(clean.as_bytes()).unwrap().1,
        Some(GitDivergence {
            ahead: 0,
            behind: 0
        })
    );
    let missing = format!("# branch.oid {}\n# branch.head main\n", oid());
    assert_eq!(
        parse_status(missing.as_bytes()),
        Ok((Some("main".into()), None))
    );
}
#[test]
fn detached_unborn_and_malformed_statuses_do_not_fabricate_branch_or_counts() {
    assert_eq!(
        parse_status(format!("# branch.oid {}\n# branch.head (detached)\n", oid()).as_bytes()),
        Ok((None, None))
    );
    assert_eq!(
        parse_status(b"# branch.oid (initial)\n# branch.head first\n"),
        Ok((Some("first".into()), None))
    );
    let prefix = format!(
        "# branch.oid {}\n# branch.head main\n# branch.upstream origin/main\n",
        oid()
    );
    for suffix in [
        "# branch.ab +01 -0\n",
        "# branch.ab +4294967296 -0\n",
        "# branch.ab +1 -2 extra\n",
        "# branch.ab +1 -2\n# branch.ab +1 -2\n",
        "# branch.head duplicate\n",
        "1 .M N... arbitrary file\n",
        "# branch.ab +1 -2",
    ] {
        assert!(
            parse_status(format!("{prefix}{suffix}").as_bytes()).is_err(),
            "{suffix}"
        );
    }
    assert!(
        parse_status(b"# branch.oid (initial)\n# branch.head first\n# branch.ab +0 -0\n").is_err()
    );
    assert!(parse_status(&[0xff, b'\n']).is_err());
}
