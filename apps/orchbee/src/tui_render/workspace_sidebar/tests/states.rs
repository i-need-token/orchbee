use super::*;
use crate::{
    client_view::Modifiers,
    terminal_input::{
        MouseAction, MouseButton, MouseEvent, TerminalInputPort, TerminalInputUnavailable,
    },
    tui_pointer::{MotionObservability, PointerState},
    tui_theme::ColorLevel,
};
use std::time::Instant;

struct Port;
impl TerminalInputPort for Port {
    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        panic!("sidebar pointer presentation must not forward terminal input")
    }
}

fn current_agent_view() -> (ClientView, SidebarTarget, SidebarTarget) {
    let mut view = view(120, 40);
    configure(
        &mut view,
        serde_json::json!([[token("workspace")], [token("branch")]]),
        0,
    );
    let workspace = view.machines.space_rows()[0].sidebar_target().unwrap();
    let SidebarTarget::Context { target, .. } = &workspace else {
        unreachable!()
    };
    let mut first = agent_for(target.clone(), 23);
    first.metadata.title = Some(MetadataTokenValue::parse("Codex task 编码").unwrap());
    let second = agent_for(target.clone(), 24);
    let member = first.target.clone();
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(vec![
            first, second,
        ])),
    )]);
    expand_context_agents(&mut view, target.clone());
    view.machines.observe_focus(
        Choice::Local,
        Some(target.clone()),
        Some(crate::terminal_session::NativeMemberTarget {
            tab: member.tab.clone(),
            panel: member.panel.clone(),
            presentation: member.member.clone(),
        }),
    );
    let agent = view
        .machines
        .sidebar_navigation_rows()
        .iter()
        .find_map(|row| match row {
            entry @ SidebarRow::Agent { row, .. }
                if row
                    .sidebar_agent()
                    .is_some_and(|agent| agent.target == member) =>
            {
                Some(entry.target())
            }
            _ => None,
        })
        .unwrap();
    (view, workspace, agent)
}

fn move_pointer(pointer: &mut PointerState, view: &mut ClientView, x: u16, y: u16, pressed: bool) {
    pointer.event(
        view,
        &mut Port,
        Instant::now(),
        MouseEvent {
            action: if pressed {
                MouseAction::Press
            } else {
                MouseAction::Motion
            },
            button: if pressed {
                MouseButton::Left
            } else {
                MouseButton::None
            },
            mods: Modifiers::NONE,
            column: x,
            row: y,
        },
    );
}

#[test]
fn current_workspace_and_agent_have_distinct_surfaces_and_collapse_restores_the_owner_fill() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::one_dark(),
            Theme::one_dark().quantize(ColorLevel::Ansi16),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
        ] {
            let (mut view, workspace, agent) = current_agent_view();
            let (buffer, _, hits) = paint_navigation_buffers(&view, locale, &theme, 40, 20);
            let owner_rows = sidebar_target_rows(&hits, 40, 20, &workspace);
            let agent_rows = sidebar_target_rows(&hits, 40, 20, &agent);
            assert_eq!(owner_rows.len(), 2);
            assert_eq!(agent_rows.len(), 2);
            assert_eq!(agent_rows[0], owner_rows[1] + 1);
            for y in owner_rows {
                assert!((0..40).all(|x| buffer[(x, y)].bg == theme.color(Role::SurfaceChrome)));
            }
            for y in agent_rows {
                assert!((0..40).all(|x| buffer[(x, y)].bg == theme.color(Role::SurfaceCurrent)));
            }
            assert!(view.machines.toggle_sidebar_context_agents(&workspace));
            let (collapsed, _, hits) = paint_navigation_buffers(&view, locale, &theme, 40, 20);
            assert!(sidebar_target_rows(&hits, 40, 20, &agent).is_empty());
            let owner_rows = sidebar_target_rows(&hits, 40, 20, &workspace);
            for y in owner_rows {
                assert!((0..40).all(|x| collapsed[(x, y)].bg == theme.color(Role::SurfaceCurrent)));
            }
            assert!(!view.machines.sidebar_navigation_active());
            assert!(view.machines.request.is_none());
        }
    }
}

