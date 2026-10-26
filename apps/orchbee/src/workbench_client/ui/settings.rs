//! The UI owns drafts and confirmed observations; the ordered lane never replays a save.
use super::{ClientActionDisposition, ModalPresentation, NativeUi, WorkbenchClientView};
use crate::{
    client_view::KeyEvent,
    native_settings::{
        Edit, Editor, Observation, Reconciliation, Row, SettingsDraw, SettingsInputContext,
        Submission, Token, View, key_edit, key_edit_for_view,
    },
    tui_presentation::{GeometrySettlement, PresentationPhase},
    workbench_client::settings_lane::{Job, Lane},
};
use orchbee_workbench_protocol::{
    NativeSettingsFailure, NativeSettingsOperation, NativeSettingsOutcome, NativeSettingsRequest,
    ProtocolVersion, SiteId, UserPreferencesSnapshot, WorkbenchRequestId,
};
use std::time::{Duration, Instant};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    Saved,
    NoChange,
    SourceChanged,
    Failed,
    Unknown,
    Busy,
    PresentationUnavailable,
    TooSmall,
    Background,
}
impl Notice {
    fn persistent(self) -> bool {
        !matches!(self, Self::Saved | Self::NoChange)
    }
}
#[derive(Default)]
pub(super) struct Coordinator {
    pub(super) editor: Editor,
    origin: Option<crate::local_client::LocalClientOrigin>,
    lane: Option<Lane>,
    site: Option<SiteId>,
    serial: u64,
    saving: bool,
    snapshot: Option<UserPreferencesSnapshot>,
    observed_source: Option<String>,
    started: Option<Instant>,
    pub(super) pending: bool,
    notice: Option<(Notice, Option<Instant>)>,
}
impl Coordinator {
    pub(super) fn set_origin(&mut self, origin: crate::local_client::LocalClientOrigin) {
        self.origin = Some(origin);
    }
    pub(super) fn close(&mut self) {
        self.editor.edit(Edit::Cancel);
        if let Some(mut lane) = self.lane.take() {
            lane.close();
        }
    }
    pub(super) fn clear_notice(&mut self) {
        self.notice = None;
    }
    pub(super) fn notice(&self, now: Instant) -> Option<Notice> {
        self.notice
            .filter(|(_, until)| until.is_none_or(|until| now < until))
            .map(|(notice, _)| notice)
    }
    fn note(&mut self, notice: Notice, now: Instant) {
        self.notice = Some((
            notice,
            if notice.persistent() {
                None
            } else {
                now.checked_add(Duration::from_secs(5))
            },
        ));
    }
    fn observe_snapshot(&mut self, snapshot: UserPreferencesSnapshot, now: Instant) -> bool {
        if self.observed_source.as_deref() == Some(&snapshot.source) {
            return false;
        }
        if self.editor.observe(&snapshot) == Observation::Conflict
            && self.notice(now) != Some(Notice::Unknown)
        {
            self.note(Notice::SourceChanged, now);
        }
        self.observed_source = Some(snapshot.source.clone());
        self.snapshot = Some(snapshot);
        true
    }
}
impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn settings(&self) -> Option<View<'_>> {
        self.settings.and_then(|settings| settings.editor.view())
    }
    /// Supplies presentation-only draft values to the whole-frame compositor.
    pub(crate) fn settings_appearance_preview(
        &self,
    ) -> Option<crate::native_settings::AppearancePreview<'a>> {
        self.settings?.editor.appearance_preview()
    }
    pub(crate) fn settings_input_context(&self) -> Option<SettingsInputContext> {
        self.settings?.editor.input_context()
    }
    pub(crate) fn settings_pending(&self) -> bool {
        self.settings.is_some_and(|settings| settings.pending)
    }
    pub(crate) fn settings_presentation(&self) -> Option<PresentationPhase> {
        let settings = self.settings?;
        settings.editor.view()?;
        Some(settings.editor.presentation())
    }
    pub(crate) fn settings_notice(&self) -> Option<Notice> {
        match self.command_notice {
            Some(super::WorkbenchClientNotice::Settings(notice)) => Some(notice),
            _ => None,
        }
    }
}
impl NativeUi {
    pub(in crate::workbench_client) fn scroll_settings(
        &mut self,
        token: Token,
        up: bool,
        lines: usize,
    ) -> Option<Token> {
        self.settings.editor.scroll(token, up, lines)
    }
    pub(in crate::workbench_client) fn apply_user_preferences(
        &mut self,
        values: &orchbee_workbench_protocol::UserPreferencesValues,
    ) {
        for projection in &mut self.projections {
            if let Some(selection) = &mut projection.selection {
                selection.set_copy_on_select(values.copy_on_select);
            }
        }
        self.preferences = values.clone();
        self.apply_notification_preferences();
    }
    pub(super) fn request_settings(&mut self) -> ClientActionDisposition {
        self.settings.clear_notice();
        let busy = self.modal_owns_input();
        #[cfg(windows)]
        let busy = busy
            || self.retirement_failure.is_some()
            || self.copy_mode_active()
            || self.popup.captured();
        if busy {
            self.settings.note(Notice::Busy, self.clock.now());
            return ClientActionDisposition::KeepNavigator;
        }
        if !self.ensure_settings_lane() {
            self.settings.note(Notice::Failed, self.clock.now());
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(token) = self.settings.editor.open() else {
            self.settings.note(Notice::Busy, self.clock.now());
            return ClientActionDisposition::KeepNavigator;
        };
        self.submit_settings(Submission {
            token,
            operation: NativeSettingsOperation::Open {},
        });
        ClientActionDisposition::DismissNavigator
    }
    fn ensure_settings_lane(&mut self) -> bool {
        let Some(site) = self
            .state
            .as_ref()
            .map(|state| state.cache().site_id().clone())
        else {
            return false;
        };
        if self.settings.site.as_ref().is_some_and(|old| old != &site) {
            let saving = self.settings.editor.saving();
            self.settings.close();
            self.settings.editor.retire();
            self.settings.started = None;
            self.settings.pending = false;
            self.settings.observed_source = None;
            self.settings.snapshot = None;
            if saving {
                self.settings.note(Notice::Unknown, self.clock.now());
            }
        }
        if self.settings.lane.is_none() {
            self.settings.lane = self
                .settings
                .origin
                .clone()
                .and_then(|origin| Lane::spawn(origin, self.clock.clone(), site.clone()).ok());
            self.settings.site = Some(site);
        }
        self.settings.lane.is_some()
    }
    fn submit_settings(&mut self, submission: Submission) {
        if matches!(submission.operation, NativeSettingsOperation::Open {}) {
            self.settings.clear_notice();
        }
        let token = submission.token;
        let Some(serial) = self.settings.serial.checked_add(1) else {
            self.fail_settings(token, Notice::Failed);
            return;
        };
        let Some(site) = self.settings.site.clone().filter(|site| {
            self.state
                .as_ref()
                .is_some_and(|state| state.cache().site_id() == site)
        }) else {
            self.fail_settings(token, Notice::Failed);
            return;
        };
        self.settings.serial = serial;
        self.settings.saving = matches!(submission.operation, NativeSettingsOperation::Save { .. });
        self.settings.started = Some(self.clock.now());
        self.settings.pending = false;
        let request = NativeSettingsRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                .expect("bounded Settings serial"),
            site_id: site,
            operation: submission.operation,
        };
        if !self
            .settings
            .lane
            .as_ref()
            .is_some_and(|lane| lane.submit(Job { token, request }))
        {
            self.fail_settings(token, Notice::Failed);
        }
    }
    fn fail_settings(&mut self, token: Token, notice: Notice) {
        let reconciliation = match notice {
            Notice::SourceChanged => Some(Reconciliation::SourceChanged),
            Notice::Unknown => Some(Reconciliation::OutcomeUnknown),
            _ => None,
        };
        if self.settings.editor.failed(token, reconciliation) {
            self.settings.started = None;
            self.settings.pending = false;
            self.settings.note(notice, self.clock.now());
        }
    }
    pub(super) fn drive_settings(&mut self) -> bool {
        self.ensure_settings_lane();
        let expired = self
            .settings
            .notice
            .is_some_and(|(_, until)| until.is_some_and(|until| self.clock.now() >= until));
        if expired {
            self.settings.notice = None;
        }
        let pending = matches!(
            self.settings.editor.view(),
            Some(View::Loading { .. } | View::Saving { .. })
        ) && self.settings.started.is_some_and(|started| {
            self.clock.now().saturating_duration_since(started) >= Duration::from_millis(200)
        });
        let mut changed = expired || pending != self.settings.pending;
        self.settings.pending = pending;
        if let Some(snapshot) = self.settings.lane.as_ref().and_then(Lane::take_observation) {
            changed |= self.settings.observe_snapshot(snapshot, self.clock.now());
        }
        let Some(reply) = self.settings.lane.as_ref().and_then(Lane::take) else {
            return changed;
        };
        let token = reply.token;
        self.settings.pending = false;
        self.settings.started = None;
        match reply.result {
            Ok(reply) if self.settings.site.as_ref() == Some(&reply.site_id) => match reply.outcome
            {
                NativeSettingsOutcome::Opened { snapshot } => {
                    self.settings.editor.loaded(token, snapshot);
                }
                NativeSettingsOutcome::Saved { changed, .. } => {
                    if self.settings.editor.complete(token) {
                        self.settings.note(
                            if changed {
                                Notice::Saved
                            } else {
                                Notice::NoChange
                            },
                            self.clock.now(),
                        );
                    }
                }
                NativeSettingsOutcome::OutcomeUnknown {} => {
                    self.fail_settings(token, Notice::Unknown)
                }
                NativeSettingsOutcome::Refused { reason } => self.fail_settings(
                    token,
                    match reason {
                        NativeSettingsFailure::SourceChanged => Notice::SourceChanged,
                        NativeSettingsFailure::Busy => Notice::Busy,
                        _ => Notice::Failed,
                    },
                ),
            },
            result => self.fail_settings(
                token,
                if self.settings.saving && !matches!(result, Err(false)) {
                    Notice::Unknown
                } else {
                    Notice::Failed
                },
            ),
        }
        true
    }
    pub(in crate::workbench_client) fn take_settings_snapshot(
        &mut self,
    ) -> Option<UserPreferencesSnapshot> {
        self.settings.snapshot.take()
    }
    pub(in crate::workbench_client) fn clear_settings_notice(&mut self) {
        self.settings.clear_notice();
    }
    pub(in crate::workbench_client) fn settings_active(&self) -> bool {
        self.settings.editor.visible()
    }
    pub(in crate::workbench_client) fn settings_input_context(
        &self,
    ) -> Option<SettingsInputContext> {
        self.view().settings_input_context()
    }
    /// Executes only against the exact Settings input snapshot selected by the compositor.
    ///
    /// `false` means that snapshot expired. The caller must discard the item rather than
    /// reprojecting it onto another input owner.
    pub(in crate::workbench_client) fn capture_settings_key_for(
        &mut self,
        expected: SettingsInputContext,
        event: KeyEvent,
    ) -> bool {
        if self.settings_input_context() != Some(expected) {
            return false;
        }
        if expected.readiness == crate::native_settings::SettingsInputReadiness::Interactive
            || matches!(key_edit(event, false), Some(Edit::Cancel))
        {
            self.capture_settings_key(event);
        }
        true
    }
    /// Applies paste only while the exact selected Settings surface remains interactive.
    ///
    /// Recovery capture consumes paste without mutating the retained workflow. An expired
    /// snapshot returns `false` and must still be discarded by the caller.
    pub(in crate::workbench_client) fn capture_settings_paste_for(
        &mut self,
        expected: SettingsInputContext,
        text: Option<&str>,
    ) -> bool {
        if self.settings_input_context() != Some(expected) {
            return false;
        }
        if expected.readiness == crate::native_settings::SettingsInputReadiness::Interactive {
            self.capture_settings_paste(text);
        }
        true
    }
    pub(in crate::workbench_client) fn capture_settings_key(&mut self, event: KeyEvent) {
        let edit = self
            .settings
            .editor
            .view()
            .and_then(|view| key_edit_for_view(event, view));
        if let Some(edit) = edit {
            let saving = matches!(self.settings.editor.view(), Some(View::Saving { .. }));
            if !matches!(
                self.settings.notice(self.clock.now()),
                Some(Notice::SourceChanged | Notice::Unknown | Notice::Failed)
            ) {
                self.settings.clear_notice();
            }
            if let Some(submission) = self.settings.editor.edit(edit) {
                self.submit_settings(submission);
            }
            if saving && matches!(edit, Edit::Cancel) {
                self.settings.note(Notice::Background, self.clock.now());
            }
        }
    }
    pub(in crate::workbench_client) fn capture_settings_paste(&mut self, text: Option<&str>) {
        if let Some(text) = text {
            self.settings.editor.paste(text);
        } else {
            self.settings.editor.reject_paste();
        }
    }
    pub(in crate::workbench_client) fn click_settings_control(
        &mut self,
        token: Token,
        control: crate::native_settings::Control,
    ) {
        let saving = matches!(self.settings.editor.view(), Some(View::Saving { token: current }) if current == token);
        if let Some(submission) = self.settings.editor.control(token, control) {
            self.submit_settings(submission);
        }
        if saving && !self.settings.editor.visible() {
            self.settings.note(Notice::Background, self.clock.now());
        }
    }
    pub(in crate::workbench_client) fn click_settings(&mut self, token: Token, row: Row) {
        if let Some(submission) = self.settings.editor.click(token, row) {
            self.submit_settings(submission);
        }
    }
    pub(super) fn after_settings_draw(&mut self, draw: Option<ModalPresentation<SettingsDraw>>) {
        match draw {
            Some(ModalPresentation::Presented(draw)) => {
                self.settings.editor.presented(draw);
                if self.settings.notice(self.clock.now()) == Some(Notice::TooSmall) {
                    self.settings.clear_notice();
                }
            }
            Some(ModalPresentation::NotPresented) => {
                match self.settings.editor.geometry_blocked() {
                    Some(GeometrySettlement::Refused) => self
                        .settings
                        .note(Notice::PresentationUnavailable, self.clock.now()),
                    Some(GeometrySettlement::Suspended) => {
                        self.settings.note(Notice::TooSmall, self.clock.now())
                    }
                    None => {}
                }
            }
            None => self.settings.editor.not_presented(),
        }
    }
}
