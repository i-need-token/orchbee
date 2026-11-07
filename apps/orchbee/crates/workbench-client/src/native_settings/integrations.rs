//! Agent integration presentation: each Agent's last Worker report, one job at a time, and how a
//! job end settles, ported from the GUI's Server client. The Server and Worker own every effect;
//! this owner keeps no path, command line, or command output.
use crate::localization::{
    IntegrationAdapter, IntegrationBlock, IntegrationFailure, IntegrationFinding,
    IntegrationStatus, IntegrationStep, IntegrationUnknown,
};
use orchbee_agent_integration::{
    Availability, CapsuleStatus, Direction, FailureCode, InspectionDetail, IntegrationId,
    IntegrationOperation, IntegrationReport, Issue, ManagementMode, MutationReport, PlannedStep,
    Refusal, SnapshotItem, State, Verdict,
};
use orchbee_workbench_protocol::AgentIntegrationFailure;

#[cfg(test)]
mod tests;

/// One Agent's facts from the last report that covered it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Integration {
    pub(crate) availability: Availability,
    pub(crate) issue: Option<Issue>,
    pub(crate) management: ManagementMode,
    pub(crate) state: State,
}

impl Integration {
    /// Rows stay uninspected until a report arrives, as in the GUI.
    const UNINSPECTED: Self = Self {
        availability: Availability::Unknown,
        issue: None,
        management: ManagementMode::ManagedLocal,
        state: State::Unknown,
    };

    const fn reported(item: &SnapshotItem) -> Self {
        Self {
            availability: item.availability,
            issue: item.issue,
            management: item.management_mode,
            state: item.state,
        }
    }

    /// A change that may have begun leaves its row unknown until the next report.
    const fn uncertain(self) -> Self {
        Self {
            issue: None,
            state: State::Unknown,
            ..self
        }
    }
}

/// One Worker job this page can ask for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationJob {
    Refresh,
    Explain(IntegrationId),
    Change {
        id: IntegrationId,
        direction: Direction,
    },
}

impl IntegrationJob {
    pub(crate) const fn operation(self) -> IntegrationOperation {
        match self {
            Self::Refresh => IntegrationOperation::List { provider: None },
            Self::Explain(provider) => IntegrationOperation::Explain { provider },
            Self::Change {
                id,
                direction: Direction::Enable,
            } => IntegrationOperation::Enable { provider: id },
            Self::Change {
                id,
                direction: Direction::Disable,
            } => IntegrationOperation::Disable { provider: id },
        }
    }

    const fn target(self) -> Option<IntegrationId> {
        match self {
            Self::Refresh => None,
            Self::Explain(id) | Self::Change { id, .. } => Some(id),
        }
    }
}

/// How a job ended for this Client, as the CLI's job driver reports it.
#[derive(Debug)]
pub(crate) enum JobEnd {
    Completed(Box<IntegrationReport>),
    /// The job outlived its deadline; the Client asked the Worker to stop it.
    Deadline,
    Failed {
        reason: AgentIntegrationFailure,
        started: bool,
    },
    /// No answer could be followed; `submitted` means the request may have reached the Server.
    Exchange {
        started: bool,
        submitted: bool,
    },
}

/// The facts one explain report establishes, without paths or command lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Explanation {
    pub(crate) item: SnapshotItem,
    pub(crate) detail: InspectionDetail,
    pub(crate) adapter: IntegrationAdapter,
    pub(crate) plan: Vec<IntegrationStep>,
}

impl Explanation {
    fn new(
        item: SnapshotItem,
        detail: InspectionDetail,
        capsule: &CapsuleStatus,
        plan: &[PlannedStep],
    ) -> Self {
        let adapter = if capsule.binding_failure {
            IntegrationAdapter::BindingFailed
        } else if capsule.ready {
            IntegrationAdapter::Ready
        } else if capsule.source_kind == "absent" {
            IntegrationAdapter::SourceAbsent
        } else {
            IntegrationAdapter::NotReady
        };
        let plan = plan
            .iter()
            .map(|step| match step {
                PlannedStep::CapsulePublish { .. } => IntegrationStep::PublishAdapter,
                PlannedStep::Command { executable, .. } => IntegrationStep::Command(
                    std::path::Path::new(executable)
                        .file_name()
                        .map_or_else(|| executable.clone(), |name| name.to_string_lossy().into()),
                ),
                PlannedStep::GuardedWrite { .. } => IntegrationStep::WriteEntry,
                PlannedStep::GuardedRemove { .. } => IntegrationStep::RemoveEntry,
            })
            .collect();
        Self {
            item,
            detail,
            adapter,
            plan,
        }
    }