#[test]
fn hover_press_and_navigation_compose_one_full_row_without_bolding_supporting_lines() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for original in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::one_dark(),
            Theme::one_dark().quantize(ColorLevel::Ansi16),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
        ] {
            let theme = original.paired_pointer_surfaces();
            for navigate in [false, true] {
                for owner in [false, true] {
                    let (mut view, workspace, agent) = current_agent_view();
                    let target = if owner { &workspace } else { &agent };
                    if navigate {
                        assert!(view.machines.focus_sidebar_target(target));
                    }
                    let focused_before = view.machines.sidebar_focused_target().cloned();
                    let current_before = view.machines.current_context.clone();
                    let (_, _, hits) = paint_navigation_buffers(&view, locale, &original, 40, 20);
                    let ys = sidebar_target_rows(&hits, 40, 20, target);
                    let mut pointer = PointerState::default();
                    pointer.set_capture(true);
                    pointer.set_motion_observability(MotionObservability::AnyMotion);
                    pointer.publish(hits);
                    for pressed in [false, true] {
                        move_pointer(&mut pointer, &mut view, 0, ys[0], pressed);
                        let (buffer, _, hits) = paint_navigation_buffers_with_pointer(
                            &view,
                            locale,
                            &original,
                            40,
                            20,
                            pointer.projection(),
                        );
                        let surface = if pressed {
                            Role::SurfacePressed
                        } else if navigate {
                            Role::SurfaceSelection
                        } else {
                            Role::SurfaceHover
                        };
                        let expected = theme.style(surface);
                        assert_eq!(ys.len(), 2, "identity and supporting lines: {target:?}");
                        let supporting_text: String = (0..40)
                            .filter(|x| !hidden_by_candidate_wide_glyph(&buffer, *x, ys[1]))
                            .map(|x| buffer[(x, ys[1])].symbol())
                            .collect();
                        assert!(
                            supporting_text.contains(if owner { "feat/测试" } else { "codex" }),
                            "expected branch/provider supporting line: {supporting_text}"
                        );
                        for y in &ys {
                            for x in 0..40 {
                                if hidden_by_candidate_wide_glyph(&buffer, x, *y) {
                                    continue;
                                }
                                let cell = &buffer[(x, *y)];
                                assert_eq!(cell.bg, expected.bg.unwrap(), "{surface:?} {x},{y}");
                                assert_eq!(
                                    cell.modifier.contains(Modifier::REVERSED),
                                    expected.add_modifier.contains(Modifier::REVERSED)
                                );
                                if *y != ys[0] && !cell.symbol().trim().is_empty() {
                                    assert!(
                                        !cell.modifier.contains(Modifier::BOLD),
                                        "metadata inherited row bold at {x},{y}"
                                    );
                                }
                            }
                        }
                        assert_eq!(
                            view.machines.sidebar_focused_target(),
                            focused_before.as_ref()
                        );
                        assert_eq!(view.machines.current_context, current_before);
                        assert!(view.machines.request.is_none());
                        pointer.publish(hits);
                    }
                }
            }
        }
    }
}

#[test]
fn explicit_identity_color_and_plain_override_survive_row_composition_without_promoting_metadata() {
    use orchbee_local_site_runtime::WindowsSidebarTokenStyle;
    for base in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark(),
        Theme::one_dark().quantize(ColorLevel::Ansi16),
        Theme::terminal_native().quantize(ColorLevel::Monochrome),
    ] {
        let theme = base.paired_pointer_surfaces();
        for surface in [
            Role::SurfaceCurrent,
            Role::SurfaceHover,
            Role::SurfacePressed,
            Role::SurfaceSelection,
        ] {
            for bold in [None, Some(false), Some(true)] {
                let identity = theme.sidebar_token_style(
                    Role::TextPrimary,
                    WindowsSidebarTokenStyle {
                        fg: Some([16, 32, 48]),
                        bold,
                        dim: Some(false),
                    },
                    false,
                    false,
                );
                let secondary = theme.style(Role::TextSecondary);
                let supporting_token = |bold| {
                    theme.sidebar_token_style(
                        Role::TextSecondary,
                        WindowsSidebarTokenStyle {
                            bold,
                            ..Default::default()
                        },
                        false,
                        false,
                    )
                };
                let state = theme.style(Role::StatusWorking);
                let mut lines = vec![
                    Line::from(vec![
                        Span::styled("Codex", identity),
                        Span::styled("provider", secondary),
                    ]),
                    Line::from(vec![
                        Span::styled("provider / branch", secondary),
                        Span::styled("explicit emphasis", supporting_token(Some(true))),
                        Span::styled("explicit plain", supporting_token(Some(false))),
                    ])
                    .style(secondary.add_modifier(Modifier::BOLD)),
                    Line::from(Span::styled("Working", state)),
                ];
                let mut provisional = lines.clone();
                sidebar_text::compose_lines(&mut provisional, Some(surface), false, &theme);
                assert_eq!(
                    provisional[0].spans[0]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD),
                    bold == Some(true)
                        || bold.is_none()
                            && theme.style(surface).add_modifier.contains(Modifier::BOLD)
                );
                sidebar_text::compose_lines(&mut lines, Some(surface), true, &theme);
                assert_eq!(lines[0].spans[0].style.fg, identity.fg);
                assert_eq!(
                    lines[0].spans[0]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD),
                    bold != Some(false)
                );
                assert!(
                    !lines[0].spans[1]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD)
                );
                assert!(
                    !lines[1].spans[0]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD)
                );
                assert!(
                    lines[1].spans[1]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD)
                );
                assert!(
                    !lines[1].spans[2]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD)
                );
                assert!(
                    lines[1].spans[2]
                        .style
                        .sub_modifier
                        .contains(Modifier::BOLD)
                );
                assert!(!lines[1].style.add_modifier.contains(Modifier::BOLD));
                assert!(lines[1].style.sub_modifier.contains(Modifier::BOLD));
                assert_eq!(lines[2].spans[0].style.fg, state.fg);
                assert!(
                    !lines[2].spans[0]
                        .style
                        .add_modifier
                        .contains(Modifier::BOLD)
                );
                for span in lines.iter().skip(1).flat_map(|line| &line.spans) {
                    assert_eq!(lines[0].spans[0].style.bg, span.style.bg);
                }
            }
        }
    }
}
