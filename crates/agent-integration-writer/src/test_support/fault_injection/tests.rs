use std::{cell::Cell, path::Path, time::Duration};

use super::super::super::exec::{self, CommandResult};
use super::*;
use crate::TerminationSource;

struct ResetPlan;

impl Drop for ResetPlan {
    fn drop(&mut self) {
        reset();
    }
}

#[test]
fn insufficient_teardown_budget_does_not_consume_a_scripted_command() {
    let mut plan = Plan::default();
    plan.script_command(ScriptedResult::Completed(b"retained".to_vec()));
    install(plan);
    let _reset = ResetPlan;
    let executable = Path::new("/fake-provider");

    let result = run_scripted(
        executable,
        &[],
        now() + Duration::from_millis(600),
        None,
        ScriptedAdmission::Ordinary,
    );
    assert!(matches!(result, CommandResult::TimedOut));
    assert!(argv_log().is_empty());

    let result = run_scripted(
        executable,
        &[],
        now() + exec::SLOT,
        None,
        ScriptedAdmission::Ordinary,
    );
    assert!(matches!(
        result,
        CommandResult::Completed { ref stdout }
            if stdout.as_slice() == b"retained"
    ));
    assert_eq!(argv_log(), vec![vec![executable.display().to_string()]]);
}

struct AdvancePastRunThenCancel {
    polls: Cell<usize>,
}

impl TerminationSource for AdvancePastRunThenCancel {
    fn requested(&self) -> bool {
        let poll = self.polls.get();
        self.polls.set(poll + 1);
        if poll == 0 {
            advance_clock(exec::SLOT);
            false
        } else {
            true
        }
    }
}

#[test]
fn wait_for_cancel_stops_at_the_current_command_slot() {
    let mut plan = Plan::default();
    plan.script_command(ScriptedResult::WaitForCancel);
    install(plan);
    let _reset = ResetPlan;
    let cancel = AdvancePastRunThenCancel {
        polls: Cell::new(0),
    };
    let result = run_scripted(
        Path::new("/fake-provider"),
        &[],
        now() + Duration::from_secs(40),
        Some(&cancel),
        ScriptedAdmission::Ordinary,
    );

    assert!(matches!(result, CommandResult::TimedOut));
    assert_eq!(cancel.polls.get(), 1);
}

#[test]
fn pre_effect_cancel_does_not_consume_or_log_the_scripted_command() {
    let mut plan = Plan::default();
    plan.script_command(ScriptedResult::Completed(b"retained".to_vec()));
    install(plan);
    let _reset = ResetPlan;
    let executable = Path::new("/fake-provider");
    let home = Path::new("/");
    let flag = cancel_flag().unwrap();
    flag.store(true, std::sync::atomic::Ordering::Relaxed);

    let result = exec::run_command(
        executable,
        &[],
        home,
        now() + exec::SLOT,
        Some(&InjectCancel),
    );

    assert!(matches!(result, CommandResult::Cancelled));
    assert!(argv_log().is_empty());
    flag.store(false, std::sync::atomic::Ordering::Relaxed);
    let retained = exec::run_command(executable, &[], home, now() + exec::SLOT, None);
    assert!(matches!(
        retained,
        CommandResult::Completed { ref stdout } if stdout.as_slice() == b"retained"
    ));
}

#[test]
fn mid_effect_cancel_consumes_exactly_one_scripted_command() {
    let mut plan = Plan::default();
    plan.script_command(ScriptedResult::WaitForCancel);
    plan.script_command(ScriptedResult::Completed(b"next".to_vec()));
    plan.on("provider-command", BoundaryAction::Sigint);
    install(plan);
    let _reset = ResetPlan;
    let executable = Path::new("/fake-provider");
    let home = Path::new("/");
    boundary("provider-command", Path::new(""));

    let result = exec::run_command(
        executable,
        &[],
        home,
        now() + exec::SLOT,
        Some(&InjectCancel),
    );

    assert!(matches!(result, CommandResult::Cancelled));
    cancel_flag()
        .unwrap()
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let next = exec::run_command(executable, &[], home, now() + exec::SLOT, None);
    assert!(matches!(
        next,
        CommandResult::Completed { ref stdout } if stdout.as_slice() == b"next"
    ));
    assert_eq!(
        argv_log(),
        vec![
            vec![executable.display().to_string()],
            vec![executable.display().to_string()]
        ]
    );
}

#[test]
fn refused_frontier_cancel_does_not_leak_into_the_next_command() {
    let mut plan = Plan::default();
    plan.script_command(ScriptedResult::Completed(b"retained".to_vec()));
    plan.on("provider-command", BoundaryAction::Sigint);
    install(plan);
    let _reset = ResetPlan;
    let executable = Path::new("/fake-provider");
    let home = Path::new("/");
    boundary("provider-command", Path::new(""));

    let refused = exec::run_command(
        executable,
        &[],
        home,
        now() + Duration::from_millis(600),
        Some(&InjectCancel),
    );

    assert!(matches!(refused, CommandResult::TimedOut));
    assert!(argv_log().is_empty());
    cancel_flag()
        .unwrap()
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let retained = exec::run_command(executable, &[], home, now() + exec::SLOT, None);
    assert!(matches!(
        retained,
        CommandResult::Completed { ref stdout } if stdout.as_slice() == b"retained"
    ));
}

#[test]
fn reset_clears_a_pending_frontier_cancel() {
    let mut first = Plan::default();
    first.on("provider-command", BoundaryAction::Sigint);
    install(first);
    boundary("provider-command", Path::new(""));
    reset();

    let mut second = Plan::default();
    second.script_command(ScriptedResult::Completed(b"clean".to_vec()));
    install(second);
    let _reset = ResetPlan;
    let result = exec::run_command(
        Path::new("/fake-provider"),
        &[],
        Path::new("/"),
        now() + exec::SLOT,
        None,
    );

    assert!(matches!(
        result,
        CommandResult::Completed { ref stdout } if stdout.as_slice() == b"clean"
    ));
}
