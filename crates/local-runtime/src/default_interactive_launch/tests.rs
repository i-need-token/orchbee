use super::{
    DefaultInteractiveLaunchError, UsableShellCandidates, select_default_interactive_launch,
};

const FALLBACK_SHELL: &str = "/bin/sh";

#[test]
fn selection_exhaustively_prefers_inherited_then_account_then_fallback() {
    for inherited_usable in [false, true] {
        for account_usable in [false, true] {
            for fallback_usable in [false, true] {
                let result = select_default_interactive_launch(UsableShellCandidates {
                    inherited: inherited_usable.then(|| "/shell/inherited".to_owned()),
                    account: account_usable.then(|| "/shell/account".to_owned()),
                    fallback: fallback_usable.then(|| FALLBACK_SHELL.to_owned()),
                });

                let expected = if inherited_usable {
                    Some("/shell/inherited")
                } else if account_usable {
                    Some("/shell/account")
                } else if fallback_usable {
                    Some(FALLBACK_SHELL)
                } else {
                    None
                };
                match expected {
                    Some(expected) => assert_eq!(
                        result.map(|launch| launch.executable),
                        Ok(expected.to_owned())
                    ),
                    None => assert_eq!(result, Err(DefaultInteractiveLaunchError::NoUsableShell)),
                }
            }
        }
    }
}

#[test]
fn absent_candidates_preserve_the_same_priority_and_typed_failure() {
    let account = select_default_interactive_launch(UsableShellCandidates {
        account: Some("/shell/account".to_owned()),
        ..UsableShellCandidates::default()
    });
    assert_eq!(
        account.map(|launch| launch.executable),
        Ok("/shell/account".to_owned())
    );

    let fallback = select_default_interactive_launch(UsableShellCandidates {
        fallback: Some(FALLBACK_SHELL.to_owned()),
        ..UsableShellCandidates::default()
    });
    assert_eq!(
        fallback.map(|launch| launch.executable),
        Ok(FALLBACK_SHELL.to_owned())
    );

    assert_eq!(
        select_default_interactive_launch(UsableShellCandidates::default()),
        Err(DefaultInteractiveLaunchError::NoUsableShell)
    );
}

#[test]
fn launch_facts_are_empty_literal_arguments_and_only_the_fixed_environment() {
    let launch = select_default_interactive_launch(UsableShellCandidates {
        inherited: Some("/shell/inherited".to_owned()),
        ..UsableShellCandidates::default()
    })
    .expect("the controlled candidate observation includes the inherited shell");

    assert!(launch.arguments.is_empty());
    assert_eq!(
        launch.environment.map(|entry| (entry.name, entry.value)),
        [
            ("TERM".to_owned(), "xterm-256color".to_owned()),
            ("COLORTERM".to_owned(), "truecolor".to_owned()),
            ("TERM_PROGRAM".to_owned(), "ghostty".to_owned()),
        ]
    );
}

/// A launch environment without a character locale gains the host's UTF-8 locale; one that names a
/// locale in any of the three variables keeps it, and a host without the UTF-8 locale adds nothing.
#[cfg(unix)]
#[test]
fn only_a_launch_without_a_character_locale_gains_the_utf8_locale() {
    use super::{UTF8_LOCALE, add_character_locale};
    let (locale, _) = UTF8_LOCALE;
    let from = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    };
    let added = add_character_locale(from(&[("HOME", "/h"), ("LANG", "")]), |_| true)
        .map(|entry| (entry.name, entry.value));
    assert_eq!(added, Some(("LANG".to_owned(), locale.to_owned())));
    for named in [
        &[("LC_ALL", "zh_CN.UTF-8")][..],
        &[("LC_CTYPE", "UTF-8")],
        &[("LANG", "en_GB.UTF-8")],
        &[("LANG", "C")],
    ] {
        let pairs: &'static [(&'static str, &'static str)] = named;
        assert_eq!(
            add_character_locale(from(pairs), |_| true),
            None,
            "{pairs:?}"
        );
    }
    assert_eq!(add_character_locale(from(&[]), |_| false), None);
}