    /// Manual setup is a definite fact of the platform; an inspection that could not finish is an
    /// unknown result, never a missing integration.
    pub(crate) const fn finding(&self) -> IntegrationFinding {
        let item = &self.item;
        if matches!(item.management_mode, ManagementMode::ManualOnly) {
            return IntegrationFinding::ManualSetup;
        }
        if item.timed_out {
            return IntegrationFinding::Unknown(IntegrationUnknown::TimedOut);
        }
        if matches!(item.issue, Some(Issue::InspectionFailed)) {
            return IntegrationFinding::Unknown(IntegrationUnknown::InspectionFailed);
        }
        if matches!(item.availability, Availability::NotFound) {
            return IntegrationFinding::AgentMissing;
        }
        match item.state {
            State::Enabled => IntegrationFinding::Ready,
            State::Disabled => IntegrationFinding::Disabled,
            State::NotInstalled => IntegrationFinding::NotInstalled,
            State::UpdateRequired => IntegrationFinding::UpdateRequired,
            State::Conflict => IntegrationFinding::Conflict,
            State::Unknown => IntegrationFinding::Unknown(IntegrationUnknown::Undetermined),
        }
    }
}

/// One Agent's diagnosis: waiting for its explain job, explained, or not answered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Diagnosis {
    Pending,
    Explained(Box<Explanation>),
    Failed(IntegrationFailure),
}

#[derive(Clone, Copy, Debug)]
struct Running {
    job: IntegrationJob,
    submitted: bool,
}

/// Every Agent in canonical order, with the single job the page may run at a time.
#[derive(Clone, Debug)]
pub(crate) struct Integrations {
    rows: [Integration; IntegrationId::ALL.len()],
    running: Option<Running>,
    failure: Option<IntegrationFailure>,
    diagnosis: Option<(IntegrationId, Diagnosis)>,
    reported: bool,
}

impl Default for Integrations {
    fn default() -> Self {
        Self {
            rows: [Integration::UNINSPECTED; IntegrationId::ALL.len()],
            running: None,
            failure: None,
            diagnosis: None,
            reported: false,
        }
    }
}

fn index(id: IntegrationId) -> usize {
    IntegrationId::ALL
        .iter()
        .position(|candidate| *candidate == id)
        .expect("canonical integration identity")
}

impl Integrations {
    pub(crate) fn row(&self, id: IntegrationId) -> Integration {
        self.rows[index(id)]
    }

    pub(crate) const fn pending(&self) -> bool {
        self.running.is_some()
    }

    /// Refresh covers every row; any other job covers its own Agent.
    pub(crate) fn pending_for(&self, id: IntegrationId) -> bool {
        self.running
            .is_some_and(|running| running.job.target().is_none_or(|target| target == id))
    }

    /// Whether any report has replaced the uninspected rows.
    pub(crate) const fn reported(&self) -> bool {
        self.reported
    }

    pub(crate) const fn failure(&self) -> Option<IntegrationFailure> {
        self.failure
    }

    pub(crate) fn diagnosis(&self, id: IntegrationId) -> Option<&Diagnosis> {
        self.diagnosis
            .as_ref()
            .filter(|(current, _)| *current == id)
            .map(|(_, diagnosis)| diagnosis)
    }

