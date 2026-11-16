use super::*;

// `ORCHBEE_HOME` is a new input boundary, and tests rely on an injected environment never reaching
// the process's own home.
#[test]
fn only_the_supplied_environment_selects_the_root() {
    #[cfg(windows)]
    let (home, chosen, relative) = (r"C:\Users\person", r"D:\orchbee data", r"relative\root");
    #[cfg(not(windows))]
    let (home, chosen, relative) = ("/home/person", "/srv/orchbee data", "relative/root");
    let environment = |home: Option<&'static str>, chosen: Option<&'static str>| {
        move |name: &str| {
            if name == HOME_SOURCE {
                home.map(OsString::from)
            } else if name == HOME_VARIABLE {
                chosen.map(OsString::from)
            } else {
                None
            }
        }
    };

    let default = DataRoot::resolve(environment(Some(home), None)).unwrap();
    assert_eq!(
        default.path(),
        Path::new(home).join(format!(".{}", crate::environment_stem()))
    );
    assert_eq!(
        DataRoot::variable(environment(Some(home), None)),
        HOME_SOURCE
    );

    let moved = environment(Some(home), Some(chosen));
    assert_eq!(DataRoot::resolve(moved).unwrap().path(), Path::new(chosen));
    assert_eq!(DataRoot::variable(moved), HOME_VARIABLE);

    for value in ["", relative] {
        assert_eq!(
            DataRoot::resolve(environment(Some(home), Some(value))),
            Err(DataRootError {
                variable: HOME_VARIABLE,
                reason: "invalid-path"
            })
        );
    }
    assert_eq!(
        DataRoot::resolve(environment(None, None)),
        Err(DataRootError {
            variable: HOME_SOURCE,
            reason: "missing"
        })
    );
}

// Below a home folder that lets another account in, a new data root and the empty one an earlier
// start left there are made private, while one holding anything is refused unchanged.
#[cfg(windows)]
#[test]
fn a_data_root_below_a_shared_home_is_made_private_unless_it_holds_anything() {
    use crate::{AdmissionRefusal, RefusalReason, private_filesystem as permissions};
    let home = crate::tests::SharedFolder::new();
    let create = |root: &Path| platform::create_below_roots(root, &[root.to_owned()]);
    let fresh = home.0.join(".fresh");
    permissions::require_private(create(&fresh).unwrap().last().unwrap()).unwrap();

    let empty = home.0.join(".empty");
    std::fs::create_dir(&empty).unwrap();
    permissions::require_private(create(&empty).unwrap().last().unwrap()).unwrap();

    let used = home.0.join(".used");
    std::fs::create_dir(&used).unwrap();
    std::fs::write(used.join("config.toml"), b"").unwrap();
    let refused = create(&used).unwrap_err();
    assert_eq!(
        AdmissionRefusal::of(&refused),
        Some(&AdmissionRefusal::new(RefusalReason::OthersCanRead, &used))
    );
    assert!(
        permissions::require_private(&permissions::open_directory(&used, false).unwrap()).is_err()
    );
}

// `newestVersion` only rises, so its comparison decides whether a newer build ever rewrites the
// record; an unreadable version must never win.
#[test]
fn product_versions_follow_semantic_version_precedence() {
    use version_record::newer_product_version as newer;
    let ascending = [
        "0.9.0",
        "0.10.0-alpha",
        "0.10.0-alpha.1",
        "0.10.0-alpha.beta",
        "0.10.0-beta.2",
        "0.10.0-beta.11",
        "0.10.0",
        "1.0.0",
    ];
    for (index, lower) in ascending.iter().enumerate() {
        for higher in &ascending[index + 1..] {
            assert!(newer(higher, lower), "{higher} > {lower}");
            assert!(!newer(lower, higher), "{lower} < {higher}");
        }
        assert!(!newer(lower, lower));
    }
    assert!(!newer("1.0.0+build", "1.0.0"));
    for unreadable in ["", "1", "1.0", "1.0.0.0", "1..0", "v1.0.0", "1.0.0-"] {
        assert!(!newer(unreadable, "0.0.1"), "{unreadable}");
        assert!(!newer("9.0.0", unreadable), "{unreadable}");
    }
}
