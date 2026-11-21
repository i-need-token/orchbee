//! The per-run decision under a controlled clock.
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use super::super::layout::Dialog;
use super::super::run::{Choice, Frame, Gate, Outcome, Refusal, Run, Seen, Settings, Step};
use super::super::{Kind, Wait};

const ON: Settings = Settings {
    agent_type: true,
    offer: true,
};
const OFF: Settings = Settings {
    agent_type: false,
    offer: true,
};

fn dialog(command: &str) -> Dialog {
    Dialog {
        layout: "bash".into(),
        kind: Kind::Command,
        text: vec![command.into()],
        marked: false,
    }
}

fn frame(revision: u64) -> Frame {
    Frame {
        generation: NonZeroU64::MIN,
        revision,
    }
}

/// A run observed on a clock the test moves by hand.
struct Bench {
    run: Run,
    start: Instant,
    now: Duration,
    revision: u64,
}

impl Bench {
    fn new() -> Self {
        Self {
            run: Run::new(),
            start: Instant::now(),
            now: Duration::ZERO,
            revision: 10,
        }
    }

    fn see(
        &mut self,
        settings: Settings,
        milliseconds: u64,
        dialog: Option<(&Dialog, Result<(), Wait>)>,
    ) -> Step {
        self.now += Duration::from_millis(milliseconds);
        self.run.observe(
            self.start + self.now,
            settings,
            Seen {
                frame: frame(self.revision),
                blocked: true,
                dialog,
            },
        )
    }

    /// Observes the same prompt until it has settled and returns what the run then asks for.
    fn settle(&mut self, settings: Settings, dialog: &Dialog) -> Step {
        let mut step = Step::Nothing;
        for _ in 0..4 {
            assert_eq!(step, Step::Nothing);
            step = self.see(settings, 400, Some((dialog, Ok(()))));
        }
        step
    }

    fn end(&mut self, outcome: Outcome) {
        self.run.pressed(self.start + self.now, outcome);
    }

    /// One whole answered prompt: it appears, settles, is pressed and goes away.
    fn answer(&mut self, dialog: &Dialog) -> Step {
        self.revision += 1;
        let step = self.settle(ON, dialog);
        if matches!(step, Step::Press(_)) {
            self.end(Outcome::Written);
            self.revision += 1;
            assert_eq!(self.see(ON, 50, None), Step::Nothing);
        }
        step
    }
}

fn press(revision: u64, first_revision: u64) -> Step {
    Step::Press(Gate {
        frame: frame(revision),
        first_revision,
    })
}

#[test]
fn a_prompt_is_pressed_once_after_it_stayed_the_same_for_a_second_over_three_looks() {
    let mut bench = Bench::new();
    let prompt = dialog("cargo test");
    // Three looks inside one second are not enough, and neither are two looks over a second.
    assert_eq!(bench.see(ON, 0, Some((&prompt, Ok(())))), Step::Nothing);
    assert_eq!(bench.see(ON, 400, Some((&prompt, Ok(())))), Step::Nothing);
    assert_eq!(bench.see(ON, 400, Some((&prompt, Ok(())))), Step::Nothing);
    // The screen redrew meanwhile: the key names the frame read now and the revision first seen.
    bench.revision = 12;
    assert_eq!(bench.see(ON, 200, Some((&prompt, Ok(())))), press(12, 10));
    bench.end(Outcome::Written);
    assert_eq!(bench.see(ON, 5000, Some((&prompt, Ok(())))), Step::Nothing);
    let facts = bench.run.facts(ON);
    assert_eq!(
        facts.pressed.map(|pressed| pressed.text),
        Some("cargo test".into())
    );

    let mut slow = Bench::new();
    assert_eq!(slow.see(ON, 0, Some((&prompt, Ok(())))), Step::Nothing);
    assert_eq!(slow.see(ON, 1500, Some((&prompt, Ok(())))), Step::Nothing);
    assert_eq!(slow.see(ON, 50, Some((&prompt, Ok(())))), press(10, 10));
}