    /// The GUI's status precedence: management, inspection, presence, then state.
    pub(crate) fn status(&self, id: IntegrationId) -> IntegrationStatus {
        if self.pending_for(id) {
            return IntegrationStatus::Working;
        }
        let row = self.row(id);
        if row.management == ManagementMode::ManualOnly {
            return IntegrationStatus::ManualOnly;
        }
        if row.issue == Some(Issue::InspectionFailed) {
            return IntegrationStatus::InspectionFailed;
        }
        if row.availability == Availability::NotFound {
            return IntegrationStatus::NotFound;
        }
        match row.state {
            State::Enabled => IntegrationStatus::Enabled,
            State::Disabled => IntegrationStatus::Disabled,
            State::NotInstalled => IntegrationStatus::NotInstalled,
            State::UpdateRequired => IntegrationStatus::UpdateRequired,
            State::Conflict => IntegrationStatus::Conflict,
            State::Unknown => IntegrationStatus::Unknown,
        }
    }

    /// The GUI's switch rule; the Worker still decides whether a requested change may proceed.
    pub(crate) fn block(&self, id: IntegrationId) -> Option<IntegrationBlock> {
        let row = self.row(id);
        if self.running.is_some() {
            Some(IntegrationBlock::Pending)
        } else if row.management == ManagementMode::ManualOnly {
            Some(IntegrationBlock::ManualOnly)
        } else if row.state == State::Conflict {
            Some(IntegrationBlock::Conflict)
        } else if row.state == State::Unknown {
            Some(IntegrationBlock::Unknown)
        } else if row.state != State::Enabled && row.availability != Availability::Available {
            Some(if row.availability == Availability::NotFound {
                IntegrationBlock::AgentMissing
            } else {
                IntegrationBlock::Unknown
            })
        } else {
            None
        }
    }

    /// The switch's next position; it never moves before the Worker reports.
    pub(crate) fn change(&self, id: IntegrationId) -> Option<IntegrationJob> {
        self.block(id).is_none().then(|| IntegrationJob::Change {
            id,
            direction: if self.row(id).state == State::Enabled {
                Direction::Disable
            } else {
                Direction::Enable
            },
        })
    }

    /// Admits one job; a running job refuses another, as in the GUI.
    pub(super) fn begin(&mut self, job: IntegrationJob) -> bool {
        if self.running.is_some() {
            return false;
        }
        self.running = Some(Running {
            job,
            submitted: false,
        });
        self.failure = None;
        if let IntegrationJob::Explain(id) = job {
            self.diagnosis = Some((id, Diagnosis::Pending));
        }
        true
    }

    /// Hands the admitted job to the transport exactly once.
    pub(super) fn take_submission(&mut self) -> Option<IntegrationJob> {
        let running = self.running.as_mut().filter(|running| !running.submitted)?;
        running.submitted = true;
        Some(running.job)
    }

    /// Settles the running job. A change that may have begun is never reported as refused, and an
    /// unknown outcome is never retried.
    pub(super) fn settle(&mut self, end: JobEnd) -> bool {
        let Some(Running { job, .. }) = self.running.take() else {
            return false;
        };
        match job {
            IntegrationJob::Refresh => self.settle_refresh(end),
            IntegrationJob::Explain(id) => self.settle_explain(id, end),
            IntegrationJob::Change { id, direction } => self.settle_change(id, direction, end),
        }
        true
    }

    fn settle_refresh(&mut self, end: JobEnd) {
        if let JobEnd::Completed(report) = &end
            && let IntegrationReport::List { items, .. } = report.as_ref()
        {
            for (row, id) in self.rows.iter_mut().zip(IntegrationId::ALL) {
                *row = items.iter().find(|item| item.integration_id == id).map_or(
                    Integration {
                        availability: Availability::Unknown,
                        issue: Some(Issue::InspectionFailed),
                        state: State::Unknown,
                        ..*row
                    },
                    Integration::reported,
                );
            }
            self.reported = true;
            return;
        }
        self.failure = Some(read_failure(&end));
    }

    fn settle_explain(&mut self, id: IntegrationId, end: JobEnd) {
        let diagnosis = match end {
            JobEnd::Completed(report) => match *report {
                IntegrationReport::Explain {
                    item,
                    detail,
                    capsule,
                    plan,
                } if item.integration_id == id => {
                    self.rows[index(id)] = Integration::reported(&item);
                    Diagnosis::Explained(Box::new(Explanation::new(item, detail, &capsule, &plan)))
                }
                report => Diagnosis::Failed(read_failure(&JobEnd::Completed(Box::new(report)))),
            },
            end => Diagnosis::Failed(read_failure(&end)),
        };
        self.diagnosis = Some((id, diagnosis));
    }

