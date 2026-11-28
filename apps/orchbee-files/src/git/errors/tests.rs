//! Git's own messages classify as VS Code's patterns at ef5ca45 classify them, the failed command's
//! rule before the general one.
use super::*;

#[test]
fn git_messages_classify_as_vs_code_classifies_them() {
    use Classifier as K;
    use GitErrorCode as C;
    let push = K::Push(PushMode::None);
    let lease = K::Push(PushMode::Lease);
    let includes = K::Push(PushMode::LeaseIfIncludes);
    let cases: &[(Classifier, &str, &str, Option<GitErrorCode>)] = &[
        (
            K::General,
            "",
            "fatal: Unable to create '/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository",
            Some(C::RepositoryIsLocked),
        ),
        (
            K::General,
            "",
            "fatal: not a git repository (or any of the parent directories): .git",
            Some(C::NotAGitRepository),
        ),
        (
            K::General,
            "",
            "fatal: detected dubious ownership in repository at '/r'",
            Some(C::NotASafeGitRepository),
        ),
        (
            K::General,
            "",
            "error: The branch 'topic' is not fully merged.",
            Some(C::BranchNotFullyMerged),
        ),
        (
            K::General,
            "",
            "error: The branch '' is not fully merged.",
            None,
        ),
        (
            K::General,
            "",
            "fatal: 'a b' is not a valid branch name",
            Some(C::InvalidBranchName),
        ),
        (
            K::General,
            "",
            "fatal: '../wt' already exists",
            Some(C::WorktreeAlreadyExists),
        ),
        (
            K::Fetch,
            "",
            "fatal: unable to access 'https://h/r/': Could not resolve host: h",
            Some(C::CantAccessRemote),
        ),
        (
            K::Fetch,
            "",
            "fatal: Could not read from remote repository.",
            Some(C::RemoteConnectionError),
        ),
        (
            K::Fetch,
            "",
            " ! [rejected]        main       -> main  (non-fast-forward)",
            Some(C::BranchFastForwardRejected),
        ),
        (
            K::Pull,
            "Auto-merging a\nCONFLICT (content): Merge conflict in a\n",
            "",
            Some(C::Conflict),
        ),
        (K::Pull, "CONFLICT (content):  a\n", "", None),
        (
            K::Pull,
            "",
            "error: Your local changes to the following files would be overwritten by merge:",
            Some(C::DirtyWorkTree),
        ),
        (
            K::Pull,
            "",
            "error: cannot lock ref 'refs/remotes/o/m': is at 1 but expected 2",
            Some(C::CantLockRef),
        ),
        (
            K::Pull,
            "",
            " ! [rejected]        v1         -> v1  (would clobber existing tag)",
            Some(C::TagConflict),
        ),
        (
            push,
            "",
            " ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs to 'h:r'",
            Some(C::PushRejected),
        ),
        (
            lease,
            "",
            " ! [rejected]        main -> main (stale info)\nerror: failed to push some refs to 'h:r'",
            Some(C::ForcePushWithLeaseRejected),
        ),
        (
            includes,
            "",
            " ! [rejected]        main -> main (stale info)\nerror: failed to push some refs to 'h:r'",
            Some(C::PushRejected),
        ),
        (
            includes,
            "",
            " ! [rejected]        main -> main (remote ref updated since checkout)\nerror: failed to push some refs to 'h:r'",
            Some(C::ForcePushWithLeaseIfIncludesRejected),
        ),
        (
            push,
            "",
            "git@h: Permission denied (publickey).\nfatal: Could not read from remote repository.",
            Some(C::PermissionDenied),
        ),
        (
            push,
            "",
            "fatal: The current branch topic has no upstream branch.",
            Some(C::NoUpstreamBranch),
        ),
        (
            push,
            "",
            "hint: x\nfatal: The current branch topic has no upstream branch.",
            None,
        ),
        (
            K::Commit,
            "",
            "error: Committing is not possible because you have unmerged files.",
            Some(C::UnmergedChanges),
        ),
        (
            K::Commit,
            "",
            "Aborting commit due to empty commit message.",
            Some(C::EmptyCommitMessage),
        ),
        (
            K::Checkout,
            "",
            "error: Your local changes would be overwritten by checkout:\nPlease commit your changes or stash them before you switch branches.",
            Some(C::DirtyWorkTree),
        ),
        // The command's own rule wins over a general one the same message also meets.
        (
            K::Fetch,
            "",
            "fatal: unable to access 'h': Authentication failed\nfatal: Could not read from remote repository.",
            Some(C::RemoteConnectionError),
        ),
        // Both spellings of a branch that exists, and of a missing stash.
        (
            K::Branch,
            "",
            "fatal: A branch named 'topic' already exists.",
            Some(C::BranchAlreadyExists),
        ),
        (K::Stash, "", "No stash found.", Some(C::NoStashFound)),
        // A pick's failure reads as a conflict, except another process's lock, which is retried.
        (
            K::CherryPick,
            "",
            "error: could not apply 1a2b3c4... topic",
            Some(C::CherryPickConflict),
        ),
        (
            K::CherryPick,
            "",
            "fatal: Unable to create '/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository",
            Some(C::RepositoryIsLocked),
        ),
    ];
    for (classifier, stdout, stderr, expected) in cases {
        assert_eq!(
            classify(*classifier, stdout.as_bytes(), stderr.as_bytes()),
            *expected,
            "{classifier:?} {stderr:?}"
        );
    }
}

#[test]
fn details_keep_the_end_of_standard_error_on_a_character_boundary() {
    let long = format!("{}é{}", "x".repeat(10), "y".repeat(DETAIL_LIMIT - 1));
    let kept = detail(long.as_bytes()).unwrap();
    assert!(kept.len() <= DETAIL_LIMIT);
    assert!(kept.ends_with('y') && !kept.contains('x'));
    assert_eq!(detail(b" \n"), None);
}
