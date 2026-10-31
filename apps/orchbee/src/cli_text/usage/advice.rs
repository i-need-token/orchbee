//! Bounded spelling advice and command discovery over the admitted offline grammar.
//! This projection never changes parsing, resolves live resources, or executes a command.

use crate::invocation::{HelpTopic, UsageContext, UsageReason, help_paths};

/// Direct grammar children, in stable name order, for a missing command diagnostic.
pub(super) fn children(context: UsageContext) -> Vec<(&'static str, HelpTopic)> {
    let words = match context {
        UsageContext::Root => Vec::new(),
        UsageContext::Path(path) => path.split_whitespace().collect(),
        UsageContext::Literal(_) => return Vec::new(),
    };
    let mut children = help_paths()
        .iter()
        .filter(|path| path.words().len() == words.len() + 1 && path.words().starts_with(&words))
        .map(|path| (path.words()[words.len()], path.topic()))
        .collect::<Vec<_>>();
    children.sort_by_key(|(name, _)| *name);
    children
}

/// Common verbs this grammar spells differently, offered only as advice.
const SYNONYMS: [(&str, &str); 10] = [
    ("delete", "close"),
    ("remove", "close"),
    ("rm", "close"),
    ("kill", "close"),
    ("destroy", "close"),
    ("new", "create"),
    ("add", "create"),
    ("ls", "list"),
    ("show", "get"),
    ("info", "get"),
];

/// At most three close spellings from this failure's grammar scope, a known
/// synonym first, or the `--label` form of a stray label word.
/// Oversized and non-ASCII input produces no advice and cannot grow the work budget.
pub(super) fn suggestions(reason: &UsageReason) -> Vec<String> {
    if let UsageReason::UnexpectedArgument { context, value } = reason {
        let takes_label = crate::invocation::usage_option_candidates(*context).contains(&"--label");
        return if takes_label && !value.starts_with('-') && value.len() <= 128 {
            vec![format!(
                "--label {}",
                crate::cli_text::escape::escape(value)
            )]
        } else {
            Vec::new()
        };
    }
    let word = match reason {
        UsageReason::UnknownCommand { word, .. } => word.as_str(),
        UsageReason::UnknownOption { option, .. } => option.split('=').next().unwrap_or(option),
        _ => return Vec::new(),
    };
    if word.len() > 64 || !word.is_ascii() {
        return Vec::new();
    }
    if matches!(reason, UsageReason::UnknownOption { .. }) && is_single_letter_short_option(word) {
        return Vec::new();
    }
    let candidates = match reason {
        UsageReason::UnknownCommand { context, .. } => children(*context)
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        UsageReason::UnknownOption { context, .. } => {
            crate::invocation::usage_option_candidates(*context)
        }
        _ => unreachable!("reason was checked above"),
    };
    let synonym = matches!(reason, UsageReason::UnknownCommand { .. })
        .then(|| {
            SYNONYMS
                .iter()
                .find(|(spelling, _)| spelling.eq_ignore_ascii_case(word))
                .map(|(_, verb)| *verb)
                .filter(|verb| candidates.contains(verb))
        })
        .flatten();
    let threshold = if word.len() <= 3 { 1 } else { 2 };
    let mut ranked = candidates
        .into_iter()
        .map(|candidate| (edit_distance(word, candidate), candidate))
        .filter(|(distance, _)| *distance > 0 && *distance <= threshold)
        .collect::<Vec<_>>();
    ranked.sort_unstable();
    ranked.dedup();
    synonym
        .into_iter()
        .chain(
            ranked
                .into_iter()
                .map(|(_, candidate)| candidate)
                .filter(|candidate| Some(*candidate) != synonym),
        )
        .take(3)
        .map(str::to_owned)
        .collect()
}

fn is_single_letter_short_option(option: &str) -> bool {
    option
        .strip_prefix('-')
        .is_some_and(|letter| letter.len() == 1 && letter.as_bytes()[0].is_ascii_alphabetic())
}

fn edit_distance(word: &str, candidate: &str) -> usize {
    let mut previous = (0..=candidate.len()).collect::<Vec<_>>();
    let mut current = vec![0; candidate.len() + 1];
    for (row, left) in word.bytes().enumerate() {
        current[0] = row + 1;
        for (column, right) in candidate.bytes().enumerate() {
            current[column + 1] = (previous[column + 1] + 1)
                .min(current[column] + 1)
                .min(previous[column] + usize::from(left != right));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[candidate.len()]
}

#[cfg(test)]
mod tests;
