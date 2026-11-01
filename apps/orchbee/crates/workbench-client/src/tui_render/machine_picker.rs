//! Machine picker painters: the capturing picker window, its Agent controls and query rows.
//! The inherent `Picker::render` entry lives here so painting reads the picker without the
//! picker state depending on presentation.
use crate::machine_picker::{
    Choice, Control, DrawnPickerTarget, Mode, Picker, PickerAction, PickerSource,
};
use crate::{
    client_view::{
        Binding, KeyCode, LabelStyle, Modifiers, format_binding_with_glyphs, join_key_labels,
    },
    localization::Locale,
    machine_discovery::{Observation, display_label},
    terminal_text::{
        display_width, sanitize_terminal_text, truncate_display, truncate_display_start,
    },
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, ListScrollTarget,
        PrimaryContract, ProjectedPointer,
    },
    tui_layout::ViewportTier,
    tui_render::controls::{
        ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState,
        FocusState, IconActionGlyph, IconActionSpec, InteractionState, KeyHint, NavigationState,
        OperationState, PointerPresentation, QueryField, RowContentBand, RowFocusCue, RowTracks,
        action_rect, frame_close_rect, measure_key_hints, paint_row_interaction, render_action,
        render_frame_icon_action, render_key_hints, render_query_field,
    },
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    widgets::{Block, Clear, Paragraph},
};
/// The effective key that opens the Agent search query, formatted for the host.
fn search_key_label(theme: &Theme) -> String {
    format_binding_with_glyphs(
        &Binding::new(KeyCode::Char('/'), Modifiers::NONE),
        LabelStyle::for_host(),
        theme.glyphs.key_labels(),
    )
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

fn interaction_spec(target: &DrawnPickerTarget) -> InteractionSpec {
    if target.available() {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    }
}

fn picker_interaction(
    source: &PickerSource,
    action: PickerAction,
) -> Option<(InteractionTarget, InteractionSpec)> {
    let drawn = source.drawn(action)?;
    let spec = interaction_spec(&drawn);
    Some((InteractionTarget::MachinePicker(drawn), spec))
}

fn component_state(
    pointer: PointerPresentation,
    navigation: NavigationState,
    current: CurrentState,
    checked: CheckedState,
    availability: Availability,
    operation: OperationState,
) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        pointer,
        navigation,
        current,
        checked,
        availability,
        operation,
    )
}

fn render_picker_close(
    frame: &mut Frame<'_>,
    area: Rect,
    source: &PickerSource,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let rect = frame_close_rect(frame.area(), area)?;
    let (target, spec) = picker_interaction(source, PickerAction::Close)?;
    let pointer =
        pointer_presentation(hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec));
    let painted = render_frame_icon_action(
        frame,
        rect,
        IconActionSpec::new(
            IconActionGlyph::Close,
            ControlSurface::Overlay,
            Availability::Enabled,
            pointer,
        ),
        theme,
    )?;
    hits.record_overlay_dismissal(&target);
    hits.record_interaction(HitLayer::Overlay, painted, painted, target, spec);
    Some(painted)
}

struct AgentAction<'a> {
    area: Rect,
    control: Control,
    label: &'a str,
    kind: ActionKind,
}

