use std::ffi::OsString;

const EN_US_HELP: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee --help\n  orchbee --version\n\nModes:\n  orchbee            Select the interactive TUI (unavailable in this development slice)\n  orchbee --help     Show this help\n  orchbee --version  Show the version\n";
const ZH_CN_HELP: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee --help\n  orchbee --version\n\n模式：\n  orchbee            选择交互式 TUI（本开发切片尚未提供）\n  orchbee --help     显示此帮助\n  orchbee --version  显示版本\n";
const EN_US_TUI_UNAVAILABLE: &str = "The Orchbee TUI is unavailable in this development slice.\n";
const ZH_CN_TUI_UNAVAILABLE: &str = "Orchbee TUI 在本开发切片中尚不可用。\n";
const EN_US_USAGE_FAILURE: &str = "Invalid invocation. Run 'orchbee --help'.\n";
const ZH_CN_USAGE_FAILURE: &str = "调用方式无效。请运行 ‘orchbee --help’。\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    pub(crate) const fn help(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_HELP,
            Self::ZhCn => ZH_CN_HELP,
        }
    }

    pub(crate) const fn tui_unavailable(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_TUI_UNAVAILABLE,
            Self::ZhCn => ZH_CN_TUI_UNAVAILABLE,
        }
    }

    pub(crate) const fn usage_failure(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_USAGE_FAILURE,
            Self::ZhCn => ZH_CN_USAGE_FAILURE,
        }
    }
}

pub(crate) fn select_locale(mut read_environment: impl FnMut(&str) -> Option<OsString>) -> Locale {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = read_environment(name) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return locale_from_value(&value);
    }
    Locale::EnUs
}

fn locale_from_value(value: &OsString) -> Locale {
    let Some(value) = value.to_str() else {
        return Locale::EnUs;
    };
    let bytes = value.as_bytes();
    match (bytes.first(), bytes.get(1)) {
        (Some(first), Some(second))
            if first.eq_ignore_ascii_case(&b'z') && second.eq_ignore_ascii_case(&b'h') =>
        {
            Locale::ZhCn
        }
        _ => Locale::EnUs,
    }
}

#[cfg(test)]
mod tests;
