//! State authority admission, clear and custom release share the report aggregate.
use super::{
    Author, Authority, Disposition, Reference, ReferenceKind, Reports, Sequence, SourcePolicy,
    Status, SuppressionReason,
};

enum Route {
    Accept { reset_sequence: bool },
    Ignore,
    Deferred,
}

impl Reports {
    pub(super) fn report_state(
        &mut self,
        author: Author,
        status: Status,
        message: Option<String>,
        reference: Option<Reference>,
        sequence: Option<Sequence>,
        at: u64,
    ) -> Disposition {
        if author.policy() == SourcePolicy::IdentityOnly {
            return Disposition::Ignored;
        }
        if !author.full()
            && self
                .recent_exit
                .as_ref()
                .is_some_and(|(agent, _)| author.known().is_some_and(|p| p.report_label() == agent))
        {
            return Disposition::Ignored;
        }
        let incoming = Authority {
            author: author.clone(),
            status,
            message,
            reference,
            at,
        };
        let reset_sequence = match self.route_state(&incoming, sequence) {
            Route::Accept { reset_sequence } => reset_sequence,
            Route::Ignore => return Disposition::Ignored,
            Route::Deferred => return Disposition::Deferred,
        };
        if self.known_conflicts(&author) {
            return Disposition::Ignored;
        }
        let conflicts = self.owner_conflicts(&author);
        let takeover = conflicts
            && incoming
                .reference
                .as_ref()
                .is_some_and(|r| self.foreground_confirms(&author, r));
        if conflicts && !takeover {
            return Disposition::Ignored;
        }
        let mut incoming = incoming;
        if let Some(reference) = incoming.reference.as_ref() {
            let mastra_replacement = self.authority.is_none()
                && author.source == "herdr:mastracode"
                && author.agent == "mastracode"
                && self.persisted.as_ref().is_some_and(|s| {
                    s.author == author
                        && s.reference.kind() == ReferenceKind::Id
                        && reference.kind() == ReferenceKind::Id
                        && s.reference != *reference
                });
            if !mastra_replacement
                && let Some(prior) = self.conflicting_reference(&author, reference, None)
            {
                incoming.reference = Some(prior);
            }
        }
        if self.authority.as_ref().is_some_and(|current| {
            current.author == author
                && current.author.full()
                && current
                    .reference
                    .as_ref()
                    .zip(incoming.reference.as_ref())
                    .is_some_and(|(a, b)| a != b)
        }) {
            return Disposition::Ignored;
        }
        if reset_sequence {
            self.sequences.remove(&author.source);
        }
        if !self.accept_sequence(&author.source, sequence) {
            return Disposition::Ignored;
        }
        if takeover {
            self.suppress_current(at);
        }
        if (incoming.reference.is_some() || reset_sequence)
            && let Some(suppressed) = self.suppressed.remove(&author.source)
            && let Some(reference) = suppressed.reference
        {
            self.remember_stale(
                &Author {
                    source: author.source.clone(),
                    agent: suppressed.agent,
                },
                reference,
            );
        }
        self.persisted = None;
        self.authority = Some(incoming);
        Disposition::Applied
    }
    fn route_state(&mut self, incoming: &Authority, sequence: Option<Sequence>) -> Route {
        let author = &incoming.author;
        if !author.full() {
            return Route::Accept {
                reset_sequence: false,
            };
        }
        if incoming.reference.as_ref().is_some_and(|reference| {
            self.stale.get(&author.source).is_some_and(|entries| {
                entries
                    .iter()
                    .any(|old| old.agent == author.agent && old.reference == *reference)
            })
        }) {
            return Route::Ignore;
        }
        let anchored = self
            .authority
            .as_ref()
            .filter(|a| a.author == *author)
            .and_then(|a| a.reference.as_ref())
            .or_else(|| {
                self.persisted
                    .as_ref()
                    .filter(|s| s.author == *author)
                    .map(|s| &s.reference)
            });
        let process = self.process_present(author);
        let session_anchored = anchored.is_some_and(|anchor| {
            incoming
                .reference
                .as_ref()
                .is_none_or(|reference| reference == anchor)
        });
        if author.source == "herdr:opencode"
            && author.agent == "opencode"
            && process
            && anchored
                .zip(incoming.reference.as_ref())
                .is_some_and(|(a, b)| a != b)
        {
            return Route::Ignore;
        }
        if let Some(suppressed) = self.suppressed.get(&author.source) {
            if suppressed.agent != author.agent {
                return Route::Ignore;
            }
            if suppressed.reason == SuppressionReason::Cleared {
                return if suppressed
                    .reference
                    .as_ref()
                    .zip(incoming.reference.as_ref())
                    .is_some_and(|(a, b)| a != b)
                {
                    Route::Accept {
                        reset_sequence: true,
                    }
                } else {
                    Route::Ignore
                };
            }
        }
        if process && session_anchored && !self.suppressed.contains_key(&author.source) {
            let reset_sequence = self.stale.get(&author.source).is_some_and(|entries| {
                entries.iter().any(|s| s.agent == author.agent)
                    && incoming.reference.as_ref().is_some_and(|r| {
                        entries
                            .iter()
                            .all(|s| s.agent != author.agent || s.reference != *r)
                    })
            });
            return Route::Accept { reset_sequence };
        }
        if incoming.reference.is_none() {
            return Route::Ignore;
        }
        let Some(sequence) = sequence else {
            return Route::Ignore;
        };
        if self
            .sequences
            .get(&author.source)
            .is_some_and(|last| sequence <= *last)
        {
            return Route::Ignore;
        }
        if !self.suppressed.contains_key(&author.source) {
            let reference = self
                .persisted
                .as_ref()
                .filter(|s| s.author == *author)
                .map(|s| s.reference.clone());
            self.suppress(author, reference, SuppressionReason::Exited, incoming.at);
        }
        let suppressed = self
            .suppressed
            .get_mut(&author.source)
            .expect("suppression admitted");
        if suppressed
            .pending
            .as_ref()
            .is_none_or(|(_, prior)| sequence > *prior)
        {
            suppressed.pending = Some((incoming.clone(), sequence));
        }
        Route::Deferred
    }
    pub(super) fn clear(
        &mut self,
        source: Option<&str>,
        sequence: Option<Sequence>,
        at: u64,
    ) -> Disposition {
        let Some(authority) = self
            .authority
            .as_ref()
            .filter(|a| source.is_none_or(|source| a.author.source == source))
        else {
            return Disposition::Ignored;
        };
        let source = authority.author.source.clone();
        if !self.accept_sequence(&source, sequence) {
            return Disposition::Ignored;
        }
        self.suppress_current(at);
        self.authority = None;
        self.persisted = None;
        Disposition::Applied
    }
    pub(super) fn release(
        &mut self,
        author: &Author,
        sequence: Option<Sequence>,
        _at: u64,
    ) -> (Disposition, bool) {
        if self.authority.as_ref().is_some_and(|a| a.author != *author) {
            return (Disposition::Ignored, false);
        }
        let matches_agent = self.effective().agent.as_deref() == Some(author.agent.as_str());
        let matches_session = self.persisted.as_ref().is_some_and(|s| s.author == *author);
        if (!matches_agent && !matches_session) || !self.accept_sequence(&author.source, sequence) {
            return (Disposition::Ignored, false);
        }
        let foreign_session = self.persisted.as_ref().is_some_and(|s| s.author != *author);
        let process_owns = self.process_present(author);
        if !process_owns {
            self.detected = None;
            self.fallback = Status::Unknown;
            self.visible_blocker = false;
            self.observed_at = None;
        }
        self.authority = None;
        if !foreign_session {
            self.persisted = None;
        }
        (Disposition::Applied, !process_owns)
    }
}
