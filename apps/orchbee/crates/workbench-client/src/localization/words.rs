//! One message in every shipped locale, for the words a feature declares beside its own values.
use super::Locale;

/// One message in every shipped locale. A new locale adds a field here, so no declaration
/// compiles until it carries the new text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Words {
    en_us: &'static str,
    zh_cn: &'static str,
}

/// The same message in `en-US` and `zh-CN`; commands, identifiers and placeholders stay as
/// written in both.
pub(crate) const fn words(en_us: &'static str, zh_cn: &'static str) -> Words {
    Words { en_us, zh_cn }
}

impl Locale {
    pub(crate) const fn words(self, words: Words) -> &'static str {
        match self {
            Self::EnUs => words.en_us,
            Self::ZhCn => words.zh_cn,
        }
    }
}
