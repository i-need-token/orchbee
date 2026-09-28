//! Trusted process observations reconcile report generations and emit metadata cleanup intent.
use super::{
    Author, Disposition, Process, Reports, Session, SourcePolicy, Status, SuppressionReason,
};

impl Reports {
    pub(super) fn observe(&mut self, process: Process, at: u64) -> (Disposition, bool) {
        let owner_at = at;
        let at = process.observed_at;
        let previous_agent = self.effective().agent;
        let previous_detected = self.detected;
        let newer_custom = process.exited
            && self.authority.as_ref().is_some_and(|a| {
                a.author.known() == process.agent
                    && a.author.policy() == SourcePolicy::Custom
                    && a.at > at
            });
        let released = process.exited && !newer_custom && previous_agent.is_some();
        let conflicts = self.authority.as_ref().is_some_and(|a| {
            process
                .agent
                .zip(a.author.known())
                .is_some_and(|(a, b)| a != b)
        });
        if self.live_full() && !process.exited && !conflicts {
            if self.authority.as_ref().and_then(|a| a.author.known()) == process.agent {
                self.detected = process.agent;
            }
            return (Disposition::Ignored, false);
        }
        let replacement = !process.exited
            && process.agent.is_some()
            && self.recent_exit.as_ref().is_some_and(|(agent, observed)| {
                Some(agent.as_str()) == process.agent.map(|p| p.report_label()) && *observed < at
            });
        if !process.exited
            && process.agent.is_some()
            && self
                .suppressed
                .values()
                .any(|s| super::Provider::lookup(&s.agent) == process.agent && at <= s.at)
        {
            return (Disposition::Ignored, false);
        }
        self.detected = process.agent;
        if !process.exited {
            self.confirm_process(
                if replacement { None } else { previous_detected },
                process.agent,
            );
        }
        self.fallback = process.status;
        self.visible_blocker = process.visible_blocker && process.status == Status::Blocked;
        self.observed_at = Some(at);
        if process.exited {
            if let Some(agent) = process.agent {
                self.recent_exit = Some((agent.report_label().to_owned(), at));
            }
        } else if process.agent.is_some() {
            self.recent_exit = None;
        }
        if process.exited {
            let mut resets = Vec::new();
            let mut stale = Vec::new();
            for (source, suppressed) in &mut self.suppressed {
                if super::Provider::lookup(&suppressed.agent) != process.agent
                    || suppressed.reason == SuppressionReason::Cleared
                {
                    continue;
                }
                let exited = suppressed
                    .replacement
                    .take()
                    .or_else(|| {
                        suppressed
                            .pending
                            .as_ref()
                            .and_then(|(p, _)| p.reference.clone())
                    })
                    .or_else(|| suppressed.reference.clone());
                if let Some((previous, exited)) = suppressed.reference.as_ref().zip(exited.as_ref())
                    && previous != exited
                {
                    stale.push((
                        Author {
                            source: source.clone(),
                            agent: suppressed.agent.clone(),
                        },
                        previous.clone(),
                    ));
                }
                suppressed.reference = exited;
                suppressed.pending = None;
                suppressed.at = at;
                resets.push(source.clone());
            }
            for (author, reference) in stale {
                self.remember_stale(&author, reference);
            }
            for source in resets {
                self.sequences.remove(&source);
            }
            let official = self
                .authority
                .as_ref()
                .filter(|a| {
                    a.author.policy() != SourcePolicy::Custom && a.author.known() == process.agent
                })
                .map(|a| (a.author.clone(), a.reference.clone()))
                .or_else(|| {
                    self.persisted
                        .as_ref()
                        .filter(|s| {
                            s.author.policy() != SourcePolicy::Custom
                                && s.author.known() == process.agent
                        })
                        .map(|s| (s.author.clone(), Some(s.reference.clone())))
                });
            if let Some((author, reference)) = official {
                self.sequences.remove(&author.source);
                self.suppress(&author, reference, SuppressionReason::Exited, at);
            }
            if let Some(source) = self
                .authority
                .as_ref()
                .filter(|a| a.author.known() == process.agent && !newer_custom)
                .map(|a| a.author.source.clone())
            {
                self.sequences.remove(&source);
                self.authority = None;
            }
            if !newer_custom
                && self
                    .persisted
                    .as_ref()
                    .is_some_and(|s| s.author.known() == process.agent)
            {
                self.persisted = None;
            }
        }
        let drop_authority = self.authority.as_ref().is_some_and(|a| {
            a.at <= at
                && (process
                    .agent
                    .zip(a.author.known())
                    .is_some_and(|(a, b)| a != b)
                    || (previous_detected.is_some()
                        && process.agent != previous_detected
                        && a.author.known() == previous_detected))
        });
        if drop_authority {
            let durable = self.authority.as_ref().and_then(|a| {
                a.reference.clone().map(|reference| Session {
                    author: a.author.clone(),
                    reference,
                })
            });
            self.suppress_current(owner_at);
            self.authority = None;
            self.persisted = durable;
        }
        (Disposition::Applied, released)
    }
}
