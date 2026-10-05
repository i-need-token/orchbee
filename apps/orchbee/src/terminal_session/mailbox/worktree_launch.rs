//! Worktree source preparation shares the existing refresh fence and UI settlement.
use super::{AdmissionError, CommandMailbox, RefreshPurpose};
use crate::local_client::LocalOperationClock;
use crate::terminal_session::state::{
    NativeSessionState, OperationDeadlines,
    worktree_launch::{Launcher, WorktreeMode, WorktreeNotice, WorktreeToken},
};
impl CommandMailbox {
    pub(in crate::terminal_session) fn try_enqueue_worktree_preparation_for(
        &self,
        canonical: &mut NativeSessionState,
        launcher: &mut Launcher,
        mode: WorktreeMode,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> bool {
        let Some((state, _)) = self.lock_before(deadlines.forward_cutoff(), clock) else {
            return false;
        };
        if clock.now() >= deadlines.forward_cutoff() {
            return false;
        }
        let mut owners = (canonical, launcher);
        self.try_enqueue_refresh_intent_for(
            state,
            &mut owners,
            deadlines.total(),
            Some(deadlines),
            |(canonical, launcher), generation| {
                let preparation = canonical
                    .worktree_preparation(mode, generation, WorktreeToken(0), deadlines)
                    .ok_or(())?;
                if !launcher.open(preparation) {
                    return Err(());
                }
                let preparation = launcher.preparation().cloned().ok_or(())?;
                if !canonical.start_pending_refresh(generation) {
                    launcher.fail(WorktreeNotice::Busy);
                    return Err(());
                }
                Ok(RefreshPurpose::Worktree(Box::new(preparation)))
            },
            |(_, launcher), result: &Result<u64, AdmissionError<()>>| {
                if result.is_err() {
                    launcher.fail(WorktreeNotice::Busy);
                }
            },
        )
        .is_ok()
    }
}
