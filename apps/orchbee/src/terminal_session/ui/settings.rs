//! Draft extraction and one Settings operation; the configuration watcher owns live application.
use super::{ModalPresentation, NativeSessionView, NativeUi, SessionActionDisposition};
use crate::terminal_session::{
    configuration,
    settings_lane::{Job, Lane},
};
use crate::{
    client_view::KeyEvent,
    native_settings::{Basis, Edit, Editor, Row, Submission, Token, Values, View, key_edit},
};
use orchbee_local_site_runtime::WindowsNotificationDelivery;
use orchbee_workbench_protocol::{
    ConfigurationSnapshot, NativeSettingsFailure, NativeSettingsOperation, NativeSettingsOutcome,
    NativeSettingsRequest, ProtocolVersion, SiteId, WorkbenchRequestId,
};
use std::time::{Duration, Instant};
#[cfg(test)]
mod tests;

/// Localized results remain value-free; committed saves have their own failure presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Notice {
    Saved,
    NoChange,
    SavedNotApplied,
    SourceChanged,
    ConfigurationChanged,
    Failed,
    Unknown,
    Busy,
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
    snapshot: Option<ConfigurationSnapshot>,
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
        if let Some(lane) = &mut self.lane {
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
}
impl NativeSessionView<'_> {
    /// The pure Settings projection never exposes source bytes or owns a live theme.
    pub(crate) fn settings(&self) -> Option<View> {
        self.settings.and_then(|settings| settings.editor.view())
    }
    pub(crate) fn settings_pending(&self) -> bool {
        self.settings.is_some_and(|settings| settings.pending)
    }
}
impl NativeUi {
    pub(super) fn request_settings(&mut self) -> SessionActionDisposition {
        self.settings.clear_notice();
        if self.retirement_failure.is_some()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
        {
            self.settings.note(Notice::Busy, self.clock.now());
            return SessionActionDisposition::KeepNavigator;
        }
        let Some(site) = self
            .state
            .as_ref()
            .map(|state| state.cache().site_id().clone())
        else {
            self.settings.note(Notice::Failed, self.clock.now());
            return SessionActionDisposition::KeepNavigator;
        };
        let Some(token) = self.settings.editor.open() else {
            self.settings.note(Notice::Busy, self.clock.now());
            return SessionActionDisposition::KeepNavigator;
        };
        self.settings.site = Some(site);
        self.submit_settings(Submission {
            token,
            operation: NativeSettingsOperation::Open {},
        });
        SessionActionDisposition::DismissNavigator
    }
    fn submit_settings(&mut self, submission: Submission) {
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
        if self.settings.lane.is_none() {
            self.settings.lane = self
                .settings
                .origin
                .clone()
                .and_then(|origin| Lane::spawn(origin, self.clock.clone()).ok());
        }
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
        if self.settings.editor.complete(token) {
            self.settings.note(notice, self.clock.now());
        }
    }
    pub(super) fn drive_settings(&mut self) -> bool {
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
        let changed = pending != self.settings.pending;
        self.settings.pending = pending;
        let Some(reply) = self.settings.lane.as_ref().and_then(Lane::take) else {
            return expired || changed;
        };
        let token = reply.token;
        self.settings.pending = false;
        self.settings.started = None;
        match reply.result {
            Ok(reply) => {
                if self.settings.site.as_ref() != Some(&reply.site_id) {
                    self.fail_settings(
                        token,
                        if self.settings.saving {
                            Notice::Unknown
                        } else {
                            Notice::Failed
                        },
                    );
                    return true;
                }
                if self
                    .state
                    .as_ref()
                    .is_none_or(|state| state.cache().site_id() != &reply.site_id)
                {
                    let notice = match reply.outcome {
                        NativeSettingsOutcome::SavedApplied { .. }
                        | NativeSettingsOutcome::SavedNotApplied { .. } => Notice::SavedNotApplied,
                        NativeSettingsOutcome::NoChange { .. } => Notice::NoChange,
                        _ => Notice::Failed,
                    };
                    self.fail_settings(token, notice);
                    return true;
                }
                match reply.outcome {
                    NativeSettingsOutcome::Opened { source, snapshot } => {
                        let Some(admitted) = configuration::admit(&snapshot) else {
                            self.fail_settings(token, Notice::Failed);
                            return true;
                        };
                        let basis = Basis {
                            revision: snapshot.revision,
                            source,
                            values: Values {
                                theme: snapshot.theme_name,
                                reminders: admitted.notifications.agent_delivery
                                    != WindowsNotificationDelivery::Off,
                                sound: admitted.notifications.sound_enabled,
                            },
                        };
                        if self.settings.editor.loaded(token, basis) {
                            self.settings.snapshot = Some(snapshot);
                        }
                    }
                    NativeSettingsOutcome::SavedApplied { snapshot } => {
                        self.finish_settings_snapshot(token, snapshot, Notice::Saved)
                    }
                    NativeSettingsOutcome::NoChange { snapshot } => {
                        self.finish_settings_snapshot(token, snapshot, Notice::NoChange)
                    }
                    NativeSettingsOutcome::SavedNotApplied { snapshot } => {
                        self.finish_settings_snapshot(token, snapshot, Notice::SavedNotApplied)
                    }
                    NativeSettingsOutcome::Refused { reason } => self.fail_settings(
                        token,
                        match reason {
                            NativeSettingsFailure::SourceChanged => Notice::SourceChanged,
                            NativeSettingsFailure::ConfigurationChanged => {
                                Notice::ConfigurationChanged
                            }
                            NativeSettingsFailure::Busy => Notice::Busy,
                            _ => Notice::Failed,
                        },
                    ),
                }
            }
            Err(submitted) => self.fail_settings(
                token,
                if submitted && self.settings.saving {
                    Notice::Unknown
                } else {
                    Notice::Failed
                },
            ),
        }
        true
    }
    fn finish_settings_snapshot(
        &mut self,
        token: Token,
        snapshot: ConfigurationSnapshot,
        notice: Notice,
    ) {
        if !self.settings.editor.complete(token) {
            return;
        }
        let admitted = configuration::admit(&snapshot).is_some();
        let notice = if admitted {
            notice
        } else if matches!(notice, Notice::Saved | Notice::SavedNotApplied) {
            Notice::SavedNotApplied
        } else {
            Notice::Failed
        };
        if admitted {
            self.settings.snapshot = Some(snapshot);
        }
        self.settings.note(notice, self.clock.now());
    }
    pub(in crate::terminal_session) fn take_settings_snapshot(
        &mut self,
    ) -> Option<ConfigurationSnapshot> {
        self.settings.snapshot.take()
    }
    pub(in crate::terminal_session) fn clear_settings_notice(&mut self) {
        self.settings.clear_notice();
    }
    pub(in crate::terminal_session) fn settings_delivery_failed(&mut self) {
        let notice = if matches!(
            self.settings.notice(self.clock.now()),
            Some(Notice::Saved | Notice::SavedNotApplied)
        ) {
            Notice::SavedNotApplied
        } else {
            Notice::Failed
        };
        self.settings.note(notice, self.clock.now());
    }
    pub(in crate::terminal_session) fn settings_active(&self) -> bool {
        self.settings.editor.visible()
    }
    pub(in crate::terminal_session) fn capture_settings_key(&mut self, event: KeyEvent) {
        let edit = key_edit(event);
        if let Some(edit) = edit {
            let saving = matches!(self.settings.editor.view(), Some(View::Saving { .. }));
            self.settings.clear_notice();
            if let Some(submission) = self.settings.editor.edit(edit) {
                self.submit_settings(submission);
            }
            if saving && matches!(edit, Edit::Cancel) {
                self.settings.note(Notice::Background, self.clock.now());
            }
        }
    }
    pub(in crate::terminal_session) fn click_settings(&mut self, token: Token, row: Row) {
        if let Some(submission) = self.settings.editor.click(token, row) {
            self.submit_settings(submission);
        }
    }
    pub(super) fn after_settings_draw(&mut self, draw: Option<ModalPresentation<Token>>) {
        match draw {
            Some(ModalPresentation::Presented(token)) => self.settings.editor.draw(token),
            Some(ModalPresentation::NotPresented) => {
                self.settings.editor.edit(Edit::Cancel);
                self.settings.note(Notice::TooSmall, self.clock.now());
            }
            None => {}
        }
    }
}
