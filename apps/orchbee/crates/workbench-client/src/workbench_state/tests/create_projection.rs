use super::*;

// Reducer-only fixtures use the actual candidate/cleanup/install plans. Carrier and atomic
// mailbox receipt consumption are covered by the native executor and WorkbenchClientUi membership tests.
impl WorkbenchClientState {
    pub(super) fn settle_create_snapshot(
        &mut self,
        operation: &CreateOperationKey,
        outcome: WorkbenchSnapshotOutcome,
    ) -> CreateProjectionDecision {
        let prepared = self.prepare_create_snapshot(operation, outcome);
        let PreparedCreateSnapshot::Available {
            baseline,
            committed,
            projection,
        } = prepared
        else {
            return self.apply_prepared_create_snapshot(prepared);
        };
        let result = (|| {
            let candidate =
                self.prepare_create_membership_candidate(&baseline, &committed, projection)?;
            let deadlines = committed.operation().deadlines();
            let desired = create_candidate_key(&committed, &candidate);
            let cleanup = self.stage_create_membership_cleanup(operation, candidate, deadlines)?;
            let prepared = self.prepare_cleaned_membership_candidate(&cleanup, cleanup.old())?;
            self.prepare_cleaned_membership_install(&cleanup, prepared, desired)
                .map(PreparedMembershipInstall::commit)
        })();
        match result {
            Ok(MembershipInstallDecision::Attach(target)) => {
                CreateProjectionDecision::Attach(target)
            }
            Ok(
                MembershipInstallDecision::SelectedWithoutAttachment
                | MembershipInstallDecision::Empty,
            ) => CreateProjectionDecision::SettledWithoutAttachment,
            Ok(MembershipInstallDecision::Preserved) => panic!("Create must install its candidate"),
            Err(failure) => {
                self.retire(failure);
                CreateProjectionDecision::Retire
            }
        }
    }
}

fn create_candidate_key(
    committed: &CommittedTabCreate,
    candidate: &PreparedMembershipCandidate,
) -> Option<AttachmentKey> {
    candidate.selected_tab_id().map(|tab| {
        if tab == committed.committed_key().tab_id() {
            committed.committed_key().clone()
        } else {
            AttachmentKey::new(committed.committed_key().generation() + 1, tab.clone())
        }
    })
}

pub(super) fn finish_create_refresh(
    state: &mut WorkbenchClientState,
    generation: u64,
    operation: &CreateOperationKey,
    candidate: PreparedMembershipCandidate,
    deadlines: OperationDeadlines,
) -> RefreshDecision {
    let committed = state.committed_target().unwrap().clone();
    let desired = create_candidate_key(&committed, &candidate);
    let cleanup = state
        .stage_create_refresh_cleanup(generation, operation, candidate, deadlines)
        .unwrap();
    assert_eq!(cleanup.deadlines(), deadlines);
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    match state
        .prepare_cleaned_membership_install(&cleanup, prepared, desired)
        .unwrap()
        .commit()
    {
        MembershipInstallDecision::Attach(target) => RefreshDecision::AttachRecovered(target),
        MembershipInstallDecision::SelectedWithoutAttachment | MembershipInstallDecision::Empty => {
            RefreshDecision::SettledWithoutAttachment
        }
        MembershipInstallDecision::Preserved => {
            panic!("Create recovery must install its candidate")
        }
    }
}
