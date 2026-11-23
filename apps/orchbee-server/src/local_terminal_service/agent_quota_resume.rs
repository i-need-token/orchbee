//! The Session's periodic sampler owns pending quota resumes. Every attempt
//! needs fresh evidence from the same live Agent and the Host's input gate.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_capability_node_protocol::{
    SendKey, SendKeyStroke, SendPasteContent, SendPrecondition, SendProgress, SendRecord,
    SendRecords, SendRejection, SendScreenGate, SendScreenState, SendStrokeKey, SendUnavailable,
    TerminalSendOutcome,
};
use orchbee_local_runtime::UserSettingsField;
use orchbee_terminal_protocol::TerminalTextFrame;
use orchbee_workbench_protocol::PanelAgentState;

use super::LocalTerminalService;
use crate::{
    agent_screen::Detection,
    session_coordinator::HostAgentRun,
    terminal_coordinator::BindingViewerTarget,
    worker_obcn_carrier::{WorkerOperationResult, WorkerRequestError, WorkerRequestHandle},
};

mod codex;
mod state;
use codex::Composer;
use state::{Run, Screen, unix_now};

pub(super) struct QuotaResumes {
    observed_since: Option<u64>,
    runs: BTreeMap<[u8; 16], Run>,
}

impl Default for QuotaResumes {
    fn default() -> Self {
        Self {
            observed_since: unix_now(),
            runs: BTreeMap::new(),
        }
    }
}

impl QuotaResumes {
    pub(super) fn retain(&mut self, retained: &BTreeSet<[u8; 16]>) {
        self.runs.retain(|terminal, _| retained.contains(terminal));
    }

    pub(super) fn observe(
        &mut self,
        service: &LocalTerminalService,
        handle: &WorkerRequestHandle,
        target: BindingViewerTarget,
        agent: HostAgentRun,
        frame: &TerminalTextFrame,
        detection: Option<Detection>,
    ) {
        if agent.provider != AgentReportProvider::Codex {
            return;
        }
        let Some(observed) = screen_evidence(frame, detection) else {
            return;
        };
        let Some(now) = unix_now() else {
            return;
        };
        let born = agent
            .created_unix_milliseconds()
            .filter(|born| self.observed_since.is_some_and(|since| *born > since));
        let run = self
            .runs
            .entry(target.terminal_id.as_bytes())
            .or_insert_with(|| Run::new(target.identity, agent, born));
        if run.identity != target.identity {
            // A different Host epoch has a different input record. Only a
            // new observation can establish authority there.
            *run = Run::new(target.identity, agent, None);
        } else if run.agent != agent {
            *run = Run::new(target.identity, agent, born);
        }
        let Some(gate) = run.observe(frame.route, observed, now, Instant::now(), || {
            service.quota_resume_enabled()
        }) else {
            return;
        };
        let outcome = service.resume_quota_prompt(handle, target, agent, gate);
        if !matches!(outcome, Delivery::Retry) {
            run.finish();
        }
        if let Some(diagnostic) = outcome.diagnostic() {
            eprintln!(
                "orchbee-server diagnostic component=agent-quota-resume outcome={diagnostic}"
            );
        }
    }
}

fn screen_evidence(frame: &TerminalTextFrame, detection: Option<Detection>) -> Option<Screen> {
    let detection = detection?;
    if detection.skip || detection.visible_blocker {
        return None;
    }
    let composer = codex::composer(frame);
    match detection.state {
        PanelAgentState::Working
            if detection.visible_working
                && !matches!(composer, Some(Composer::Quota(_)))
                && codex::active_turn(frame) =>
        {
            Some(Screen::Working)
        }
        PanelAgentState::Idle | PanelAgentState::Done | PanelAgentState::Error
            if !detection.visible_working =>
        {
            match composer? {
                Composer::Clear => Some(Screen::Clear),
                Composer::Quota(retry) => Some(Screen::Quota(retry)),
            }
        }
        _ => None,
    }
}

/// A transient result can be tried again only with proof that no input queued.
#[derive(Debug, Eq, PartialEq)]
enum Delivery {
    Queued,
    Retry,
    Refused,
    Unknown,
}

impl Delivery {
    fn diagnostic(&self) -> Option<&'static str> {
        match self {
            Self::Queued => Some("queued"),
            Self::Retry => None,
            Self::Refused => Some("refused"),
            Self::Unknown => Some("unknown"),
        }
    }
}