impl Picker {
    fn render_capture_status(&self, frame: &mut Frame<'_>, locale: Locale, theme: &Theme) {
        let area = frame.area();
        if area.is_empty() {
            return;
        }
        let status = Rect::new(area.x, area.bottom() - 1, area.width, 1);
        frame.render_widget(Clear, status);
        frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), status);
        let editing = self.mode() == Mode::Agents && self.editing_query();
        let label = if self.mode() == Mode::Workspaces {
            locale.machine_picker_text("workspace-search")
        } else if editing {
            locale.machine_picker_text("agent-search")
        } else {
            self.mode().label(locale)
        };
        let glyphs = theme.glyphs.key_labels();
        let style = LabelStyle::for_host();
        let close =
            format_binding_with_glyphs(&Binding::new(KeyCode::Esc, Modifiers::NONE), style, glyphs);
        let enter = format_binding_with_glyphs(
            &Binding::new(KeyCode::Enter, Modifiers::NONE),
            style,
            glyphs,
        );
        let tab =
            format_binding_with_glyphs(&Binding::new(KeyCode::Tab, Modifiers::NONE), style, glyphs);
        let up =
            format_binding_with_glyphs(&Binding::new(KeyCode::Up, Modifiers::NONE), style, glyphs);
        let down = format_binding_with_glyphs(
            &Binding::new(KeyCode::Down, Modifiers::NONE),
            style,
            glyphs,
        );
        let choose = join_key_labels(&[up, down], style, "/");
        // Editing keys such as Ctrl+U are conventions, not advertised hints.
        let hints = if editing {
            vec![
                KeyHint::new(&close, locale.machine_picker_text("picker-finish-hint")),
                KeyHint::new(&enter, locale.machine_picker_text("picker-finish-hint")),
            ]
        } else {
            vec![
                KeyHint::new(&close, locale.machine_picker_text("picker-close-hint")),
                KeyHint::new(&choose, locale.machine_picker_text("picker-choose-hint")),
                KeyHint::new(&enter, locale.machine_picker_text("picker-open-hint")),
                KeyHint::new(&tab, locale.machine_picker_text("picker-mode-hint")),
            ]
        };
        let full_width = measure_key_hints(&hints, theme);
        let label_width = u16::try_from(display_width(label)).unwrap_or(u16::MAX);
        let show_label = full_width.is_some_and(|width| {
            usize::from(label_width)
                .saturating_add(1)
                .saturating_add(width)
                <= usize::from(status.width)
        });
        let hint_area = if show_label {
            let width = u16::try_from(full_width.unwrap_or(0)).unwrap_or(status.width);
            frame.render_widget(
                Paragraph::new(label)
                    .style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD)),
                Rect::new(status.x, status.y, label_width.min(status.width), 1),
            );
            Rect::new(status.right().saturating_sub(width), status.y, width, 1)
        } else {
            status
        };
        let _ = render_key_hints(frame, hint_area, &hints, theme);
    }

    pub fn render(
        &self,
        frame: &mut Frame<'_>,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let Some(source) = self.picker_source() else {
            return;
        };
        let area = frame.area();
        self.render_capture_status(frame, locale, theme);
        if ViewportTier::from_area(area) == ViewportTier::TooSmall {
            return;
        }
        hits.start_modal();
        let width = area.width.saturating_sub(4).min(72);
        let height = area.height.saturating_sub(3).min(
            ((if self.mode() == Mode::Workspaces {
                self.workspace_rows().len()
            } else {
                self.visible_rows().len()
            }) + match self.mode() {
                Mode::Agents => 13,
                Mode::Workspaces => 11,
                Mode::Machines => 10,
            })
            .min(24) as u16,
        );
        if width < 5 || height == 0 {
            let _ = render_picker_close(frame, area, &source, theme, hits);
            return;
        }
        let panel = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, panel);
        let title = format!(
            "{} {}/{}",
            self.mode().label(locale),
            (self.selected() + 1).min(self.visible_rows().len()),
            self.visible_rows().len()
        );
        frame.render_widget(
            crate::tui_render::titled_overlay(
                theme,
                ratatui::widgets::Padding::ZERO,
                &title,
                panel.width,
            ),
            panel,
        );
        let _ = render_picker_close(frame, panel, &source, theme, hits);
        if width < 34 || height < 5 {
            return;
        }
        let mut inner = panel.inner(ratatui::layout::Margin::new(1, 1));
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        if inner.height >= 4 {
            let mut x = inner.x;
            for mode in [Mode::Machines, Mode::Workspaces, Mode::Agents] {
                let label = mode.label(locale);
                let Ok(label_width) = u16::try_from(display_width(label)) else {
                    continue;
                };
                let Some(width) = label_width.checked_add(1) else {
                    continue;
                };
                if width == 1 || x.saturating_add(width) > inner.right() {
                    continue;
                }
                let rect = Rect::new(x, inner.y, width, 1);
                let primary = Rect::new(rect.x + 1, rect.y, label_width, 1);
                let Some((target, spec)) = picker_interaction(&source, PickerAction::Mode(mode))
                else {
                    continue;
                };
                let pointer = pointer_presentation(hits.projected_pointer(
                    HitLayer::Overlay,
                    rect,
                    rect,
                    &target,
                    spec,
                ));
                frame.render_widget(
                    Paragraph::new(label).style(theme.style(Role::TextPrimary)),
                    primary,
                );
                // Mode labels form a tab strip: the active mode is current, not a checked
                // option, so it needs no checked-glyph track.
                let current = self.mode() == mode;
                let state = component_state(
                    pointer,
                    NavigationState::None,
                    if current {
                        CurrentState::Current
                    } else {
                        CurrentState::NotCurrent
                    },
                    CheckedState::Unchecked,
                    Availability::Enabled,
                    OperationState::Idle,
                );
                if paint_row_interaction(
                    frame,
                    rect,
                    RowTracks::new(RowFocusCue::EnclosingFrame, primary),
                    &[],
                    ControlSurface::Overlay,
                    state,
                    theme,
                )
                .is_some()
                {
                    if current {
                        frame.buffer_mut().set_style(
                            primary,
                            theme
                                .style(Role::SurfaceSelection)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                        );
                    }
                    hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
                }
                // The label is already painted, so the next one must never overlap it.
                x = rect.right().saturating_add(1);
            }
            inner.y += 1;
            inner.height -= 1;
        }
        self.render_workspace_query(frame, &mut inner, locale, theme, &source, hits);
        self.render_agent_controls(frame, &mut inner, locale, theme, &source, hits);
        let has_remote = self
            .rows
            .iter()
            .any(|row| matches!(row.choice, Choice::Remote(_)));
        let details = if inner.height >= 9 {
            if self.mode() == Mode::Agents {
                // Workspace, provider, task, metadata and Copy ID have separate rows.
                5 + u16::from(has_remote)
            } else if has_remote {
                5
            } else if self.mode() == Mode::Workspaces {
                // Grouping, the exact name and its directory have separate rows.
                4
            } else {
                3
            }
        } else {
            0
        };
        let count = usize::from(inner.height.saturating_sub(2 + details));
        let window = self.picker_window(count);
        let list = Rect::new(inner.x, inner.y, inner.width, count as u16);
        for (index, row) in self
            .visible_rows()
            .iter()
            .enumerate()
            .skip(window.top)
            .take(count)
        {
            let enabled = self.workspace_row_available(row);
            let rect = Rect::new(
                inner.x,
                inner.y + (index - window.top) as u16,
                inner.width,
                1,
            );
            let name = if row.choice == Choice::Local {
                locale.machine_picker_text("local")
            } else {
                &display_label(&row.label)
            };
            let prefix = if has_remote {
                format!("{name} / ")
            } else {
                String::new()
            };
            let mut identity_suffix = String::new();
            let name = if let Some(agent) = row.agent() {
                // The name identifies the Agent; a distinct task title says what it is doing.
                let identity = display_label(agent.display_name());
                let title = display_label(agent.title());
                let title = if title.trim().is_empty() || title == identity {
                    String::new()
                } else {
                    format!(" · {title}")
                };
                format!(
                    "{}{prefix}{identity}{title}",
                    if index < 9 {
                        format!("{} ", index + 1)
                    } else {
                        String::new()
                    },
                )
            } else if let Some(context) = row.context() {
                identity_suffix = self.workspace_identity_suffix(row);
                format!(
                    "{prefix}{}",
                    context
                        .workspace
                        .as_deref()
                        .unwrap_or(locale.machine_picker_text("no-workspace")),
                )
            } else {
                name.to_owned()
            };
            let suffix = if let Some(status) = row.agent_status() {
                locale.machine_picker_text(status).to_owned()
            } else if let Some(agent) = row.agent() {
                let fallback = locale.machine_picker_text(match agent.display_state() {
                    orchbee_workbench_protocol::MetadataState::Idle => "agent-idle",
                    orchbee_workbench_protocol::MetadataState::Done => "agent-done",
                    orchbee_workbench_protocol::MetadataState::Error => "agent-error",
                    orchbee_workbench_protocol::MetadataState::Working => "agent-working",
                    orchbee_workbench_protocol::MetadataState::Blocked => "agent-blocked",
                    orchbee_workbench_protocol::MetadataState::Unknown => "agent-unknown",
                });
                // Sanitize the owned label before it enters a terminal render widget.
                agent
                    .state_label()
                    .map(display_label)
                    .unwrap_or_else(|| fallback.into())
            } else if !enabled {
                locale.machine_picker_text("disabled").to_owned()
            } else if row.context().is_some() && self.space_checkpoint_stale(row) {
                locale.machine_picker_text("stale").to_owned()
            } else if let Some(observation) = &row.observation {
                locale.machine_picker_text(observation.key()).to_owned()
            } else if row.choice == self.current {
                locale.machine_picker_text("current").to_owned()
            } else {
                String::new()
            };
            let status_width = match self.mode() {
                Mode::Machines => rect.width.min(16),
                Mode::Agents => rect.width.min(18),
                Mode::Workspaces => 0,
            }
            .min(rect.width.saturating_sub(4));
            let current_rect = Rect::new(rect.x, rect.y, 1, 1);
            let navigation_rect = Rect::new(rect.x + 1, rect.y, 1, 1);
            let operation_rect = Rect::new(rect.right() - 1, rect.y, 1, 1);
            let primary_width = rect
                .width
                .saturating_sub(2)
                .saturating_sub(status_width)
                .saturating_sub(1);
            let primary_rect = Rect::new(rect.x + 2, rect.y, primary_width, 1);
            // As in the Sidebar, the identity suffix is budgeted first and only the name truncates.
            let name = if identity_suffix.is_empty() {
                name
            } else {
                let budget =
                    usize::from(primary_width).saturating_sub(display_width(&identity_suffix));
                format!(
                    "{}{identity_suffix}",
                    truncate_display(&name, budget, theme.glyphs.ellipsis)
                )
            };
            let status_rect = Rect::new(primary_rect.right(), rect.y, status_width, 1);
            let current = row
                .agent()
                .is_some_and(|a| self.current_agent(&row.choice, a))
                || (self.mode() != Mode::Agents
                    && row.choice == self.current
                    && row
                        .context()
                        .is_none_or(|c| self.current_context.as_ref() == Some(&c.target)));
            let Some((target, spec)) = picker_interaction(&source, PickerAction::Row(index)) else {
                continue;
            };
            let pointer = pointer_presentation(hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                spec,
            ));
            let primary_role = if enabled {
                Role::TextPrimary
            } else {
                Role::TextDisabled
            };
            frame.render_widget(
                Paragraph::new(if current {
                    theme.glyphs.active_marker
                } else {
                    " "
                })
                .style(theme.style(if current {
                    Role::AccentSelection
                } else {
                    Role::TextSecondary
                })),
                current_rect,
            );
            frame.render_widget(
                Paragraph::new(name).style(theme.style(primary_role)),
                primary_rect,
            );
            let status_role = if !enabled {
                Role::TextDisabled
            } else if row.agent().is_some_and(|agent| {
                matches!(
                    agent.display_state(),
                    orchbee_workbench_protocol::MetadataState::Error
                        | orchbee_workbench_protocol::MetadataState::Blocked
                )
            }) || matches!(
                row.observation.as_ref(),
                Some(Observation::Attention | Observation::Stale)
            ) {
                Role::StatusWarning
            } else if row.agent().is_some_and(|agent| {
                agent.display_state() == orchbee_workbench_protocol::MetadataState::Working
            }) || matches!(
                row.observation.as_ref(),
                Some(Observation::Waiting | Observation::Connecting | Observation::Reconnecting)
            ) {
                Role::StatusWorking
            } else if matches!(row.observation.as_ref(), Some(Observation::Online(_))) {
                Role::StatusSuccess
            } else {
                Role::TextSecondary
            };
            frame.render_widget(
                Paragraph::new(suffix).style(theme.style(status_role)),
                status_rect,
            );
            let availability = if enabled {
                Availability::Enabled
            } else {
                Availability::Disabled
            };
            let state = component_state(
                pointer,
                if index == self.selected() {
                    NavigationState::Provisional
                } else {
                    NavigationState::None
                },
                if current {
                    CurrentState::Current
                } else {
                    CurrentState::NotCurrent
                },
                CheckedState::Unchecked,
                availability,
                if self.busy && index == self.selected() {
                    OperationState::Pending
                } else {
                    OperationState::Idle
                },
            );
            let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary_rect)
                .with_primary_role(primary_role)
                .with_provisional(navigation_rect)
                .with_status(operation_rect);
            let mut bands = vec![RowContentBand::semantic(
                current_rect,
                if current {
                    Role::AccentSelection
                } else {
                    Role::TextSecondary
                },
            )];
            if !status_rect.is_empty() {
                bands.push(RowContentBand::semantic(status_rect, status_role));
            }
            if paint_row_interaction(
                frame,
                rect,
                tracks,
                &bands,
                ControlSurface::Overlay,
                state,
                theme,
            )
            .is_some()
            {
                hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
            }
        }
        if let Some(list_source) = source.drawn_list(window) {
            hits.record(
                HitLayer::Overlay,
                list,
                HitTarget::ListScroll(ListScrollTarget::MachinePicker(list_source)),
            );
        }
        if self.mode() == Mode::Workspaces && self.workspace_matches().is_empty() && count > 0 {
            frame.render_widget(
                Paragraph::new(locale.machine_picker_text(
                    if self.workspace_query().text().is_empty() {
                        "no-workspaces"
                    } else {
                        "workspaces-no-matches"
                    },
                ))
                .style(theme.style(Role::TextSecondary)),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
        }
        if details > 0 {
            let online: Vec<_> = self
                .rows
                .iter()
                .filter(|row| matches!(row.choice, Choice::Remote(_)))
                .filter_map(|r| match &r.observation {
                    Some(Observation::Online(o)) => Some(o),
                    _ => None,
                })
                .collect();
            let mut lines = Vec::new();
            if has_remote {
                lines.push(locale.machine_overview(
                    online.len(),
                    online.iter().map(|o| o.workspaces).sum(),
                    online.iter().map(|o| o.tabs).sum(),
                ));
            }
            if let Some(agent) = self
                .visible_rows()
                .get(self.selected())
                .and_then(|r| r.agent())
            {
                lines.push(if agent.workspace.is_empty() {
                    locale.machine_picker_text("no-workspace").into()
                } else {
                    agent.workspace.clone()
                });
                lines.push(display_label(agent.provider()));
                lines.push(display_label(agent.title()));
                lines.push(
                    agent
                        .tokens
                        .as_map()
                        .iter()
                        .take(3)
                        .map(|(k, v)| {
                            format!(
                                "{}: {}",
                                display_label(k.as_str()),
                                display_label(v.as_str())
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · "),
                );
            } else if let Some(row) = self.visible_rows().get(self.selected())
                && let Some(context) = row.context()
            {
                match (&context.project, &context.workspace) {
                    (Some(project), _) => lines.push(project.clone()),
                    (None, Some(_)) => {
                        lines.push(locale.machine_picker_text("ungrouped").to_owned());
                    }
                    (None, None) => {}
                }
                let width = usize::from(inner.width);
                lines.push(match &context.workspace {
                    Some(workspace) => {
                        let suffix = self.workspace_identity_suffix(row);
                        let budget = width.saturating_sub(display_width(&suffix));
                        format!(
                            "{}{suffix}",
                            truncate_display(workspace, budget, theme.glyphs.ellipsis)
                        )
                    }
                    None => locale.machine_picker_text("no-workspace").to_owned(),
                });
                // The directory explains which checkout a same-named result opens; its
                // distinguishing tail survives truncation.
                if let Some(directory) = self
                    .space_checkpoint_row(row)
                    .and_then(|checkpoint| checkpoint.directory.as_ref())
                {
                    lines.push(truncate_display_start(
                        &sanitize_terminal_text(directory.path.as_str()),
                        width,
                        theme.glyphs.ellipsis,
                    ));
                }
            } else if let Some(Observation::Online(o)) = self
                .visible_rows()
                .get(self.selected())
                .and_then(|r| r.observation.as_ref())
            {
                lines.push(locale.machine_details(o.projects, o.workspaces, o.tabs));
                lines.extend(o.labels.iter().take(3).map(|label| format!("  {label}")));
            }
            if self.row_id(self.selected()).is_some() {
                let label = locale.machine_picker_text("copy-id");
                let copy_width = display_width(label) as u16;
                if copy_width <= inner.width {
                    let rect = Rect::new(
                        inner.right() - copy_width,
                        inner.bottom() - 3,
                        copy_width,
                        1,
                    );
                    if let Some((target, spec)) =
                        picker_interaction(&source, PickerAction::CopyId(self.selected()))
                    {
                        let hovered =
                            hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec)
                                != ProjectedPointer::Rest;
                        frame.render_widget(
                            Paragraph::new(label).style(theme.style(Role::HintKey).add_modifier(
                                if hovered {
                                    Modifier::REVERSED
                                } else {
                                    Modifier::UNDERLINED
                                },
                            )),
                            rect,
                        );
                        hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
                    }
                }
            }
            frame.render_widget(
                Paragraph::new(lines.join("\n")).style(theme.style(Role::TextSecondary)),
                Rect::new(
                    inner.x,
                    inner.bottom() - 2 - details,
                    inner.width,
                    details.saturating_sub(1),
                ),
            );
        }
        let status = self
            .error
            .map(|error| (locale.machine_picker_text(error), Role::StatusWarning));
        let status = status.or_else(|| {
            self.busy.then(|| {
                (
                    locale.machine_picker_text("connecting"),
                    Role::StatusPending,
                )
            })
        });
        if let Some((status, role)) = status {
            frame.render_widget(
                Paragraph::new(status).style(theme.style(role)),
                Rect::new(inner.x, inner.bottom() - 2, inner.width, 2),
            );
        }
    }

    fn render_agent_action(
        &self,
        frame: &mut Frame<'_>,
        action: AgentAction<'_>,
        theme: &Theme,
        source: &PickerSource,
        hits: &mut HitMapBuilder,
    ) -> Option<Rect> {
        let AgentAction {
            area,
            control,
            label,
            kind,
        } = action;
        let (target, interaction) = picker_interaction(source, PickerAction::Control(control))?;
        let state = component_state(
            PointerPresentation::Rest,
            NavigationState::None,
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        );
        let base = ActionSpec::new(label, kind, ControlSurface::Overlay, state);
        let rect = action_rect(frame.area(), area, base, theme)?;
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &target,
            interaction,
        ));
        let painted = render_action(
            frame,
            rect,
            ActionSpec::new(
                label,
                kind,
                ControlSurface::Overlay,
                component_state(
                    pointer,
                    NavigationState::None,
                    CurrentState::NotCurrent,
                    CheckedState::Unchecked,
                    Availability::Enabled,
                    OperationState::Idle,
                ),
            ),
            theme,
        )?;
        hits.record_interaction(HitLayer::Overlay, painted, painted, target, interaction);
        Some(painted)
    }

    fn render_agent_controls(
        &self,
        frame: &mut Frame<'_>,
        inner: &mut Rect,
        locale: Locale,
        theme: &Theme,
        source: &PickerSource,
        hits: &mut HitMapBuilder,
    ) {
        if self.mode() != Mode::Agents {
            return;
        }
        if inner.height < 5 {
            if self.editing_query() && inner.height >= 3 {
                self.render_agent_query(
                    frame,
                    Rect::new(inner.x, inner.y, inner.width, 1),
                    locale,
                    theme,
                    source,
                    hits,
                );
                inner.y += 1;
                inner.height -= 1;
            }
            return;
        }
        for (index, control) in [Control::State, Control::Order, Control::Scope]
            .into_iter()
            .enumerate()
        {
            let width = inner.width / 3;
            let rect = Rect::new(inner.x + index as u16 * width, inner.y, width, 1);
            let label = if control == Control::Order
                && self.source_spec().is_some_and(|s| !s.sort.is_empty())
            {
                locale.machine_picker_text("agent-source-sort").to_owned()
            } else {
                locale.machine_agent_control(control_label(control, self.agent_view()))
            };
            let label = crate::terminal_text::truncate_display(
                &label,
                usize::from(width.saturating_sub(5)),
                theme.glyphs.ellipsis,
            );
            let _ = self.render_agent_action(
                frame,
                AgentAction {
                    area: rect,
                    control,
                    label: &label,
                    kind: ActionKind::Secondary,
                },
                theme,
                source,
                hits,
            );
        }
        let rect = Rect::new(inner.x, inner.y + 1, inner.width, 1);
        self.render_agent_query(frame, rect, locale, theme, source, hits);
        inner.y += 2;
        inner.height -= 2;
    }

    fn render_agent_query(
        &self,
        frame: &mut Frame<'_>,
        mut rect: Rect,
        locale: Locale,
        theme: &Theme,
        source: &PickerSource,
        hits: &mut HitMapBuilder,
    ) {
        for control in [Control::Clear, Control::Done] {
            if control == Control::Done && !self.editing_query() {
                continue;
            }
            let label = locale.machine_agent_control(control_label(control, self.agent_view()));
            let state = component_state(
                PointerPresentation::Rest,
                NavigationState::None,
                CurrentState::NotCurrent,
                CheckedState::Unchecked,
                Availability::Enabled,
                OperationState::Idle,
            );
            let spec = ActionSpec::new(
                &label,
                ActionKind::Secondary,
                ControlSurface::Overlay,
                state,
            );
            let Some(measured) = action_rect(frame.area(), rect, spec, theme) else {
                continue;
            };
            if rect.width <= measured.width.saturating_add(8) {
                continue;
            }
            let area = Rect::new(
                rect.right().saturating_sub(measured.width),
                rect.y,
                measured.width,
                1,
            );
            if let Some(area) = self.render_agent_action(
                frame,
                AgentAction {
                    area,
                    control,
                    label: &label,
                    kind: ActionKind::Secondary,
                },
                theme,
                source,
                hits,
            ) {
                rect.width = area.x.saturating_sub(rect.x).saturating_sub(1);
            }
        }
        let label = locale.machine_picker_text("agent-search");
        let query = self.agent_view().query.as_str();
        let key = search_key_label(theme);
        let field = if self.editing_query() {
            QueryField::Editing {
                label,
                query,
                cursor: query.len(),
            }
        } else if query.is_empty() {
            QueryField::Rest {
                key: &key,
                prompt: locale.machine_picker_text("agent-search-prompt"),
            }
        } else {
            QueryField::Retained { label, query }
        };
        let mut field_rect = rect;
        if !self.editing_query()
            && let Some(spec) = self.source_spec()
        {
            let view = format!(
                "{} {}: {}",
                theme.glyphs.middle_dot,
                locale.machine_picker_text("agent-source"),
                crate::machine_discovery::display_label(
                    spec.label.as_deref().unwrap_or(&spec.source)
                )
            );
            let width = u16::try_from(display_width(&view)).unwrap_or(u16::MAX);
            if rect.width > width.saturating_add(17) {
                field_rect.width -= width + 1;
                frame.render_widget(
                    Paragraph::new(view).style(theme.style(Role::HintLabel)),
                    Rect::new(field_rect.right() + 1, rect.y, width, 1),
                );
            }
        }
        let _ = render_query_field(frame, field_rect, field, theme);
        let Some((target, interaction)) =
            picker_interaction(source, PickerAction::Control(Control::Search))
        else {
            return;
        };
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &target,
            interaction,
        ));
        let primary_role = if self.editing_query() {
            Role::TextPrimary
        } else {
            Role::HintLabel
        };
        if paint_row_interaction(
            frame,
            rect,
            RowTracks::new(RowFocusCue::EnclosingFrame, rect).with_primary_role(primary_role),
            &[],
            ControlSurface::Overlay,
            component_state(
                pointer,
                NavigationState::None,
                CurrentState::NotCurrent,
                CheckedState::Unchecked,
                Availability::Enabled,
                OperationState::Idle,
            ),
            theme,
        )
        .is_some()
        {
            hits.record_interaction(HitLayer::Overlay, rect, rect, target, interaction);
        }
    }

    fn render_workspace_query(
        &self,
        frame: &mut Frame<'_>,
        inner: &mut Rect,
        locale: Locale,
        theme: &Theme,
        source: &PickerSource,
        hits: &mut HitMapBuilder,
    ) {
        if self.mode() != Mode::Workspaces || inner.height < 4 {
            return;
        }
        let mut field = Rect::new(inner.x, inner.y, inner.width, 1);
        let clear_label = locale.machine_picker_text("picker-clear-hint");
        let clear_width = u16::try_from(display_width(clear_label) + 4).unwrap_or(u16::MAX);
        if field.width > clear_width.saturating_add(16) {
            let area = Rect::new(field.right() - clear_width, field.y, clear_width, 1);
            if let Some(painted) = self.render_agent_action(
                frame,
                AgentAction {
                    area,
                    control: Control::Clear,
                    label: clear_label,
                    kind: ActionKind::Secondary,
                },
                theme,
                source,
                hits,
            ) {
                field.width = painted.x.saturating_sub(field.x).saturating_sub(1);
            }
        }
        // Typing in Workspaces mode always reaches this query, so it keeps its editing form.
        let _ = render_query_field(
            frame,
            field,
            QueryField::Editing {
                label: locale.machine_picker_text("workspace-search"),
                query: self.workspace_query().text(),
                cursor: self.workspace_query().cursor(),
            },
            theme,
        );
        if let Some((target, interaction)) =
            picker_interaction(source, PickerAction::Control(Control::Search))
        {
            hits.record_interaction(HitLayer::Overlay, field, field, target, interaction);
        }
        inner.y += 1;
        inner.height -= 1;
    }
}

