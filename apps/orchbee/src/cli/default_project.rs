//! First-Project admission is revision-fenced, never a label-based singleton.
use super::{Failure, FailureKind, results, target_details};
use crate::invocation::{Resource, Target};
use orchbee_workbench_protocol::{Snapshot, WorkbenchProjectId};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Selection {
    Existing(WorkbenchProjectId),
    CreateDefault,
}

pub(super) fn select(
    snapshot: &Snapshot,
    explicit: Option<&Target>,
    source: Option<&WorkbenchProjectId>,
) -> Result<Selection, Failure> {
    let projects = &snapshot.navigation.projects;
    if let Some(target) = explicit {
        return results::resolve_or_fail(
            projects
                .iter()
                .map(|project| (project.project_id.as_str(), project.label.as_str(), project)),
            target,
            Resource::Project,
        )
        .map(|project| Selection::Existing(project.project_id.clone()));
    }
    if let Some(source) = source {
        return projects
            .iter()
            .find(|project| &project.project_id == source)
            .map(|project| Selection::Existing(project.project_id.clone()))
            .ok_or_else(|| {
                Failure::new(FailureKind::NotFound)
                    .details(target_details(Resource::Project, source.as_str()))
            });
    }
    match projects.as_slice() {
        [] => Ok(Selection::CreateDefault),
        [project] => Ok(Selection::Existing(project.project_id.clone())),
        _ => Err(results::ambiguous(
            Resource::Project,
            &Target::Label(String::new()),
            projects
                .iter()
                .map(|project| project.project_id.to_string())
                .collect(),
        )),
    }
}

#[cfg(windows)]
pub(super) fn create(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    label: orchbee_workbench_protocol::WorkbenchLabel,
    deadline: std::time::Instant,
    signals: &mut super::source::ArmedSignals,
    committed: &mut Option<super::mutation::Receipt>,
) -> Result<(WorkbenchProjectId, Snapshot), Failure> {
    use super::source::SignalWatch;
    signals.check_before(deadline)?;
    if !snapshot.navigation.projects.is_empty() || committed.is_some() {
        return Err(Failure::new(FailureKind::Denied));
    }
    let receipt = super::mutation::submit_creation(
        client,
        snapshot,
        &crate::invocation::MutationCommand::ProjectCreate { label },
        deadline.min(std::time::Instant::now() + std::time::Duration::from_secs(2)),
        signals.poll_fd(),
    )?;
    // Publish the known commit to the invocation before any fallible follow-up.
    *committed = Some(receipt);
    let receipt = committed
        .as_ref()
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let project = receipt
        .id_value("projectId")
        .and_then(|value| WorkbenchProjectId::parse(value).ok())
        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
    let minimum = receipt
        .committed_revision()
        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
    let fresh =
        super::query::refresh_snapshot(client, &snapshot.site_id, minimum, deadline, signals)?;
    if !fresh
        .navigation
        .projects
        .iter()
        .any(|entry| entry.project_id == project)
    {
        return Err(Failure::new(FailureKind::NotFound)
            .details(target_details(Resource::Project, project.as_str())));
    }
    Ok((project, fresh))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn id(n: u8) -> String {
        format!("{n:032x}")
    }
    fn snapshot(count: u8) -> Snapshot {
        serde_json::from_value(json!({"protocolVersion":1,"siteId":id(1),"revision":"2","navigation":{"standaloneTerminalContextId":id(2),"projects":(0..count).map(|n| json!({"projectId":id(n+3),"label":if n==0 {"Default"} else {"other"},"directories":[],"workspaces":[]})).collect::<Vec<_>>()},"terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[]}]})).unwrap()
    }
    #[test]
    fn zero_projects_is_the_only_default_creation_admission() {
        assert_eq!(
            select(&snapshot(0), None, None).unwrap(),
            Selection::CreateDefault
        );
        assert_eq!(
            select(&snapshot(1), None, None).unwrap(),
            Selection::Existing(WorkbenchProjectId::parse(&id(3)).unwrap())
        );
        assert_eq!(
            select(&snapshot(2), None, None).unwrap_err().kind,
            FailureKind::Ambiguous
        );
        let ambiguity = select(&snapshot(10), None, None).unwrap_err();
        let details = ambiguity.details.unwrap();
        assert_eq!(details["totalCount"], 10);
        assert_eq!(details["candidates"].as_array().unwrap().len(), 8);
        assert_eq!(
            select(
                &snapshot(0),
                Some(&Target::Label("missing".to_owned())),
                None
            )
            .unwrap_err()
            .kind,
            FailureKind::NotFound
        );
    }
    #[test]
    fn explicit_and_source_projects_use_identity_without_default_label_privilege() {
        let mut snapshot = snapshot(2);
        let source = WorkbenchProjectId::parse(&id(3)).unwrap();
        assert_eq!(
            select(
                &snapshot,
                Some(&Target::Label("other".to_owned())),
                Some(&source)
            )
            .unwrap(),
            Selection::Existing(WorkbenchProjectId::parse(&id(4)).unwrap())
        );
        snapshot.navigation.projects[0].label = "renamed".to_owned();
        assert_eq!(
            select(&snapshot, None, Some(&source)).unwrap(),
            Selection::Existing(source)
        );
        let stale = WorkbenchProjectId::parse(&id(9)).unwrap();
        assert_eq!(
            select(&snapshot, None, Some(&stale)).unwrap_err().kind,
            FailureKind::NotFound
        );
    }
}