#[test]
fn another_prompt_starts_its_own_wait() {
    let mut bench = Bench::new();
    let first = dialog("cargo test");
    let second = dialog("cargo build");
    assert_eq!(bench.see(ON, 0, Some((&first, Ok(())))), Step::Nothing);
    assert_eq!(bench.see(ON, 900, Some((&first, Ok(())))), Step::Nothing);
    bench.revision = 20;
    assert_eq!(bench.see(ON, 900, Some((&second, Ok(())))), Step::Nothing);
    assert_eq!(bench.see(ON, 500, Some((&second, Ok(())))), Step::Nothing);
    assert_eq!(bench.see(ON, 500, Some((&second, Ok(())))), press(20, 20));
}

#[test]
fn a_prompt_that_must_wait_is_never_pressed_and_says_why() {
    let mut bench = Bench::new();
    let prompt = dialog("rm -rf build");
    for _ in 0..6 {
        assert_eq!(
            bench.see(ON, 500, Some((&prompt, Err(Wait::Destructive)))),
            Step::Nothing
        );
    }
    assert_eq!(bench.run.facts(ON).wait, Some(Wait::Destructive));
    // The Agent waits on something no layout reads.
    assert_eq!(bench.see(ON, 500, None), Step::Nothing);
    assert_eq!(bench.run.facts(ON).wait, Some(Wait::NotAnswered));
    // Off, the row gives no reason: nothing changes from today.
    assert_eq!(Bench::new().run.facts(OFF).wait, None);
}

#[test]
fn each_refusal_is_followed_as_its_rule_says() {
    let prompt = dialog("cargo test");
    // The screen changed, input was queued, or the revision was typed on: decide again.
    for refusal in [
        Refusal::Changed,
        Refusal::InputPending,
        Refusal::AlreadyTyped,
        Refusal::AgentGone,
    ] {
        let mut bench = Bench::new();
        assert_eq!(bench.settle(ON, &prompt), press(10, 10));
        bench.end(Outcome::Refused(refusal));
        bench.revision = 11;
        assert_eq!(
            bench.see(ON, 50, Some((&prompt, Ok(())))),
            press(11, 10),
            "{refusal:?}"
        );
    }
    // A person was there since the prompt appeared: the prompt is theirs.
    let mut bench = Bench::new();
    assert_eq!(bench.settle(ON, &prompt), press(10, 10));
    bench.end(Outcome::Refused(Refusal::PersonInput));
    assert_eq!(
        bench.see(ON, 10_000, Some((&prompt, Ok(())))),
        Step::Nothing
    );
    assert_eq!(bench.run.facts(ON).wait, Some(Wait::Person));
    // A person was there shortly before: decide again once the quiet time has passed.
    let mut bench = Bench::new();
    assert_eq!(bench.settle(ON, &prompt), press(10, 10));
    bench.end(Outcome::Refused(Refusal::PersonRecent));
    assert_eq!(bench.see(ON, 2900, Some((&prompt, Ok(())))), Step::Nothing);
    assert_eq!(bench.see(ON, 200, Some((&prompt, Ok(())))), press(10, 10));
    // A Host without the gate: nothing is sent again in this run.
    let mut bench = Bench::new();
    assert_eq!(bench.settle(ON, &prompt), press(10, 10));
    bench.end(Outcome::Unsupported);
    assert_eq!(bench.see(ON, 5000, Some((&prompt, Ok(())))), Step::Nothing);
    assert_eq!(bench.run.facts(ON).wait, Some(Wait::HostTooOld));
    // No answer: the key may have been written, so it counts as written.
    let mut bench = Bench::new();
    assert_eq!(bench.settle(ON, &prompt), press(10, 10));
    bench.end(Outcome::Unknown);
    assert_eq!(bench.see(ON, 5000, Some((&prompt, Ok(())))), Step::Nothing);
    // A key on its way is not sent twice.
    let mut bench = Bench::new();
    assert_eq!(bench.settle(ON, &prompt), press(10, 10));
    assert_eq!(bench.see(ON, 5000, Some((&prompt, Ok(())))), Step::Nothing);
}

