use super::*;
use crate::local_client::test_server::FakeServer;
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::{
    AgentViewResult, MetadataSource, MetadataTokenKey, MetadataTokenValue, Navigation,
    PanelAgentResult, PanelAgentRow, PanelMetadataResult, Presentation, Project, Tab,
    TerminalContext, TerminalId, TerminalTitleRow, WorkbenchProjectId, WorkbenchRequest,
    WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId, Workspace, read_workbench_request,
    write_workbench_response,
};
use std::{io, num::NonZeroU64, sync::atomic::Ordering, thread, time::Duration};

fn id(n: u64) -> String {
    format!("{n:032x}")
}

#[test]
fn regrouping_changes_placement_labels_without_changing_agent_terminal_targets() {
    let mut snapshot = fixture();
    let original = snapshot.terminal_contexts[0].clone();
    snapshot.terminal_contexts[0].tabs.clear();
    let mut context = original;
    context.terminal_context_id = WorkbenchTerminalContextId::parse(&id(80)).unwrap();
    snapshot.terminal_contexts.push(context.clone());
    let workspace = Workspace {
        workspace_id: WorkbenchWorkspaceId::parse(&id(81)).unwrap(),
        project_id: None,
        label: "feature".into(),
        primary_directory_id: None,
        terminal_context_id: context.terminal_context_id,
    };
    snapshot
        .navigation
        .workspace_order
        .push(workspace.workspace_id.clone());
    snapshot.navigation.workspaces.push(workspace);
    let before = placements(&snapshot).unwrap();
    assert_eq!(before.len(), 4);
    assert!(before.iter().all(|row| row.workspace == "feature"));
    let project = Project {
        project_id: WorkbenchProjectId::parse(&id(82)).unwrap(),
        label: "Repo".into(),
    };
    snapshot.navigation.workspaces[0].project_id = Some(project.project_id.clone());
    snapshot.navigation.projects.push(project);
    let grouped = placements(&snapshot).unwrap();
    for (before, grouped) in before.iter().zip(&grouped) {
        assert_eq!(grouped.workspace, "Repo / feature");
        assert_eq!(before.context, grouped.context);
        assert_eq!(before.tab, grouped.tab);
        assert_eq!(before.panel, grouped.panel);
        assert_eq!(before.member, grouped.member);
        assert_eq!(before.terminal, grouped.terminal);
        assert_eq!(before.order, grouped.order);
    }
    snapshot.navigation.projects.clear();
    snapshot.navigation.workspaces[0].project_id = None;
    for (before, after) in before.iter().zip(placements(&snapshot).unwrap()) {
        assert_eq!(after.workspace, "feature");
        assert_eq!(before.context, after.context);
        assert_eq!(before.member, after.member);
    }
}

