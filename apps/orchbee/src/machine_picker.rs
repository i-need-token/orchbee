//! Identity-based machine selection and painted input ownership; no storage or network effects.
use crate::{
    client_view::{KeyCode, KeyEvent, KeyKind, Modifiers},
    localization::Locale,
    machine_discovery::{Observation, display_label},
    machines::{Profile, ProfileId},
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Choice {
    Local,
    Remote(ProfileId),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) choice: Choice,
    pub(crate) label: String,
    pub(crate) enabled: bool,
    pub(crate) observation: Option<Observation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Picker {
    pub(crate) available: bool,
    pub(crate) open: bool,
    pub(crate) current: Choice,
    pub(crate) current_label: String,
    pub(crate) rows: Vec<Row>,
    pub(crate) busy: bool,
    pub(crate) error: Option<&'static str>,
    pub(crate) request: Option<Choice>,
    selected: usize,
    top: usize,
    painted: Vec<(Rect, Choice)>,
    indicator: Option<Rect>,
}
impl Default for Picker {
    fn default() -> Self {
        Self {
            available: false,
            open: false,
            current: Choice::Local,
            current_label: String::new(),
            rows: vec![Row {
                choice: Choice::Local,
                label: String::new(),
                enabled: true,
                observation: None,
            }],
            busy: false,
            error: None,
            request: None,
            selected: 0,
            top: 0,
            painted: Vec::new(),
            indicator: None,
        }
    }
}
impl Picker {
    pub(crate) fn replace(&mut self, profiles: &[Profile]) {
        let selected = self.rows.get(self.selected).map(|r| r.choice.clone());
        let mut rows = vec![Row {
            choice: Choice::Local,
            label: String::new(),
            enabled: true,
            observation: None,
        }];
        rows.extend(profiles.iter().map(|p| {
            Row {
                choice: Choice::Remote(p.id.clone()),
                label: p.draft.label.clone(),
                enabled: p.enabled,
                observation: p
                    .enabled
                    .then(|| {
                        self.rows
                            .iter()
                            .find(|r| r.choice == Choice::Remote(p.id.clone()))
                            .and_then(|r| r.observation.clone())
                    })
                    .flatten(),
            }
        }));
        if rows != self.rows {
            self.rows = rows;
            self.selected = selected
                .and_then(|id| self.rows.iter().position(|r| r.choice == id))
                .unwrap_or(0);
            self.invalidate();
        }
    }
    pub(crate) fn observe(&mut self, observations: &[(ProfileId, Observation)]) -> bool {
        let mut changed = false;
        for row in &mut self.rows {
            let next = if row.enabled {
                observations
                    .iter()
                    .find(|(id, _)| row.choice == Choice::Remote(id.clone()))
                    .map(|(_, o)| o.clone())
            } else {
                None
            };
            if row.observation != next {
                row.observation = next;
                changed = true;
            }
        }
        if changed {
            self.invalidate();
        }
        changed
    }
    pub(crate) fn show(&mut self) {
        self.open = true;
        self.error = None;
        self.selected = self
            .rows
            .iter()
            .position(|r| r.choice == self.current)
            .unwrap_or(0);
        self.invalidate();
    }
    pub(crate) fn invalidate(&mut self) {
        self.painted.clear();
        self.indicator = None;
    }
    /// All keys, paste and malformed events are consumed by the caller while open.
    pub(crate) fn key(&mut self, key: KeyEvent) {
        if key.kind == KeyKind::Release {
            return;
        }
        let (code, mods) = key.normalized();
        if code == KeyCode::Char('m') && mods == Modifiers::CTRL {
            self.choose(self.selected);
            return;
        }
        if mods != Modifiers::NONE {
            return;
        }
        match code {
            KeyCode::Esc => {
                self.open = false;
                self.request = None;
                self.invalidate();
            }
            KeyCode::Up | KeyCode::Char('k') => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1))
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.rows.len().saturating_sub(1),
            KeyCode::Enter => self.choose(self.selected),
            _ => {}
        }
    }
    fn choose(&mut self, index: usize) {
        if let Some(row) = self.rows.get(index) {
            if row.enabled {
                self.request = Some(row.choice.clone());
                self.error = None;
            } else {
                self.error = Some("disabled");
            }
        }
    }
    /// Returns whether this drawn surface owns the pointer packet.
    pub(crate) fn pointer(&mut self, event: MouseEvent) -> bool {
        let point = (event.column, event.row).into();
        if !self.open {
            if self.indicator.is_some_and(|r| r.contains(point)) {
                if event.action == MouseAction::Press && event.button == MouseButton::Left {
                    self.show();
                }
                return true;
            }
            return false;
        }
        if event.action == MouseAction::Press && event.button == MouseButton::WheelUp {
            self.selected = self.selected.saturating_sub(1);
        }
        if event.action == MouseAction::Press && event.button == MouseButton::WheelDown {
            self.selected = (self.selected + 1).min(self.rows.len().saturating_sub(1));
        }
        if event.action == MouseAction::Press
            && event.button == MouseButton::Left
            && let Some((_, choice)) = self.painted.iter().find(|(r, _)| r.contains(point))
            && let Some(index) = self.rows.iter().position(|r| &r.choice == choice)
        {
            self.selected = index;
            self.choose(index);
        }
        true
    }
    pub(crate) fn render(&mut self, frame: &mut Frame<'_>, locale: Locale, theme: &Theme) {
        self.painted.clear();
        if !self.available {
            self.indicator = None;
            return;
        }
        let area = frame.area();
        if area.width == 0 || area.height == 0 {
            self.indicator = None;
            return;
        }
        let label = if self.current == Choice::Local {
            locale.machine_picker_text("local")
        } else {
            &display_label(&self.current_label)
        };
        let width = area.width.min(28);
        let indicator = Rect::new(area.right() - width, area.bottom() - 1, width, 1);
        frame.render_widget(Clear, indicator);
        frame.render_widget(
            Paragraph::new(format!(" ◇ {label} ▾ ")).style(theme.style(Role::AccentFocus)),
            indicator,
        );
        self.indicator = Some(indicator);
        if !self.open {
            return;
        }
        let width = area.width.saturating_sub(4).min(72);
        let height = area
            .height
            .saturating_sub(3)
            .min((self.rows.len() + 9).min(24) as u16);
        if width < 34 || height < 5 {
            frame.render_widget(
                Paragraph::new(locale.machine_picker_text("compact"))
                    .style(theme.style(Role::HintLabel)),
                indicator,
            );
            return;
        }
        let panel = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, panel);
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_type(theme.corners)
                .border_style(theme.style(Role::FrameFocused))
                .style(theme.style(Role::SurfaceOverlay))
                .title(format!(
                    "{} {}/{} ",
                    locale.machine_picker_text("title"),
                    self.selected + 1,
                    self.rows.len()
                )),
            panel,
        );
        let inner = panel.inner(ratatui::layout::Margin::new(1, 1));
        if inner.height < 2 || inner.width == 0 {
            return;
        }
        let details = if inner.height >= 9 { 5 } else { 0 };
        let count = usize::from(inner.height.saturating_sub(2 + details));
        if self.selected < self.top {
            self.top = self.selected;
        }
        if self.selected >= self.top + count && count > 0 {
            self.top = self.selected + 1 - count;
        }
        for (index, row) in self.rows.iter().enumerate().skip(self.top).take(count) {
            let rect = Rect::new(inner.x, inner.y + (index - self.top) as u16, inner.width, 1);
            let name = if row.choice == Choice::Local {
                locale.machine_picker_text("local")
            } else {
                &display_label(&row.label)
            };
            let suffix = if !row.enabled {
                locale.machine_picker_text("disabled")
            } else if let Some(observation) = &row.observation {
                locale.machine_picker_text(observation.key())
            } else if row.choice == self.current {
                locale.machine_picker_text("current")
            } else {
                ""
            };
            let id = match &row.choice {
                Choice::Local => String::new(),
                Choice::Remote(id) => {
                    let width = (8..=32).step_by(4).find(|width| self.rows.iter().filter(|r| matches!(&r.choice, Choice::Remote(other) if other.as_str()[..*width] == id.as_str()[..*width])).count() == 1).unwrap_or(32);
                    format!("{} · ", &id.as_str()[..width])
                }
            };
            let style = if index == self.selected {
                theme
                    .style(Role::SurfaceSelection)
                    .patch(theme.style(Role::TextInverse))
            } else if !row.enabled {
                theme.style(Role::TextDisabled)
            } else {
                theme.style(Role::TextPrimary)
            };
            let status_width = rect.width.min(16);
            let name_rect = Rect::new(rect.x, rect.y, rect.width.saturating_sub(status_width), 1);
            let status_rect = Rect::new(name_rect.right(), rect.y, status_width, 1);
            let current = if row.choice == self.current {
                "•"
            } else {
                " "
            };
            frame.render_widget(
                Paragraph::new(format!("{current}{id}{name}")).style(style),
                name_rect,
            );
            frame.render_widget(Paragraph::new(suffix).style(style), status_rect);
            self.painted.push((rect, row.choice.clone()));
        }
        if details > 0 {
            let online: Vec<_> = self
                .rows
                .iter()
                .filter_map(|r| match &r.observation {
                    Some(Observation::Online(o)) => Some(o),
                    _ => None,
                })
                .collect();
            let mut lines = vec![locale.machine_overview(
                online.len(),
                online.iter().map(|o| o.workspaces).sum(),
                online.iter().map(|o| o.tabs).sum(),
            )];
            if let Some(Observation::Online(o)) = self
                .rows
                .get(self.selected)
                .and_then(|r| r.observation.as_ref())
            {
                lines.push(locale.machine_details(o.projects, o.workspaces, o.tabs));
                lines.extend(o.labels.iter().take(3).map(|label| format!("  {label}")));
            }
            frame.render_widget(
                Paragraph::new(lines.join("\n")).style(theme.style(Role::TextSecondary)),
                Rect::new(inner.x, inner.bottom() - 2 - details, inner.width, details),
            );
        }
        let status = self
            .error
            .map(|e| locale.machine_picker_text(e))
            .unwrap_or_else(|| {
                if self.busy {
                    locale.machine_picker_text("connecting")
                } else {
                    locale.machine_picker_text("hint")
                }
            });
        frame.render_widget(
            Paragraph::new(status).style(theme.style(if self.error.is_some() {
                Role::StatusWarning
            } else {
                Role::HintLabel
            })),
            Rect::new(inner.x, inner.bottom() - 2, inner.width, 2),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines::Draft;
    fn profile(n: u8, label: &str, enabled: bool) -> Profile {
        Profile {
            id: ProfileId::parse(&format!("{n:032x}")).unwrap(),
            draft: Draft::parse(label, "example.invalid", "default").unwrap(),
            enabled,
        }
    }
    fn paint(picker: &mut Picker, locale: Locale, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| picker.render(frame, locale, &Theme::terminal_native()))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect()
    }
    fn click(rect: Rect) -> MouseEvent {
        MouseEvent {
            action: MouseAction::Press,
            button: MouseButton::Left,
            mods: Modifiers::NONE,
            column: rect.x,
            row: rect.y,
        }
    }
    #[test]
    fn rendered_rows_select_exact_ids_and_reordering_invalidates_old_hits() {
        let a = profile(1, "same", true);
        let b = profile(2, "same", true);
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[a.clone(), b.clone()]);
        picker.show();
        let text = paint(&mut picker, Locale::EnUs, 100, 30);
        assert!(text.contains("Machines"));
        let rect = picker.painted[2].0;
        picker.pointer(click(rect));
        assert_eq!(picker.request.take(), Some(Choice::Remote(b.id.clone())));
        picker.replace(&[b.clone(), a]);
        picker.pointer(click(rect));
        assert!(picker.request.is_none());
        assert!(
            paint(&mut picker, Locale::ZhCn, 100, 30)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .contains("机器")
        );
        picker.pointer(click(picker.painted[1].0));
        assert_eq!(picker.request.take(), Some(Choice::Remote(b.id)));
    }
    #[test]
    fn disabled_selection_and_small_resize_never_fall_through_to_a_terminal() {
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(&[profile(1, "offline", false)]);
        picker.show();
        paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(picker.pointer(click(picker.painted[1].0)));
        assert_eq!(picker.error, Some("disabled"));
        assert!(picker.request.is_none());
        picker.invalidate();
        paint(&mut picker, Locale::ZhCn, 1, 1);
        assert!(picker.painted.is_empty());
        assert!(picker.pointer(click(Rect::new(0, 0, 1, 1))));
    }
    #[test]
    fn machine_overview_keeps_status_visible_and_localizes_without_claiming_stale_online() {
        let p = profile(1, &"long-name".repeat(10), true);
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.replace(std::slice::from_ref(&p));
        picker.show();
        picker.selected = 1;
        let overview = crate::machine_discovery::Overview {
            site: orchbee_workbench_protocol::SiteId::parse(p.id.as_str()).unwrap(),
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            projects: 2,
            workspaces: 3,
            tabs: 7,
            labels: vec!["Project A".into(), "Workspace B".into()],
        };
        picker.observe(&[(p.id.clone(), Observation::Online(overview))]);
        let text = paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(text.contains("Online"));
        assert!(text.contains("3 workspaces"));
        assert!(text.contains("Project A"));
        let text = paint(&mut picker, Locale::ZhCn, 80, 24);
        assert!(
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .contains("在线")
        );
        let rect = picker.painted[1].0;
        picker.observe(&[(p.id, Observation::Stale)]);
        assert!(picker.pointer(click(rect)));
        assert!(
            picker.request.is_none(),
            "old painted rows are invalid after a status update"
        );
        let text = paint(&mut picker, Locale::EnUs, 80, 24);
        assert!(text.contains("Stale"));
        assert!(text.contains("0 machines"));
        assert!(!text.contains("Workspace B"));
    }
}
