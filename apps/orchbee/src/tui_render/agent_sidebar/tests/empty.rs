use super::*;
use crate::terminal_session::{WorkspaceCreationPhase, WorkspaceCreationPreview};
use orchbee_workbench_protocol::SiteId;

#[test]
fn empty_sidebar_keeps_actions_and_settings_without_site_business_or_zero_count_filler() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (18, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                let mut view = ClientView::new(width);
                view.machines.available = true;
                if width < 100 {
                    for key in [
                        KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL),
                        KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE),
                    ] {
                        let transition = view.decide(PrefixInput::Key(key));
                        view.commit(transition, SessionActionDisposition::KeepNavigator);
                    }
                }
                let site = SiteId::parse(&format!("{:032x}", 1)).unwrap();
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                // A pending creation is real content. Its removal must leave a fresh empty hit map.
                for creating in [true, false] {
                    let mut builder = HitMapBuilder::new();
                    let sidebar =
                        project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
                            .sidebar;
                    terminal
                        .draw(|frame| {
                            if let Some(sidebar) = sidebar {
                                render(
                                    frame,
                                    sidebar,
                                    RenderContext {
                                        view: &view,
                                        sidebar_surface: view.sidebar_surface(),
                                        locale,
                                        backend_status: SiteStatus {
                                            text: "No terminal tabs",
                                            role: Role::TextSecondary,
                                        },
                                        theme: &theme,
                                        input: SidebarInput::Terminal,
                                        creation: creating.then_some(WorkspaceCreationPreview {
                                            site: &site,
                                            label: Some("Codex 工作区"),
                                            project_id: None,
                                            committed: None,
                                            phase: WorkspaceCreationPhase::Preparing,
                                            current: true,
                                        }),
                                    },
                                    Some(&mut builder),
                                );
                            }
                        })
                        .unwrap();
                    let hits = builder.finish();
                    let buffer = terminal.backend().buffer();
                    let Some(sidebar) = sidebar else {
                        assert!(
                            (0..height)
                                .all(|y| (0..width).all(|x| hits.hit_target_at(x, y).is_none()))
                        );
                        continue;
                    };
                    for action in [
                        HitTarget::NewWorkspaceButton,
                        HitTarget::SearchButton,
                        HitTarget::SettingsButton,
                    ] {
                        assert!(
                            (sidebar.area.y..sidebar.area.bottom())
                                .any(|y| (sidebar.area.x..sidebar.area.right())
                                    .any(|x| hits.hit_target_at(x, y) == Some(&action))),
                            "{action:?}"
                        );
                    }
                    if !creating {
                        for y in 4..height.saturating_sub(3) {
                            for x in 0..sidebar.area.width.saturating_sub(2) {
                                assert_eq!(buffer[(x, y)].symbol(), " ", "empty body at {x},{y}");
                            }
                        }
                        assert!(
                            (0..height).all(|y| (0..sidebar.area.width).all(|x| !matches!(
                                hits.hit_target_at(x, y),
                                Some(
                                    HitTarget::MachineWorkspaceRow { .. }
                                        | HitTarget::MachineAgentRow { .. }
                                        | HitTarget::MachineProjectRow { .. }
                                )
                            )))
                        );
                    } else if height >= 24 {
                        let text: String =
                            buffer.content().iter().map(|cell| cell.symbol()).collect();
                        assert!(text.contains(locale.shell().workspaces));
                        assert!(text.contains("Codex"));
                    }
                }
            }
        }
    }
}
