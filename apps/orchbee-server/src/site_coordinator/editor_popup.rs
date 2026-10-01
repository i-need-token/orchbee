//! Editor preparation binds an already-authorized read source before committing a popup.
use super::SiteCoordinator;
use crate::terminal_coordinator::{BindingViewerTarget, PreparedOperation, WorkerPlacement};
use orchbee_capability_node_protocol as node;
use orchbee_client_server_protocol as client;
use orchbee_workbench_protocol::{
    ScrollbackEditorSpec, TerminalEditorFailure as Failure, WorkbenchRequestId,
};

pub(crate) struct EditorPopupLaunch<'a> {
    pub(crate) id: &'a WorkbenchRequestId,
    pub(crate) spec: &'a ScrollbackEditorSpec,
    pub(crate) source: node::TerminalEditorSource,
    pub(crate) target: BindingViewerTarget,
    pub(crate) selection: &'a node::LaunchSelection,
}

impl SiteCoordinator {
    pub(crate) fn editor_source_is_current(
        &self,
        spec: &ScrollbackEditorSpec,
        target: BindingViewerTarget,
        worker: Option<WorkerPlacement>,
    ) -> bool {
        spec.validate().is_ok()
            && worker == Some(target.worker)
            && spec.source.member.binding().is_some_and(|binding| {
                binding.worker_id.as_str() == hex(target.worker.worker_id.as_bytes())
                    && binding.terminal_id.as_str() == hex(target.terminal_id.as_bytes())
            })
            && self
                .workbench
                .as_ref()
                .is_some_and(|workbench| workbench.popup_source_matches(&spec.source))
            && self.target_lifecycle(target) == super::TargetLifecycle::Unfenced
    }

    pub(crate) fn prepare_editor_popup(
        &mut self,
        launch: EditorPopupLaunch<'_>,
        worker: Option<WorkerPlacement>,
        expiry: Option<node::LeaseExpiresAtUnixMilliseconds>,
    ) -> Result<
        (
            PreparedOperation,
            String,
            Option<node::TerminalOperationAuthorization>,
        ),
        Failure,
    > {
        let EditorPopupLaunch {
            id: popup_id,
            spec,
            source,
            target,
            selection,
        } = launch;
        if !self.editor_source_is_current(spec, target, worker)
            || source.host_epoch != target.identity.host_epoch()
            || source.terminal_id != target.identity.terminal_id()
        {
            return Err(Failure::StaleSource);
        }
        let reference =
            client::RequestId::parse(popup_id.as_str()).map_err(|_| Failure::InvalidIntent)?;
        let launch = client::LaunchSelection::new(
            selection.executable().to_owned(),
            selection.arguments().to_vec(),
        )
        .map_err(|_| Failure::EditorUnavailable)?;
        let title = std::path::Path::new(selection.executable())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(Failure::EditorUnavailable)?
            .to_owned();
        let request = client::TerminalCreateRequest::new(
            reference,
            None,
            None,
            client::StartMode::Deferred,
            Some(launch),
        );
        let mut plan = match self.terminals.plan_create(request, worker, expiry) {
            Ok(plan) => plan,
            Err(result) => return Ok((PreparedOperation::Ready(result), title, None)),
        };
        plan.set_editor_source(source)
            .map_err(|_| Failure::InvalidIntent)?;
        let authorization = plan.mark_popup();
        Ok((
            PreparedOperation::Dispatch(plan.commit()),
            title,
            Some(authorization),
        ))
    }
}
fn hex(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}