pub(crate) fn fixture() -> Snapshot {
    Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&id(1)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            directories: Vec::new(),
            workspaces: Vec::new(),
            projects: Vec::new(),
            workspace_order: Vec::new(),
            standalone_terminal_context_id: WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
            groups: Vec::new(),
            tabs: [10, 20]
                .into_iter()
                .map(|n| Tab {
                    automatic_label: false,
                    group_id: None,
                    label: "tab".into(),
                    pinned: false,
                    tab_id: WorkbenchTabId::parse(&id(n)).unwrap(),
                    root: PanelNode::Panel {
                        right_click: Default::default(),
                        panel_id: WorkbenchPanelId::parse(&id(n + 1)).unwrap(),
                        primary_presentation: presentation(n + 2),
                        child_presentations: vec![presentation(n + 3)],
                    },
                })
                .collect(),
        }],
    }
}
fn presentation(n: u64) -> Presentation {
    Presentation {
        custom_label: None,
        label: "terminal\u{202e}label".into(),
        presentation_id: TerminalPresentationId::parse(&id(n)).unwrap(),
        binding: Some(PresentationBinding {
            worker_id: WorkerId::parse(&id(99)).unwrap(),
            terminal_id: TerminalId::parse(&id(n + 100)).unwrap(),
        }),
    }
}
pub(crate) fn physical(n: u64) -> AgentLaunchBinding {
    use orchbee_agent_observation_protocol::*;
    AgentLaunchBinding {
        presentation_id: AgentPresentationId::parse(&id(n)).unwrap(),
        generation: AgentAttachmentGeneration::parse(&id(1)).unwrap(),
        host_epoch: AgentHostEpoch::parse(&id(2)).unwrap(),
        host_terminal_id: AgentHostTerminalId::parse(&id(n + 100)).unwrap(),
    }
}
pub(crate) fn row(n: u64) -> Row {
    let mut target = target();
    target.member = TerminalPresentationId::parse(&id(n)).unwrap();
    target.terminal = presentation(n).binding.unwrap();
    target.physical = physical(n);
    Row {
        seen: true,
        order: PlacementOrder::default(),
        target,
        label: "build".into(),
        workspace: "space".into(),
        tab_label: Some("Task".into()),
        terminal_title: orchbee_workbench_protocol::TerminalTitleObservation::Unavailable {},
        state: PanelAgentState::Working,
        sequence: WorkbenchRevision::new(1),
        metadata: PanelMetadataEffective::default(),
        tokens: MetadataTokens::default(),
    }
}
pub(crate) fn target() -> Target {
    Target {
        context: NativeContextTarget {
            site_id: SiteId::parse(&id(1)).unwrap(),
            location: NativeContextLocation::Standalone(
                WorkbenchTerminalContextId::parse(&id(2)).unwrap(),
            ),
        },
        tab: WorkbenchTabId::parse(&id(20)).unwrap(),
        panel: WorkbenchPanelId::parse(&id(21)).unwrap(),
        member: TerminalPresentationId::parse(&id(23)).unwrap(),
        terminal: presentation(23).binding.unwrap(),
        physical: physical(23),
        agent: "codex".into(),
    }
}
fn state(member: &TerminalPresentationId) -> TerminalAgentStateRow {
    let n = u64::from_str_radix(member.as_str(), 16).unwrap();
    TerminalAgentStateRow {
        presentation_id: member.clone(),
        snapshot: Some(TerminalAgentStateSnapshot {
            binding: physical(n),
            sequence: WorkbenchRevision::new(8),
            agent: (n == 12 || n == 23).then(|| "codex".into()),
            state: PanelAgentState::Working,
        }),
    }
}
pub(crate) fn accept(stream: &mut orchbee_local_ipc::LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
        },
    )
    .map_err(io::Error::other)
}
pub(crate) fn checkpoint(rows: Vec<Row>) -> Arc<Checkpoint> {
    Arc::new(Checkpoint {
        site_id: SiteId::parse(&id(1)).unwrap(),
        report_revision: WorkbenchRevision::new(2),
        contexts: std::iter::once(target().context)
            .chain(rows.iter().map(|r| r.target.context.clone()))
            .collect::<Vec<_>>()
            .into(),
        view: AgentViewSnapshot {
            revision: WorkbenchRevision::new(0),
            spec: None,
        },
        rows: rows.into(),
    })
}
fn view_reply(stream: &mut orchbee_local_ipc::LocalStream, changed: bool) -> io::Result<()> {
    let WorkbenchRequest::AgentView(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("view read")
    };
    assert_eq!(request.action, AgentViewAction::Read {});
    write_workbench_response(
        stream,
        &WorkbenchResponse::AgentView(AgentViewResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: AgentViewOutcome::Available {
                snapshot: AgentViewSnapshot {
                    revision: WorkbenchRevision::new(u64::from(changed)),
                    spec: None,
                },
            },
        }),
    )
    .map_err(io::Error::other)
}
fn peer(bad: u8) -> io::Result<FakeServer> {
    FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("snapshot only")
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: fixture(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        view_reply(&mut stream, false)?;
        for round in 0..=1 {
            if round == 1 {
                if !title_reply(&mut stream, bad)? {
                    return Ok(());
                }
                if !enrich(&mut stream, bad)? {
                    return Ok(());
                }
            }
            let WorkbenchRequest::PanelAgent(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("read-only states")
            };
            let PanelAgentAction::States { presentation_ids } = &request.action else {
                panic!("no mutation")
            };
            assert_eq!(presentation_ids.len(), if round == 0 { 4 } else { 0 });
            let mut rows: Vec<_> = presentation_ids.iter().map(state).collect();
            if bad == 2 && round == 0 {
                rows.pop();
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::PanelAgent(PanelAgentResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    site_id: request.site_id,
                    outcome: PanelAgentOutcome::States {
                        workbench_revision: WorkbenchRevision::new(1),
                        report_revision: WorkbenchRevision::new(if bad == 1 && round == 1 {
                            3
                        } else {
                            2
                        }),
                        presentations: rows,
                    },
                }),
            )
            .map_err(io::Error::other)?;
            if bad == 2 {
                break;
            }
        }
        if bad != 1 && bad != 2 {
            view_reply(&mut stream, bad == 11)?;
        }
        Ok(())
    })
}
#[test]
fn agent_checkpoint_joins_exact_primary_and_child_without_terminal_effects() -> io::Result<()> {
    let peer = peer(0)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
        .map_err(io::Error::other)?;
    let rows = read(&mut client, deadline, &AtomicU8::new(0)).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].target, target());
    assert_eq!(rows[0].target.member.as_str(), id(12));
    assert_eq!(rows[0].tab_label.as_deref(), Some("tab"));
    assert!(
        matches!(&rows[0].terminal_title,TerminalTitleObservation::Observed {title:Some(t),..} if t=="⠋ 修复🙂标题")
    );
    assert!(!rows[0].label.contains('\u{202e}'));
    drop(client);
    peer.finish()
}
#[test]
fn agent_checkpoint_rejects_partial_membership_and_spliced_report_revisions() -> io::Result<()> {
    for bad in [1, 2, 11, 12, 13] {
        let peer = peer(bad)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        assert!(read(&mut client, deadline, &AtomicU8::new(0)).is_err());
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
#[test]
fn agent_placements_respect_membership_and_row_capacity() {
    let mut snapshot = fixture();
    assert_eq!(placements(&snapshot).unwrap().len(), 4);
    snapshot.terminal_contexts[0].tabs.truncate(1);
    snapshot.terminal_contexts[0].tabs[0].automatic_label = true;
    assert!(
        placements(&snapshot)
            .unwrap()
            .iter()
            .all(|p| p.tab_label.is_none())
    );
    snapshot.terminal_contexts[0].tabs[0].automatic_label = false;
    snapshot.terminal_contexts[0].tabs[0].label = "Manual".into();
    assert!(
        placements(&snapshot)
            .unwrap()
            .iter()
            .all(|p| p.tab_label.as_deref() == Some("Manual"))
    );
    snapshot.terminal_contexts.clear();
    assert!(placements(&snapshot).is_err());
    let snapshot = fixture();
    let mut rows = Vec::new();
    for _ in 0..MAXIMUM_ROWS / 2 {
        collect(
            &snapshot.terminal_contexts[0].tabs[0].root,
            &target().context,
            &target().tab,
            "",
            &mut rows,
        )
        .unwrap();
    }
    assert_eq!(rows.len(), MAXIMUM_ROWS);
    assert!(
        collect(
            &snapshot.terminal_contexts[0].tabs[0].root,
            &target().context,
            &target().tab,
            "",
            &mut rows
        )
        .is_err()
    );
}

#[test]
fn agent_checkpoint_cancel_interrupts_real_inflight_read_without_followup() -> io::Result<()> {
    let (arrived, reading) = std::sync::mpsc::sync_channel(1);
    let peer = FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(_)
        ));
        arrived.send(()).map_err(io::Error::other)?;
        // Cancellation closes the owned lane; no States, Attach or other followup.
        assert!(read_workbench_request(&mut stream).is_err());
        Ok(())
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
        .map_err(io::Error::other)?;
    let cancel = Arc::new(AtomicU8::new(0));
    let worker = cancel.clone();
    let task = thread::spawn(move || read(&mut client, deadline, &worker));
    reading
        .recv_timeout(Duration::from_secs(2))
        .map_err(io::Error::other)?;
    cancel.store(1, Ordering::Release);
    assert!(task.join().unwrap().is_err());
    peer.finish()
}