    fn settle_change(&mut self, id: IntegrationId, direction: Direction, end: JobEnd) {
        let slot = index(id);
        let uncertain = |rows: &mut [Integration; IntegrationId::ALL.len()]| {
            rows[slot] = rows[slot].uncertain();
            Some(IntegrationFailure::VerificationFailed)
        };
        self.failure = match end {
            JobEnd::Completed(report) => match *report {
                IntegrationReport::HomeUnavailable {} => Some(IntegrationFailure::OperationFailed),
                IntegrationReport::Mutation { report }
                    if report.integration_id == id && report.direction == direction =>
                {
                    self.mutated(slot, &report);
                    verdict_failure(report.verdict, report.effect_begun)
                }
                _ => uncertain(&mut self.rows),
            },
            JobEnd::Deadline => uncertain(&mut self.rows),
            JobEnd::Failed { started, reason }
                if started || reason == AgentIntegrationFailure::Abandoned =>
            {
                uncertain(&mut self.rows)
            }
            JobEnd::Failed { reason, .. } => Some(if reason == AgentIntegrationFailure::Busy {
                IntegrationFailure::Busy
            } else {
                IntegrationFailure::ControlUnavailable
            }),
            JobEnd::Exchange { started, submitted } if started || submitted => {
                uncertain(&mut self.rows)
            }
            JobEnd::Exchange { .. } => Some(IntegrationFailure::ControlUnavailable),
        };
    }

    fn mutated(&mut self, slot: usize, report: &MutationReport) {
        match report.observed {
            Some(observed) if observed.integration_id == report.integration_id => {
                self.rows[slot] = Integration::reported(&observed);
            }
            _ if report.effect_begun => self.rows[slot] = self.rows[slot].uncertain(),
            _ => {}
        }
    }
}

/// A read that did not finish changes nothing; only an unanswered start proves Busy.
fn read_failure(end: &JobEnd) -> IntegrationFailure {
    match end {
        JobEnd::Deadline => IntegrationFailure::OperationFailed,
        JobEnd::Completed(report)
            if matches!(report.as_ref(), IntegrationReport::HomeUnavailable {}) =>
        {
            IntegrationFailure::OperationFailed
        }
        JobEnd::Failed {
            reason: AgentIntegrationFailure::Busy,
            started: false,
        } => IntegrationFailure::Busy,
        _ => IntegrationFailure::ControlUnavailable,
    }
}

fn verdict_failure(verdict: Verdict, effect_begun: bool) -> Option<IntegrationFailure> {
    match verdict {
        Verdict::Completed => None,
        Verdict::Refused(refusal) => Some(match refusal {
            Refusal::AgentUnavailable => IntegrationFailure::AgentUnavailable,
            Refusal::Conflict => IntegrationFailure::Conflict,
            Refusal::ManualActionRequired => IntegrationFailure::ManualActionRequired,
            Refusal::CapsuleUnavailable | Refusal::InspectionFailed => {
                IntegrationFailure::OperationFailed
            }
            Refusal::Deadline if effect_begun => IntegrationFailure::VerificationFailed,
            Refusal::Deadline => IntegrationFailure::OperationFailed,
        }),
        Verdict::Failed(code) => Some(match code {
            FailureCode::Conflict => IntegrationFailure::Conflict,
            FailureCode::ManualActionRequired => IntegrationFailure::ManualActionRequired,
            FailureCode::OperationFailed => IntegrationFailure::OperationFailed,
            FailureCode::Leaked
            | FailureCode::OutputTooLarge
            | FailureCode::TimedOut
            | FailureCode::VerificationFailed => IntegrationFailure::VerificationFailed,
        }),
        Verdict::Cancelled | Verdict::Unknown => Some(IntegrationFailure::VerificationFailed),
    }
}
