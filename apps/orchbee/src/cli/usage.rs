//! Bounded spelling advice and command discovery over the admitted offline grammar.
//! This projection never changes parsing, resolves live resources, or executes a command.

use crate::invocation::{HelpTopic, UsageContext, UsageReason, help_paths};

/// Direct grammar children, in stable name order, for a missing command diagnostic.
pub(crate) fn children(context: UsageContext) -> Vec<(&'static str, HelpTopic)> {
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

/// At most three close static spellings from this failure's grammar scope.
/// Oversized and non-ASCII input produces no advice and cannot grow the work budget.
pub(crate) fn suggestions(reason: &UsageReason) -> Vec<&'static str> {
    let word = match reason {
        UsageReason::UnknownCommand { word, .. } => word.as_str(),
        UsageReason::UnknownOption { option, .. } => option.split('=').next().unwrap_or(option),
        _ => return Vec::new(),
    };
    if word.len() > 64 || !word.is_ascii() {
        return Vec::new();
    }
    let candidates = match reason {
        UsageReason::UnknownCommand { context, .. } => children(*context)
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>(),
        UsageReason::UnknownOption { context, .. } => {
            super::completion::usage_option_candidates(*context)
        }
        _ => unreachable!("reason was checked above"),
    };
    let threshold = if word.len() <= 3 { 1 } else { 2 };
    let mut ranked = candidates
        .into_iter()
        .map(|candidate| (edit_distance(word, candidate), candidate))
        .filter(|(distance, _)| *distance > 0 && *distance <= threshold)
        .collect::<Vec<_>>();
    ranked.sort_unstable();
    ranked.dedup();
    ranked
        .into_iter()
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
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