#[test]
fn turning_off_owes_the_host_a_hold_and_shows_off_once_it_is_answered() {
    let prompt = dialog("cargo test");
    let mut bench = Bench::new();
    assert_eq!(bench.settle(ON, &prompt), press(10, 10));
    // The key is on its way when the person turns the Agent type off.
    assert_eq!(bench.see(OFF, 50, Some((&prompt, Ok(())))), Step::Hold);
    assert_eq!(bench.run.facts(OFF).choice, Choice::Run);
    bench.run.held();
    assert_eq!(bench.run.facts(OFF).choice, Choice::Off);
    assert_eq!(bench.see(OFF, 50, Some((&prompt, Ok(())))), Step::Nothing);

    // "This Agent only" is the run's own choice and turns off the same way.
    let mut bench = Bench::new();
    bench.run.choose(true);
    assert_eq!(bench.settle(OFF, &prompt), press(10, 10));
    assert_eq!(bench.run.facts(OFF).choice, Choice::Run);
    bench.run.choose(false);
    assert_eq!(bench.see(OFF, 50, Some((&prompt, Ok(())))), Step::Hold);
    // A run that was never on owes nothing.
    assert_eq!(
        Bench::new().see(OFF, 0, Some((&prompt, Ok(())))),
        Step::Nothing
    );
}

#[test]
fn the_same_prompt_coming_back_after_three_presses_in_a_row_stops_the_run() {
    let prompt = dialog("cargo test");
    let other = dialog("cargo build");
    let mut bench = Bench::new();
    for _ in 0..3 {
        assert!(matches!(bench.answer(&prompt), Step::Press(_)));
    }
    assert_eq!(bench.answer(&prompt), Step::Hold);
    let facts = bench.run.facts(ON);
    assert!(facts.stopped);
    assert_eq!(facts.wait, Some(Wait::Repeating));
    bench.run.held();
    assert_eq!(bench.run.facts(ON).choice, Choice::Off);
    assert_eq!(bench.see(ON, 5000, Some((&prompt, Ok(())))), Step::Nothing);
    // The person may turn it on again.
    bench.run.choose(true);
    assert!(matches!(bench.answer(&other), Step::Press(_)));

    // Another prompt answered in between is progress, and the count starts over. A send with
    // no answer counts like a written one.
    let mut bench = Bench::new();
    for _ in 0..2 {
        assert!(matches!(bench.answer(&prompt), Step::Press(_)));
        assert!(matches!(bench.answer(&prompt), Step::Press(_)));
        assert!(matches!(bench.answer(&other), Step::Press(_)));
    }
    let mut bench = Bench::new();
    for _ in 0..3 {
        bench.revision += 1;
        assert!(matches!(bench.settle(ON, &prompt), Step::Press(_)));
        bench.end(Outcome::Unknown);
        assert_eq!(bench.see(ON, 50, None), Step::Nothing);
    }
    assert_eq!(bench.answer(&prompt), Step::Hold);
}

#[test]
fn a_run_offers_at_most_twice_and_only_on_a_prompt_it_would_answer() {
    let mut bench = Bench::new();
    let risky = dialog("rm -rf build");
    for _ in 0..5 {
        assert_eq!(
            bench.see(OFF, 500, Some((&risky, Err(Wait::Destructive)))),
            Step::Nothing
        );
    }
    assert!(!bench.run.facts(OFF).offer);
    for round in 0..3 {
        let prompt = dialog(&format!("cargo test --package p{round}"));
        assert_eq!(bench.settle(OFF, &prompt), Step::Nothing);
        assert_eq!(bench.run.facts(OFF).offer, round < 2, "round {round}");
        // The person answers the Agent themselves, and the offer goes with the prompt.
        assert_eq!(bench.see(OFF, 50, None), Step::Nothing);
        assert!(!bench.run.facts(OFF).offer);
    }
    // After "no" nothing is offered, and an accepted offer presses the prompt it was shown on.
    let never = Settings {
        agent_type: false,
        offer: false,
    };
    let prompt = dialog("cargo test");
    let mut bench = Bench::new();
    assert_eq!(bench.settle(never, &prompt), Step::Nothing);
    assert!(!bench.run.facts(never).offer);
    let mut bench = Bench::new();
    assert_eq!(bench.settle(OFF, &prompt), Step::Nothing);
    assert!(bench.run.facts(OFF).offer);
    bench.run.choose(true);
    assert_eq!(bench.see(OFF, 50, Some((&prompt, Ok(())))), press(10, 10));
    assert!(!bench.run.facts(OFF).offer);
}
