//! Help text routing: every topic renders a localized static document.

use crate::invocation::HelpTopic;
use crate::localization::Locale;

pub(crate) fn help(topic: HelpTopic, locale: Locale) -> &'static str {
    locale.cli_help(topic)
}
