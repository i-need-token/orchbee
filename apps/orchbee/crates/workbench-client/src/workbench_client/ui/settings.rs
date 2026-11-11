//! The UI owns the Settings page and confirmed observations; the ordered lane never replays an
//! edit.
use super::{ClientActionDisposition, ModalPresentation, WorkbenchClientUi, WorkbenchClientView};
use crate::{
    client_view::KeyEvent,
    native_settings::{
        Edit, Editor, JobEnd, Row, SettingsDraw, SettingsInputContext, Submission, Token, View,
        key_edit, key_edit_for_view,
    },
    tui_presentation::{GeometrySettlement, PresentationPhase},
    workbench_client::{
        integration_lane::Lane as IntegrationLane,
        settings_lane::{Job, Lane},
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, SettingsOperation, SettingsOutcome, SettingsRefusal,
    SettingsRequest, SettingsSnapshot, WorkbenchRequestId,
};
use std::time::{Duration, Instant};
#[cfg(any(test, feature = "test-support"))]
pub(super) mod test_support;

pub(crate) use crate::localization::SettingsNotice as Notice;

/// The notice that names why the Server refused a read or edit. A refused edit wrote nothing. An
/// unusable file whose `detail` is a read failure is one that cannot be read, not its location.
fn refusal_notice(reason: SettingsRefusal, detail: Option<&str>) -> Notice {
    match reason {
        SettingsRefusal::UnusableFile
            if matches!(detail, Some("not-file" | "io-error" | "too-large")) =>
        {
            Notice::Unreadable
        }
        SettingsRefusal::Changed => Notice::SourceChanged,
        SettingsRefusal::Busy | SettingsRefusal::Locked | SettingsRefusal::FileInUse => {
            Notice::FileBusy
        }
        SettingsRefusal::InvalidValue => Notice::InvalidValue,
        SettingsRefusal::UnknownKey => Notice::Unsupported,
        SettingsRefusal::SyntaxError => Notice::SyntaxError,
        SettingsRefusal::ReadOnly => Notice::ReadOnly,
        SettingsRefusal::TooLarge => Notice::TooLarge,
        SettingsRefusal::UnusableFile => Notice::UnusableFile,
        SettingsRefusal::WriteFailed => Notice::WriteFailed,
        SettingsRefusal::Denied
        | SettingsRefusal::Unavailable
        | SettingsRefusal::SessionMismatch => Notice::Failed,
    }
}
#[derive(Default)]
pub(super) struct Coordinator {
    pub(super) editor: Editor,
    origin: Option<crate::local_client::LocalClientOrigin>,
    lane: Option<Lane>,
    /// Created by the first integration job; tied to that job's Session.
    integrations: Option<IntegrationLane>,
    session: Option<SessionId>,
    serial: u64,
    saving: bool,
    snapshot: Option<SettingsSnapshot>,
    observed_revision: Option<u64>,
    started: Option<Instant>,
    pub(super) pending: bool,
    notice: crate::workbench_client::settings_notice::SettingsNotice,
}
impl Coordinator {
    pub(super) fn new(login_shell_supported: bool) -> Self {
        Self {
            editor: Editor::new(login_shell_supported),
            ..Self::default()
        }
    }
    pub(super) fn set_origin(&mut self, origin: crate::local_client::LocalClientOrigin) {
        // Integration management runs on the origin's Worker; remote hosts stay manual.
        self.editor.configure_integrations(origin.is_local());
        self.origin = Some(origin);
    }
    pub(super) fn close(&mut self) {
        self.editor.edit(Edit::Cancel);
        if let Some(mut lane) = self.lane.take() {
            lane.close();
        }
        if let Some(end) = self.integrations.take().and_then(|mut lane| lane.abandon()) {
            self.editor.settle_integration(end);
        }
    }
    pub(super) fn clear_notice(&mut self) {
        self.notice.clear();
    }
    pub(super) const fn notice(&self) -> Option<Notice> {
        self.notice.current()
    }
    fn note(&mut self, notice: Notice) {
        self.notice.note(notice);
    }
    fn observe_snapshot(&mut self, snapshot: SettingsSnapshot) -> bool {
        let revision = snapshot.revision.value();
        if self
            .observed_revision
            .is_some_and(|observed| observed >= revision)
        {
            return false;
        }
        // The file state changes without the values when the file stops or starts parsing.
        self.editor.observe(&snapshot);
        if let Some(notice) = self.editor.take_notice() {
            self.note(notice);
        }
        // A notice about the file's state lasts only while that state does.
        if crate::native_settings::FileState::of(&snapshot)
            == crate::native_settings::FileState::Usable
            && matches!(
                self.notice(),
                Some(Notice::SyntaxError | Notice::Unreadable)
            )
        {
            self.clear_notice();
        }
        self.observed_revision = Some(revision);
        self.snapshot = Some(snapshot);
        true
    }
}
impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn settings(&self) -> Option<View<'_>> {
        self.settings.and_then(|settings| settings.editor.view())
    }
    /// Supplies presentation-only draft values to the whole-frame compositor.
    pub fn settings_appearance_preview(
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
impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn scroll_settings(
        &mut self,
        token: Token,
        up: bool,
        lines: usize,
    ) -> Option<Token> {
        self.settings.editor.scroll(token, up, lines)
    }
    /// Settings presents named themes as unavailable while the host shows fewer than 24-bit
    /// colors.
    pub(in crate::workbench_client) fn configure_reduced_color(
        &mut self,
        reduced_color: bool,
    ) -> bool {
        self.settings.editor.configure_reduced_color(reduced_color)
    }
    pub(in crate::workbench_client) fn apply_user_preferences(
        &mut self,
        values: &orchbee_workbench_protocol::UserSettingsValues,
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
        let busy = busy
            || self.retirement_failure.is_some()
            || self.copy_mode_active()
            || self.popup.captured();
        if busy {
            self.settings.note(Notice::Busy);
            return ClientActionDisposition::KeepNavigator;
        }
        if !self.ensure_settings_lane() {
            self.settings.note(Notice::Failed);
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(token) = self.settings.editor.open() else {
            self.settings.note(Notice::Busy);
            return ClientActionDisposition::KeepNavigator;
        };
        self.submit_settings(Submission {
            token,
            operation: SettingsOperation::Read {},
        });
        ClientActionDisposition::DismissNavigator
    }
    fn ensure_settings_lane(&mut self) -> bool {
        let Some(session) = self
            .state
            .as_ref()
            .map(|state| state.cache().session_id().clone())
        else {
            return false;
        };
        if self
            .settings
            .session
            .as_ref()
            .is_some_and(|old| old != &session)
        {
            let saving = self.settings.editor.in_flight();
            self.settings.close();
            self.settings.editor.retire();
            self.settings.editor.reset_integrations();
            self.settings.started = None;
            self.settings.pending = false;
            self.settings.observed_revision = None;
            self.settings.snapshot = None;
            if saving {
                self.settings.note(Notice::Unknown);
            }
        }
        if self.settings.lane.is_none() {
            self.settings.lane =
                self.settings.origin.clone().and_then(|origin| {
                    Lane::spawn(origin, self.clock.clone(), session.clone()).ok()
                });
            self.settings.session = Some(session);
        }
        self.settings.lane.is_some()
    }
    fn submit_settings(&mut self, submission: Submission) {
        // A new read or change replaces the outcome of the last one.
        self.settings.clear_notice();
        self.send_settings(submission);
    }
    /// Sends a read or edit without touching the notice, so a refusal stays shown while the
    /// changes waiting behind it are sent.
    fn send_settings(&mut self, submission: Submission) {
        let token = submission.token;
        let Some(serial) = self.settings.serial.checked_add(1) else {
            self.fail_settings(token, Notice::Failed);
            return;
        };
        let Some(session) = self.settings.session.clone().filter(|session| {
            self.state
                .as_ref()
                .is_some_and(|state| state.cache().session_id() == session)
        }) else {
            self.fail_settings(token, Notice::Failed);
            return;
        };
        self.settings.serial = serial;
        self.settings.saving = matches!(submission.operation, SettingsOperation::Edit { .. });
        self.settings.started = Some(self.clock.now());
        self.settings.pending = false;
        let request = SettingsRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                .expect("bounded Settings serial"),
            session_id: session,
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
        if self.settings.editor.failed(token) {
            self.settings.started = None;
            self.settings.pending = false;
            self.settings.note(notice);
        }
    }
    pub(super) fn drive_settings(&mut self) -> bool {
        self.ensure_settings_lane();
        self.send_waiting_settings();
        let pending = matches!(self.settings.editor.view(), Some(View::Loading { .. }))
            && self.settings.started.is_some_and(|started| {
                self.clock.now().saturating_duration_since(started) >= Duration::from_millis(200)
            });
        let mut changed = pending != self.settings.pending;
        self.settings.pending = pending;
        if let Some(snapshot) = self.settings.lane.as_ref().and_then(Lane::take_observation) {
            changed |= self.settings.observe_snapshot(snapshot);
        }
        changed |= self.drive_integrations();
        let Some(reply) = self.settings.lane.as_ref().and_then(Lane::take) else {
            return changed;
        };
        let token = reply.token;
        self.settings.pending = false;
        self.settings.started = None;
        match reply.result {
            Ok(reply) if self.settings.session.as_ref() == Some(&reply.session_id) => {
                self.finish_settings(token, reply.outcome)
            }
            result => self.fail_settings(
                token,
                if self.settings.saving && !matches!(result, Err(false)) {
                    Notice::Unknown
                } else {
                    Notice::Failed
                },
            ),
        }
        self.send_waiting_settings();
        true
    }
    /// Sends the changes waiting their turn, one at a time: the next only after the one before
    /// it is answered. A send that fails at once settles it and lets the next one go.
    fn send_waiting_settings(&mut self) {
        while let Some(submission) = self.settings.editor.next_submission() {
            self.send_settings(submission);
            if self.settings.editor.in_flight() {
                break;
            }
        }
    }
    /// Submits what a key or click asked for and names a request the page could not keep.
    fn submit_settings_change(&mut self, submission: Option<Submission>) {
        if let Some(submission) = submission {
            self.submit_settings(submission);
        }
        if let Some(notice) = self.settings.editor.take_notice() {
            self.settings.note(notice);
        }
    }
    /// Submits the job the page admitted and settles whatever ended, even while Settings is
    /// closed. A job of another Session is no longer followed.
    fn drive_integrations(&mut self) -> bool {
        let session = self
            .state
            .as_ref()
            .map(|state| state.cache().session_id().clone());
        let mut changed = false;
        if self
            .settings
            .integrations
            .as_ref()
            .is_some_and(|lane| Some(lane.session()) != session.as_ref())
        {
            self.settings.integrations = None;
            self.settings.editor.reset_integrations();
            changed = true;
        }
        if let Some(job) = self.settings.editor.take_integration_job() {
            let end = match (self.settings.origin.clone(), session) {
                (Some(origin), Some(session)) if origin.is_local() => self
                    .settings
                    .integrations
                    .get_or_insert_with(|| {
                        IntegrationLane::new(origin, self.clock.clone(), session)
                    })
                    .start(job),
                _ => Some(JobEnd::Exchange {
                    started: false,
                    submitted: false,
                }),
            };
            if let Some(end) = end {
                changed |= self.settings.editor.settle_integration(end);
            }
        }
        if let Some(end) = self
            .settings
            .integrations
            .as_mut()
            .and_then(IntegrationLane::poll)
        {
            changed |= self.settings.editor.settle_integration(end);
        }
        changed
    }
    fn finish_settings(&mut self, token: Token, outcome: SettingsOutcome) {
        match outcome {
            SettingsOutcome::Current { snapshot } => {
                if self.settings.editor.loaded(token, &snapshot) {
                    self.settings.observe_snapshot(*snapshot);
                }
            }
            // The confirmed snapshot shows at once, before the observation stream repeats it.
            SettingsOutcome::Edited { snapshot, .. } => {
                if self.settings.editor.edited(token) {
                    self.settings.observe_snapshot(*snapshot);
                }
            }
            SettingsOutcome::Refused { reason, detail, .. } => {
                self.fail_settings(token, refusal_notice(reason, detail.as_deref()))
            }
        }
    }
    pub(in crate::workbench_client) fn take_settings_snapshot(
        &mut self,
    ) -> Option<SettingsSnapshot> {
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
        let visible = self.settings.editor.visible();
        let edit = self
            .settings
            .editor
            .view()
            .and_then(|view| key_edit_for_view(event, view));
        if let Some(edit) = edit {
            let submission = self.settings.editor.edit(edit);
            self.submit_settings_change(submission);
            // A text the page refused names its reason on the status line, over an older notice.
            let refused_text = match self.settings.editor.view() {
                Some(View::Page(page)) => page.text_requirement().is_some(),
                _ => false,
            };
            if refused_text {
                self.settings.clear_notice();
            }
        }
        self.settle_settings_left(visible);
    }
    /// Leaving the page withdraws the notice it showed about one change.
    fn settle_settings_left(&mut self, was_visible: bool) {
        if was_visible && !self.settings.editor.visible() {
            self.settings.clear_notice();
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
        let visible = self.settings.editor.visible();
        let submission = self.settings.editor.control(token, control);
        self.submit_settings_change(submission);
        self.settle_settings_left(visible);
    }
    pub(in crate::workbench_client) fn click_settings(&mut self, token: Token, row: Row) {
        let submission = self.settings.editor.click(token, row);
        self.submit_settings_change(submission);
    }
    pub(super) fn after_settings_draw(&mut self, draw: Option<ModalPresentation<SettingsDraw>>) {
        match draw {
            Some(ModalPresentation::Presented(draw)) => {
                self.settings.editor.presented(draw);
                if self.settings.notice() == Some(Notice::TooSmall) {
                    self.settings.clear_notice();
                }
            }
            Some(ModalPresentation::NotPresented) => {
                match self.settings.editor.geometry_blocked() {
                    Some(GeometrySettlement::Refused) => {
                        self.settings.note(Notice::PresentationUnavailable)
                    }
                    Some(GeometrySettlement::Suspended) => self.settings.note(Notice::TooSmall),
                    None => {}
                }
            }
            None => self.settings.editor.not_presented(),
        }
    }
}
