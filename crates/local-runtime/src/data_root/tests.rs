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
