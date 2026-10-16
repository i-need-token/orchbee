use super::*;
use orchbee_workbench_protocol::WorkbenchProjectId;

#[test]
fn conditional_dissolve_requires_exact_kind_and_base_for_confirmation_refusal() {
    for conditional in [false, true] {
        let project_id = WorkbenchProjectId::parse("00000000000000000000000000000002").unwrap();
        let action = if conditional {
            WorkbenchAction::ProjectDissolveIfIdle {
                project_id: project_id.clone(),
            }
        } else {
            WorkbenchAction::ProjectDissolve {
                project_id: project_id.clone(),
            }
        };
        for revision in [7, 8] {
            let result = classify_outcome(
                &action,
                WorkbenchRevision::new(7),
                WorkbenchCommandOutcome::Rejected {
                    revision: WorkbenchRevision::new(revision),
                    reason: WorkbenchRejectionReason::ConfirmationRequired,
                },
            );
            if conditional && revision == 7 {
                assert!(matches!(
                    result,
                    Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                        WorkbenchSetupPrecommit::Rejected {
                            reason: WorkbenchRejectionReason::ConfirmationRequired,
                            ..
                        }
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(WorkbenchSetupOutcomeUnknownReason::ActionIncompatible)
                ));
            }
        }
        assert!(
            matches!(classify_outcome(&action, WorkbenchRevision::new(7), WorkbenchCommandOutcome::ProjectDissolved {
            revision: WorkbenchRevision::new(8), project_id: project_id.clone(),
        }), Ok(WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::ProjectDissolved {
            revision, project_id: actual,
        })) if revision == WorkbenchRevision::new(8) && actual == project_id)
        );
    }
}