fn delivery(result: Result<WorkerOperationResult, WorkerRequestError>) -> Delivery {
    match result {
        Ok(WorkerOperationResult::Send(result)) => match result.outcome() {
            TerminalSendOutcome::Accepted { .. } => Delivery::Queued,
            TerminalSendOutcome::OutcomeUnknown { .. } => Delivery::Unknown,
            TerminalSendOutcome::Rejected {
                reason, progress, ..
            } if progress == SendProgress::none()
                && matches!(
                    reason,
                    SendRejection::WrongWorker
                        | SendRejection::StaleWorkerGeneration
                        | SendRejection::StaleHostEpoch
                        | SendRejection::TerminalUnavailable
                        | SendRejection::HostInputBackpressure
                        | SendRejection::HostInputUnavailable
                        | SendRejection::ScreenRefused(
                            SendScreenState::AgentGone
                                | SendScreenState::Changed
                                | SendScreenState::InputPending
                                | SendScreenState::PersonRecent
                        )
                ) =>
            {
                Delivery::Retry
            }
            TerminalSendOutcome::Unavailable {
                reason: SendUnavailable::HostUnavailable | SendUnavailable::InputBudgetExhausted,
                progress,
                ..
            } if progress == SendProgress::none() => Delivery::Retry,
            TerminalSendOutcome::Rejected { .. } | TerminalSendOutcome::Unavailable { .. } => {
                Delivery::Refused
            }
        },
        Ok(_) | Err(WorkerRequestError::PostDeliveryOutcomeUnknown(_)) => Delivery::Unknown,
        Err(
            WorkerRequestError::Unavailable
            | WorkerRequestError::Backpressure
            | WorkerRequestError::WrongWorker
            | WorkerRequestError::WrongGeneration
            | WorkerRequestError::NotDelivered(_)
            | WorkerRequestError::Declined,
        ) => Delivery::Retry,
        Err(WorkerRequestError::InvalidRequest(_)) => Delivery::Refused,
    }
}

impl LocalTerminalService {
    fn quota_resume_enabled(&self) -> bool {
        self.configuration
            .with_settings(quota_enabled)
            .unwrap_or(false)
    }

    fn quota_resume_prompt(&self) -> Option<String> {
        self.configuration
            .with_settings(|settings| {
                quota_enabled(settings).then(|| settings.values.quota_resume_prompt.clone())
            })
            .flatten()
    }

    fn resume_quota_prompt(
        &self,
        handle: &WorkerRequestHandle,
        target: BindingViewerTarget,
        agent: HostAgentRun,
        gate: SendScreenGate,
    ) -> Delivery {
        let Some(prompt) = self.quota_resume_prompt() else {
            return Delivery::Retry;
        };
        let Some(records) = prompt_records(&prompt) else {
            return Delivery::Refused;
        };
        self.send_at_agent_screen(
            handle,
            target,
            Some(agent),
            SendPrecondition::Screen(gate),
            records,
            || {
                self.configuration
                    .with_settings(|settings| {
                        quota_enabled(settings) && settings.values.quota_resume_prompt == prompt
                    })
                    .unwrap_or(false)
            },
        )
        .map_or(Delivery::Retry, delivery)
    }
}

fn quota_enabled(settings: &orchbee_local_runtime::UserSettings) -> bool {
    let key = UserSettingsField::QuotaResumePrompt
        .key()
        .expect("prompt key");
    settings.values.quota_resume_enabled
        && (!settings.file_keys.contains(key) || settings.file_values.contains_key(key))
}

fn prompt_records(prompt: &str) -> Option<SendRecords> {
    if !orchbee_workbench_protocol::valid_quota_resume_prompt(prompt) {
        return None;
    }
    let mut records = vec![SendRecord::Paste {
        content: SendPasteContent::new(prompt.as_bytes().to_vec()).ok()?,
    }];
    // Windows Codex consumes Enter as part of a pending paste burst. Right
    // flushes that burst at the end of the draft before the queued Enter.
    if cfg!(windows) {
        records.push(SendRecord::Stroke(
            SendKeyStroke::new(SendStrokeKey::Named(SendKey::Right), 0).ok()?,
        ));
    }
    records.push(SendRecord::Stroke(
        SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0).ok()?,
    ));
    SendRecords::new(records).ok()
}