fn title_reply(stream: &mut orchbee_local_ipc::LocalStream, bad: u8) -> io::Result<bool> {
    let WorkbenchRequest::PanelAgent(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("titles read");
    };
    let PanelAgentAction::Titles { bindings } = request.action else {
        panic!("no attachment or mutation");
    };
    assert_eq!(bindings.len(), 2);
    let mut presentations: Vec<_> = bindings
        .into_iter()
        .map(|binding| TerminalTitleRow {
            binding,
            observation: TerminalTitleObservation::Observed {
                revision: WorkbenchRevision::new(1),
                title: Some("⠋ 修复🙂标题".into()),
            },
        })
        .collect();
    if bad == 13 {
        presentations[0].binding.generation =
            orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(&id(999)).unwrap();
    }
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelAgent(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome: PanelAgentOutcome::Titles {
                workbench_revision: WorkbenchRevision::new(if bad == 12 { 9 } else { 1 }),
                presentations,
            },
        }),
    )
    .map_err(io::Error::other)?;
    Ok(bad != 12 && bad != 13)
}
fn enrich(stream: &mut orchbee_local_ipc::LocalStream, bad: u8) -> io::Result<bool> {
    for (step, metadata) in [(0, true), (1, false), (2, true), (3, false)] {
        let request = read_workbench_request(stream).map_err(io::Error::other)?;
        let response = if metadata {
            let WorkbenchRequest::PanelMetadata(request) = request else {
                panic!("metadata read only")
            };
            let PanelMetadataAction::ReadPresentations { presentation_ids } = request.action else {
                panic!("exact metadata")
            };
            assert_eq!(presentation_ids.len(), if step == 0 { 4 } else { 0 });
            let mut rows: Vec<_>=presentation_ids.iter().map(|member| {
                let n=u64::from_str_radix(member.as_str(),16).unwrap();
                PresentationMetadataRow { presentation_id:member.clone(), panel:PanelMetadataRow {
                    panel_id:WorkbenchPanelId::parse(&id(n/10*10+1)).unwrap(), binding:presentation(n).binding,
                    tokens:MetadataTokens::new([(MetadataTokenKey::parse("issue").unwrap(),MetadataTokenValue::parse(&format!("ORCH-{n}")).unwrap())].into()).unwrap(),
                    presentations:vec![serde_json::from_value(serde_json::json!({"source":"hook","agent":"codex","appliesToSource":"hook","title":{"value":format!("Title {n}"),"order":"1"},"displayAgent":{"value":"Friendly Agent","order":"1"},"stateLabels":{"working":{"value":"Writing code","order":"1"}}})).unwrap()],
                }}
            }).collect();
            if bad == 5 && step == 0 {
                rows[0].panel.binding.as_mut().unwrap().terminal_id =
                    TerminalId::parse(&id(999)).unwrap();
            }
            if bad == 8 && step == 0 {
                for row in &mut rows {
                    row.panel.presentations[0].applies_to_source =
                        Some(MetadataSource::parse("other").unwrap());
                }
            }
            WorkbenchResponse::PanelMetadata(PanelMetadataResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: PanelMetadataOutcome::PresentationsRead {
                    workbench_revision: WorkbenchRevision::new(1),
                    metadata_revision: WorkbenchRevision::new(if bad == 3 && step == 2 {
                        3
                    } else {
                        1
                    }),
                    presentations: rows,
                },
            })
        } else {
            let WorkbenchRequest::PanelAgent(request) = request else {
                panic!("reports read only")
            };
            let PanelAgentAction::ReadPresentations { presentation_ids } = request.action else {
                panic!("exact reports")
            };
            assert_eq!(presentation_ids.len(), if step == 1 { 4 } else { 0 });
            let mut rows: Vec<_> = presentation_ids
                .iter()
                .map(|member| {
                    let n = u64::from_str_radix(member.as_str(), 16).unwrap();
                    let agent = (n == 12 || n == 23).then(|| "codex".to_owned());
                    PresentationAgentRow {
                        presentation_id: member.clone(),
                        panel: PanelAgentRow {
                            panel_id: WorkbenchPanelId::parse(&id(n / 10 * 10 + 1)).unwrap(),
                            binding: presentation(n).binding,
                            report: Some(PanelAgentView {
                                authority_source: agent.as_ref().map(|_| "hook".into()),
                                agent,
                                state: PanelAgentState::Working,
                                message: None,
                                session: None,
                            }),
                        },
                    }
                })
                .collect();
            if bad == 6 && step == 1 {
                rows[0].panel.panel_id = WorkbenchPanelId::parse(&id(999)).unwrap();
            }
            if bad == 7 && step == 1 {
                rows[0].panel.report.as_mut().unwrap().agent = Some("other".into());
            }
            WorkbenchResponse::PanelAgent(PanelAgentResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                site_id: request.site_id,
                outcome: PanelAgentOutcome::PresentationsRead {
                    workbench_revision: WorkbenchRevision::new(1),
                    report_revision: WorkbenchRevision::new(
                        if (bad == 4 && step == 3) || bad == 9 {
                            3
                        } else {
                            2
                        },
                    ),
                    presentations: rows,
                },
            })
        };
        write_workbench_response(stream, &response).map_err(io::Error::other)?;
        if (bad == 3 && step == 2)
            || (bad == 4 && step == 3)
            || (bad == 5 && step == 0)
            || (bad == 6 && step == 1)
            || (bad == 9 && step == 1)
        {
            return Ok(false);
        }
    }
    Ok(true)
}
#[test]
fn agent_checkpoint_metadata_requires_overlapping_revisions_exact_binding_and_matching_agent()
-> io::Result<()> {
    for bad in [3, 4, 5, 6, 7, 9] {
        let peer = peer(bad)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        assert!(read(&mut client, deadline, &AtomicU8::new(0)).is_err());
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
#[test]
fn agent_checkpoint_metadata_uses_authority_guards_without_losing_independent_tokens()
-> io::Result<()> {
    for bad in [0, 8] {
        let peer = peer(bad)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        let rows = read(&mut client, deadline, &AtomicU8::new(0)).unwrap();
        assert_eq!(rows[1].sequence, WorkbenchRevision::new(8));
        if bad == 0 {
            assert_eq!(rows[1].title(), "Title 23");
            assert_eq!(rows[1].provider(), "Friendly Agent");
            assert_eq!(rows[1].state_label(), Some("Writing code"));
        } else {
            assert_eq!(rows[1].title(), rows[1].label);
            assert_eq!(rows[1].provider(), "codex");
            assert!(rows[1].state_label().is_none());
        }
        assert_eq!(
            rows[1].tokens.as_map().values().next().unwrap().as_str(),
            "ORCH-23"
        );
        assert_eq!(rows[1].target, target());
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
