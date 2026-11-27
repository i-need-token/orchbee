//! Locale negotiation from a language tag, and the failure messages under both locales.
use super::{Locale, locale};

#[test]
fn any_chinese_tag_selects_zh_cn_and_everything_else_en_us() {
    for (tag, expected) in [
        (Some("zh-CN"), Locale::ZhCn),
        (Some("zh-Hans-CN"), Locale::ZhCn),
        (Some("zh-TW"), Locale::ZhCn),
        (Some("zh-Hant-HK"), Locale::ZhCn),
        (Some("ZH_cn.UTF-8"), Locale::ZhCn),
        (Some("en-US"), Locale::EnUs),
        (Some("fr-FR"), Locale::EnUs),
        (Some("z"), Locale::EnUs),
        (Some(""), Locale::EnUs),
        (None, Locale::EnUs),
    ] {
        assert_eq!(locale(tag), expected, "{tag:?}");
    }
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
fn every_failure_names_its_path_in_both_locales() {
    use super::failure;
    use crate::decision::Failure;
    use std::path::PathBuf;

    let path = PathBuf::from(r"C:\Store\desktop\versions\0.2.0\orchbee-desktop.exe");
    for message in [
        Failure::NoActiveVersion(path.clone()),
        Failure::ClientUnavailable(path.clone()),
    ] {
        let english = failure(Locale::EnUs, &message);
        let chinese = failure(Locale::ZhCn, &message);
        for text in [&english, &chinese] {
            assert!(text.contains(&*path.to_string_lossy()), "{text}");
        }
        assert_ne!(english, chinese);
    }
}
