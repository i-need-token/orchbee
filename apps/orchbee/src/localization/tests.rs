use std::{collections::HashMap, ffi::OsString};

use super::{Locale, select_locale};

#[test]
fn uses_locale_precedence_and_skips_empty_values() {
    let empty_override = HashMap::from([
        ("LANG", OsString::from("zh_CN.UTF-8")),
        ("LC_MESSAGES", OsString::from("en_US.UTF-8")),
        ("LC_ALL", OsString::from("")),
    ]);
    assert_eq!(
        select_locale(|name| empty_override.get(name).cloned()),
        Locale::EnUs
    );

    let explicit_override = HashMap::from([
        ("LANG", OsString::from("en_US.UTF-8")),
        ("LC_MESSAGES", OsString::from("en_US.UTF-8")),
        ("LC_ALL", OsString::from("zh_CN.UTF-8")),
    ]);
    assert_eq!(
        select_locale(|name| explicit_override.get(name).cloned()),
        Locale::ZhCn
    );
}

#[test]
fn recognizes_chinese_prefix_case_insensitively() {
    for value in ["zh", "zh-CN", "zh_CN.UTF-8", "ZH_TW"] {
        assert_eq!(
            select_locale(|name| (name == "LANG").then(|| OsString::from(value))),
            Locale::ZhCn
        );
    }
}

#[test]
fn falls_back_to_english_for_absent_other_and_non_unicode_values() {
    assert_eq!(select_locale(|_name| None), Locale::EnUs);
    assert_eq!(
        select_locale(|name| (name == "LANG").then(|| OsString::from("fr_FR.UTF-8"))),
        Locale::EnUs
    );

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        assert_eq!(
            select_locale(|name| (name == "LANG").then(|| OsString::from_vec(vec![0xff]))),
            Locale::EnUs
        );
    }
}
