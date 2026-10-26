//! Workspace fixtures shared by library and service regression tests through public authority APIs.
use orchbee_server::{
    CommittedWorkbenchTabCreateMetadata, PreparedWorkbenchTabCreate, WorkbenchAuthority,
    WorkbenchGrant, WorkbenchTabCreateBegin, WorkbenchTabCreatePreparation,
    WorkbenchTerminalInvariantError, WorkbenchTerminalMembership,
};
use orchbee_workbench_protocol::*;

fn identity(value: u128) -> String {
    format!("{value:032x}")
}

pub(crate) trait WorkbenchFixture {
    fn workspace_fixture(site: SiteId) -> Result<WorkbenchAuthority, ContractError>;
    fn seed_fixture_workspace(&mut self, worker: WorkerId);
    fn fixture_context_id(&self) -> &WorkbenchTerminalContextId;
    fn fixture_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError>;
    fn fixture_tree_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError>;
    fn fixture_presentation_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError>;
    fn prepare_fixture_tab(
        &mut self,
        binding: PresentationBinding,
    ) -> Result<Box<PreparedWorkbenchTabCreate<'_>>, WorkbenchCommandResult>;
    fn append_fixture_tab(
        &mut self,
        binding: PresentationBinding,
    ) -> Result<CommittedWorkbenchTabCreateMetadata, WorkbenchCommandResult>;
}

impl WorkbenchFixture for WorkbenchAuthority {
    fn workspace_fixture(site: SiteId) -> Result<Self, ContractError> {
        let mut authority = Self::new(site)?;
        authority
            .seed_fixture_workspace(WorkerId::parse("0909090909090909090909090909090909").unwrap());
        Ok(authority)
    }

    fn seed_fixture_workspace(&mut self, worker: WorkerId) {
        assert!(self.snapshot().navigation.workspaces.is_empty());
        let site = self.site_id().clone();
        let grant = WorkbenchGrant::manage_navigation(site.clone()).with_directory_association();
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&identity(100_001)).unwrap(),
            site_id: site.clone(),
            expected_revision: self.snapshot().revision,
            action: WorkbenchAction::WorkspaceCreate {
                project_id: None,
                label: WorkbenchLabel::parse("Workspace").unwrap(),
            },
        };
        let WorkbenchResponse::Command(result) = self.execute(
            Some(&grant),
            WorkbenchRequest::Command(command).try_into().unwrap(),
        ) else {
            panic!("fixture Workspace result")
        };
        let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = result.outcome else {
            panic!("fixture Workspace commit")
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&identity(100_002)).unwrap(),
            site_id: site.clone(),
            expected_revision: self.snapshot().revision,
            action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                workspace_id,
                path: WorkbenchDirectoryPath::parse("/workspace").unwrap(),
            },
        };
        let result = self.accept_workspace_directory(Some(&grant), command, worker.clone());
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
        ));
        // A fresh restored authority provides revision zero and an empty command ledger.
        // This is fixture setup, using the current strict checkpoint shape.
        let checkpoint = self.checkpoint(&worker).unwrap();
        *self = Self::from_checkpoint(&checkpoint, site, &worker).unwrap();
    }

    fn fixture_context_id(&self) -> &WorkbenchTerminalContextId {
        &self
            .snapshot()
            .navigation
            .workspaces
            .first()
            .expect("fixture Workspace")
            .terminal_context_id
    }

    fn fixture_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        self.terminal_membership(self.fixture_context_id())
    }

    fn fixture_tree_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        self.terminal_tree_membership(self.fixture_context_id())
    }

    fn fixture_presentation_membership(
        &self,
    ) -> Result<WorkbenchTerminalMembership, WorkbenchTerminalInvariantError> {
        self.presentation_membership(self.fixture_context_id())
    }

    /// Append and settle a real TabCreate command in the explicitly seeded Workspace.
    fn prepare_fixture_tab(
        &mut self,
        binding: PresentationBinding,
    ) -> Result<Box<PreparedWorkbenchTabCreate<'_>>, WorkbenchCommandResult> {
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&identity(
                100_000 + u128::from(self.snapshot().revision.value()),
            ))
            .unwrap(),
            site_id: self.site_id().clone(),
            expected_revision: self.snapshot().revision,
            action: WorkbenchAction::TabCreate {
                placement: None,
                automatic_label: false,
                terminal_context_id: self.fixture_context_id().clone(),
                label: WorkbenchLabel::parse("1").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        };
        let grant = WorkbenchGrant::manage_navigation_and_terminal_create(self.site_id().clone());
        let admission = match self.begin_terminal_tab_create(Some(&grant), command) {
            WorkbenchTabCreateBegin::Admitted(admission) => admission,
            WorkbenchTabCreateBegin::Ready(result) => return Err(result),
        };
        match admission.finish(binding) {
            WorkbenchTabCreatePreparation::Prepared(prepared) => Ok(prepared),
            WorkbenchTabCreatePreparation::Ready(result) => Err(result),
        }
    }

    fn append_fixture_tab(
        &mut self,
        binding: PresentationBinding,
    ) -> Result<CommittedWorkbenchTabCreateMetadata, WorkbenchCommandResult> {
        let committed = self.prepare_fixture_tab(binding)?.commit();
        let metadata = committed.metadata().clone();
        let pending = committed.into_pending();
        let settled = self
            .prepare_terminal_tab_settlement(
                pending,
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started {
                    geometry: metadata.geometry(),
                },
            )
            .unwrap()
            .commit();
        assert!(matches!(
            settled.outcome,
            WorkbenchCommandOutcome::TabCreated {
                terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
                ..
            }
        ));
        Ok(metadata)
    }
}
