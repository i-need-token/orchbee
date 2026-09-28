//! Provider reference replacement and full-lifecycle reanchoring preserve source fences.
use super::{
    Author, Disposition, Provider, Reference, ReferenceKind, Reports, Sequence, Session,
    SourcePolicy, Start, SuppressionReason,
};

impl Reports {
    pub(super) fn report_session(
        &mut self,
        author: Author,
        reference: Option<Reference>,
        sequence: Option<Sequence>,
        start: Option<Start>,
        at: u64,
    ) -> Disposition {
        let Some(reference) = reference else {
            return Disposition::Ignored;
        };
        let process = self.process_present(&author);
        let full = author.full();
        let generation_gated = self
            .suppressed
            .get(&author.source)
            .is_some_and(|s| s.agent == author.agent && s.reason != SuppressionReason::Cleared);
        let anchored = self
            .authority
            .as_ref()
            .is_some_and(|a| a.author == author && a.reference.is_some())
            || self.persisted.as_ref().is_some_and(|s| s.author == author);
        let selection = author.source == "herdr:opencode"
            && author.agent == "opencode"
            && start == Some(Start::Select)
            && sequence.is_none();
        let selection_reconciles = selection && process;
        if selection_reconciles {
            self.suppressed.remove(&author.source);
        } else if full && selection {
            let previous = self
                .authority
                .as_ref()
                .filter(|a| a.author == author)
                .and_then(|a| a.reference.clone())
                .or_else(|| {
                    self.persisted
                        .as_ref()
                        .filter(|s| s.author == author)
                        .map(|s| s.reference.clone())
                });
            if !self.suppressed.contains_key(&author.source) {
                self.suppress(&author, previous, SuppressionReason::Exited, at);
            }
            let suppressed = self
                .suppressed
                .get_mut(&author.source)
                .expect("suppression admitted");
            suppressed.replacement = Some(reference);
            suppressed.pending = None;
            return Disposition::Deferred;
        }
        if full && !selection_reconciles && (!process || generation_gated || !anchored) {
            if !start.is_some_and(Start::is_full_lifecycle_start) {
                return Disposition::Ignored;
            }
            let Some(sequence) = sequence else {
                return Disposition::Ignored;
            };
            if self
                .sequences
                .get(&author.source)
                .is_some_and(|prior| sequence <= *prior)
            {
                return Disposition::Ignored;
            }
            if !self.suppressed.contains_key(&author.source) {
                self.suppress(&author, None, SuppressionReason::Exited, at);
            }
            let suppressed = self
                .suppressed
                .get_mut(&author.source)
                .expect("suppression admitted");
            if suppressed.replacement.as_ref() != Some(&reference) {
                if suppressed
                    .pending
                    .as_ref()
                    .is_some_and(|(pending, _)| pending.reference.as_ref() != Some(&reference))
                {
                    suppressed.pending = None;
                }
                suppressed.replacement = Some(reference);
            }
            self.sequences.insert(author.source.clone(), sequence);
            if process {
                self.confirm_process(None, author.known());
                return Disposition::Applied;
            }
            return Disposition::Deferred;
        }
        if !selection && !self.accept_sequence(&author.source, sequence) {
            return Disposition::Ignored;
        }
        if self.known_conflicts(&author) {
            return Disposition::Ignored;
        }
        let replacement = author.replacement(start);
        let identity_replacement = author.policy() == SourcePolicy::IdentityOnly
            && replacement
            && self.current_session().is_some_and(|s| {
                s.author == author
                    && s.reference.kind() == ReferenceKind::Id
                    && reference.kind() == ReferenceKind::Id
                    && s.reference != reference
            });
        if identity_replacement && !process {
            return Disposition::Ignored;
        }
        let conflicts = self.owner_conflicts(&author);
        let takeover = conflicts
            && !(author.source == "herdr:grok" && author.agent == "grok")
            && start.is_some_and(Start::is_full_lifecycle_start)
            && self.foreground_confirms(&author, &reference);
        if conflicts && !takeover {
            return Disposition::Ignored;
        }
        if self
            .conflicting_reference(&author, &reference, start)
            .is_some()
        {
            return Disposition::Ignored;
        }
        let replaced_hook = self
            .authority
            .as_ref()
            .filter(|a| a.author == author && a.author.full())
            .and_then(|a| a.reference.as_ref().filter(|prior| **prior != reference))
            .cloned();
        if replaced_hook.is_some() && !replacement {
            return Disposition::Ignored;
        }
        if replacement || takeover {
            self.forget_stale(&author, &reference);
        }
        if let Some(prior) = replaced_hook {
            self.remember_stale(&author, prior);
            self.authority = None;
        } else if takeover {
            self.suppress_current(at);
            self.authority = None;
        }
        self.persisted = Some(Session { author, reference });
        Disposition::Applied
    }
    pub(super) fn confirm_process(
        &mut self,
        previous: Option<Provider>,
        current: Option<Provider>,
    ) {
        let Some(current) = current else {
            return;
        };
        if previous == Some(current) {
            return;
        }
        let mut stale = Vec::new();
        let mut replacements = Vec::new();
        self.suppressed.retain(|source, suppressed| {
            if Provider::lookup(&suppressed.agent) != Some(current) {
                return true;
            }
            let author = Author {
                source: source.clone(),
                agent: suppressed.agent.clone(),
            };
            if suppressed.reason == SuppressionReason::Exited {
                let Some(reference) = suppressed.replacement.take() else {
                    return true;
                };
                if let Some(prior) = suppressed
                    .reference
                    .as_ref()
                    .filter(|old| **old != reference)
                {
                    stale.push((author.clone(), prior.clone()));
                }
                let start_sequence = self.sequences.get(source);
                let pending = suppressed.pending.take().filter(|(pending, sequence)| {
                    pending.reference.as_ref() == Some(&reference)
                        && start_sequence.is_none_or(|start| sequence > start)
                });
                replacements.push((author, reference, pending));
                false
            } else {
                if let Some(reference) = suppressed.reference.clone() {
                    stale.push((author, reference));
                }
                false
            }
        });
        for (author, reference) in stale {
            self.remember_stale(&author, reference);
        }
        self.sequences.retain(|source, _| {
            replacements.iter().any(|(a, _, _)| a.source == *source)
                || super::agent_report_source_policy(source, current.report_label())
                    != SourcePolicy::FullLifecycle
        });
        for (author, reference, pending) in replacements {
            self.forget_stale(&author, &reference);
            self.persisted = Some(Session {
                author: author.clone(),
                reference,
            });
            if let Some((authority, sequence)) = pending {
                self.sequences.insert(author.source, sequence);
                self.authority = Some(authority);
            }
        }
    }
}
