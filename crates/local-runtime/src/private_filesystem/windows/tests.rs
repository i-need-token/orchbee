//! The private rule over owner and access-list inputs as data. A real elevated token cannot be
//! made in a test, so these cases stand in for the owners an ordinary session, an elevated session
//! and an installer give a folder, and for the accounts that must never be let in.
use super::{Entry, admit};
use crate::RefusalReason;

const ACCOUNT: &str = "S-1-5-21-1-2-3-1001";
const SYSTEM: &str = "S-1-5-18";
const ADMINISTRATORS: &str = "S-1-5-32-544";
const OTHER_USER: &str = "S-1-5-21-1-2-3-1002";
const EVERYONE: &str = "S-1-1-0";
const USERS: &str = "S-1-5-32-545";
const AUTHENTICATED_USERS: &str = "S-1-5-11";

const TRUSTED: [&str; 3] = [ACCOUNT, SYSTEM, ADMINISTRATORS];

fn decide(owner: Option<&str>, access: Option<&[Entry<'_, str>]>) -> Result<(), RefusalReason> {
    admit(owner, access, &TRUSTED)
}

/// The access list `protect_new` writes: the account, SYSTEM and Administrators.
fn protected() -> Vec<Entry<'static, str>> {
    vec![
        Entry::Allow(ACCOUNT),
        Entry::Allow(SYSTEM),
        Entry::Allow(ADMINISTRATORS),
    ]
}

#[test]
fn the_account_system_or_administrators_may_own_a_private_object() {
    // The owner an ordinary session, an elevated session and an elevated installer each give.
    for owner in TRUSTED {
        assert_eq!(decide(Some(owner), Some(&protected())), Ok(()), "{owner}");
    }
}

#[test]
fn another_account_as_owner_is_refused_even_behind_a_private_access_list() {
    for owner in [OTHER_USER, USERS, EVERYONE, AUTHENTICATED_USERS] {
        assert_eq!(
            decide(Some(owner), Some(&protected())),
            Err(RefusalReason::OtherOwner),
            "{owner}"
        );
    }
    assert_eq!(
        decide(None, Some(&protected())),
        Err(RefusalReason::OtherOwner)
    );
}

#[test]
fn an_allow_entry_for_any_other_account_is_refused_whoever_owns_the_object() {
    for owner in TRUSTED {
        for intruder in [OTHER_USER, USERS, EVERYONE, AUTHENTICATED_USERS] {
            let mut access = protected();
            access.push(Entry::Allow(intruder));
            assert_eq!(
                decide(Some(owner), Some(&access)),
                Err(RefusalReason::OthersCanRead),
                "{owner} {intruder}"
            );
        }
    }
}

#[test]
fn a_missing_access_list_or_an_entry_of_another_kind_is_refused() {
    for owner in TRUSTED {
        assert_eq!(
            decide(Some(owner), None),
            Err(RefusalReason::OthersCanChange),
            "{owner}"
        );
        let mut access = protected();
        access.push(Entry::Other);
        assert_eq!(
            decide(Some(owner), Some(&access)),
            Err(RefusalReason::AccessControlList),
            "{owner}"
        );
    }
}

#[test]
fn deny_entries_only_take_access_away() {
    let access = [Entry::Deny, Entry::Allow(ACCOUNT), Entry::Deny];
    assert_eq!(decide(Some(ADMINISTRATORS), Some(&access)), Ok(()));
    assert_eq!(decide(Some(ACCOUNT), Some(&[])), Ok(()));
}
