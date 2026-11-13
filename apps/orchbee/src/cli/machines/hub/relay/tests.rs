//! The time and no-effect rules a real hub reaches only by chance: which pairs a closing hub ends
//! at once, which it lets finish until the allowance, and how a pair reads when its link is lost or
//! the far end resets it.
//! The relaying itself runs through real processes in `tests/cli_windows_machine_hub.rs`.

use super::{ALLOWANCE, Ending, carrier_end, closing, closing_done, link_reset_end};
use orchbee_machine_protocol::{
    ResetReason,
    hub::{ChannelKind, Outcome},
};
use std::time::{Duration, Instant};

#[test]
fn a_lost_link_claims_no_effect_only_where_none_can_have_started() {
    // A `session` has an effect from its `accept`, a `route` from the dispatch of its `end`.
    assert_eq!(
        carrier_end(ChannelKind::Session, false, false),
        Ending::Reset(ResetReason::LinkLost)
    );
    assert_eq!(
        carrier_end(ChannelKind::Session, true, false),
        Ending::Close(Outcome::Lost)
    );
    for accepted in [false, true] {
        assert_eq!(
            carrier_end(ChannelKind::Route, accepted, false),
            Ending::Reset(ResetReason::LinkLost)
        );
        assert_eq!(
            carrier_end(ChannelKind::Route, accepted, true),
            Ending::Close(Outcome::Lost)
        );
    }
}

#[test]
fn a_far_reset_reaches_the_client_only_while_no_effect_can_have_started() {
    for reason in [
        ResetReason::Stopping,
        ResetReason::Unavailable,
        ResetReason::SessionNotRunning,
    ] {
        // Before the dispatch of its `end` a route has caused nothing, so the reason stands.
        for accepted in [false, true] {
            assert_eq!(
                link_reset_end(ChannelKind::Route, accepted, false, reason),
                Ending::Reset(reason)
            );
            // After it the far end's reset cannot state that the command did not run.
            assert_eq!(
                link_reset_end(ChannelKind::Route, accepted, true, reason),
                Ending::Close(Outcome::Lost)
            );
        }
        assert_eq!(
            link_reset_end(ChannelKind::Session, false, false, reason),
            Ending::Reset(reason)
        );
        assert_eq!(
            link_reset_end(ChannelKind::Session, true, false, reason),
            Ending::Close(Outcome::Lost)
        );
    }
}

#[test]
fn closing_ends_sessions_at_once_and_lets_routes_finish_until_the_allowance() {
    let began = Instant::now();
    let just_before = began + ALLOWANCE - Duration::from_millis(1);
    let at_allowance = began + ALLOWANCE;
    // A Session stream ends the moment closing begins, `lost` once it may have had an effect.
    assert_eq!(
        closing(ChannelKind::Session, true, false, began, began),
        Ending::Close(Outcome::Lost)
    );
    assert_eq!(
        closing(ChannelKind::Session, false, false, began, began),
        Ending::Reset(ResetReason::Stopping)
    );
    // A route runs on through the allowance, then ends as a lost link would end it.
    for (accepted, dispatched) in [(false, false), (true, false), (true, true), (false, true)] {
        assert_eq!(
            closing(ChannelKind::Route, accepted, dispatched, began, just_before),
            Ending::Keep
        );
        assert_eq!(
            closing(
                ChannelKind::Route,
                accepted,
                dispatched,
                began,
                at_allowance
            ),
            carrier_end(ChannelKind::Route, accepted, dispatched)
        );
    }
}

#[test]
fn a_closing_hub_says_goodbye_once_nothing_runs_or_the_allowance_passed() {
    let began = Instant::now();
    let later = began + Duration::from_secs(5);
    assert!(closing_done(0, 0, began, began));
    assert!(!closing_done(1, 0, began, later));
    assert!(!closing_done(0, 1, began, later));
    assert!(closing_done(3, 2, began, began + ALLOWANCE));
}