fn control_label(
    control: Control,
    view: &crate::machine_agents::view::View,
) -> crate::localization::MachineControlLabel {
    use crate::machine_agents::view::{Order, Scope};
    use orchbee_workbench_protocol::MetadataState as State;
    match control {
        Control::Done => crate::localization::MachineControlLabel::Done,
        Control::Clear => crate::localization::MachineControlLabel::Clear,
        Control::State => match view.state {
            None => crate::localization::MachineControlLabel::AllStates,
            Some(State::Blocked) => crate::localization::MachineControlLabel::Blocked,
            Some(State::Done) => crate::localization::MachineControlLabel::DoneState,
            Some(State::Error) => crate::localization::MachineControlLabel::Error,
            Some(State::Working) => crate::localization::MachineControlLabel::Working,
            Some(State::Idle) => crate::localization::MachineControlLabel::Idle,
            Some(State::Unknown) => crate::localization::MachineControlLabel::Unknown,
        },
        Control::Scope => match view.scope {
            Scope::All => crate::localization::MachineControlLabel::AllScope,
            Scope::Machine => crate::localization::MachineControlLabel::MachineScope,
            Scope::Context => crate::localization::MachineControlLabel::ContextScope,
        },
        Control::Order => match view.order {
            Order::Placement => crate::localization::MachineControlLabel::PlacementOrder,
            Order::Priority => crate::localization::MachineControlLabel::PriorityOrder,
            Order::Provider => crate::localization::MachineControlLabel::ProviderOrder,
            Order::Title => crate::localization::MachineControlLabel::TitleOrder,
            Order::Sequence => crate::localization::MachineControlLabel::SequenceOrder,
        },
        Control::Search => crate::localization::MachineControlLabel::Search,
    }
}
