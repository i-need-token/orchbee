//! The remember box's rules: offered only for a password or passphrase while a store can be used,
//! and a ticked answer leaves for the store only after the login succeeded, as the answer that
//! worked.

use super::*;

const WORD: &str = "test-word-one";
const OTHER_WORD: &str = "test-word-two";

/// A run whose store keeps answers as `remembering` says, with the password question shown.
fn asked(remembering: Remembering) -> Run {
    let mut run = Run::new();
    run.logins.set_remembering(remembering);
    run.step(Step::Claim(1));
    run.step(password(1, false));
    run
}

fn tick(run: &mut Run, ticked: bool) {
    let token = run.token.expect("a question was shown");
    run.logins.remember(token, ticked);
}

fn answer(run: &mut Run, word: &'static str) {
    run.step(Step::Type(word));
    run.step(Step::Act(Act::Submit));
    run.step(Step::Delivered(Delivery::Taken));
}

#[test]
fn the_box_is_offered_only_where_an_answer_can_be_remembered() {
    let run = asked(Remembering::Lasting);
    assert_eq!(
        run.logins.presented().unwrap().remember,
        RememberBox::Offered {
            ticked: false,
            until_restart: false
        },
        "unticked by default"
    );
    let run = asked(Remembering::UntilRestart);
    assert!(matches!(
        run.logins.presented().unwrap().remember,
        RememberBox::Offered {
            until_restart: true,
            ..
        }
    ));
    let run = asked(Remembering::Unavailable);
    assert_eq!(
        run.logins.presented().unwrap().remember,
        RememberBox::Unavailable
    );
    let mut run = Run::new();
    run.logins.set_remembering(Remembering::Lasting);
    run.step(Step::Claim(1));
    run.step(Step::Ask {
        login: 1,
        question: 1,
        prompt: NEW_HOST,
        repeated: false,
    });
    assert_eq!(
        run.logins.presented().unwrap().remember,
        RememberBox::Absent,
        "a new host is never remembered"
    );
}

#[test]
fn only_a_ticked_answer_of_a_login_that_succeeded_is_remembered() {
    let ends = [
        (true, LoginEnd::Succeeded, true),
        (false, LoginEnd::Succeeded, false),
        (true, LoginEnd::Failed, false),
        (true, LoginEnd::Dismissed, false),
        (true, LoginEnd::Abandoned, false),
    ];
    for (ticked, end, remembered) in ends {
        let mut run = asked(Remembering::Lasting);
        tick(&mut run, ticked);
        answer(&mut run, WORD);
        let expected = if remembered {
            vec![Did::Remember(
                SecretKind::Password,
                WORD.as_bytes().to_vec(),
            )]
        } else {
            Vec::new()
        };
        assert_eq!(run.step(Step::Ended(1, end)), expected, "{ticked} {end:?}");
    }
}

#[test]
fn the_answer_remembered_is_the_one_that_worked() {
    let mut run = asked(Remembering::Lasting);
    tick(&mut run, true);
    answer(&mut run, WORD);
    run.step(password(2, true));
    answer(&mut run, OTHER_WORD);
    assert_eq!(
        run.step(Step::Ended(1, LoginEnd::Succeeded)),
        [Did::Remember(
            SecretKind::Password,
            OTHER_WORD.as_bytes().to_vec()
        )]
    );
}

#[test]
fn nothing_is_kept_when_no_store_can_be_used() {
    let mut run = asked(Remembering::Unavailable);
    tick(&mut run, true);
    answer(&mut run, WORD);
    assert!(run.step(Step::Ended(1, LoginEnd::Succeeded)).is_empty());
}

#[test]
fn a_refused_remembered_answer_is_said_at_the_question() {
    let mut run = Run::new();
    run.logins.set_remembering(Remembering::Lasting);
    run.step(Step::Claim(1));
    let mut question = ask(1, 1, PASSWORD, false, hub_kind(PASSWORD));
    question.remembered_rejected = true;
    run.logins
        .notice(Notice::Question(question), |_| Some(TARGET.to_owned()));
    assert_eq!(
        run.logins.presented().unwrap().notice,
        Some(AskNotice::RememberedRejected)
    );
}
