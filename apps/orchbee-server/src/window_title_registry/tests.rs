use super::*;
use orchbee_workbench_protocol::WindowTitle;
fn id(n: u64) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn set(title: &str) -> Action {
    Action::Set {
        title: WindowTitle::normalize(title).unwrap(),
    }
}
fn run(r: &mut Registry, generation: u64, request: u64, action: Action, at: Instant) -> Outcome {
    r.execute(
        NonZeroU64::new(generation).unwrap(),
        id(request),
        action,
        at,
    )
}
fn listen(r: &mut Registry, g: u64, focused: bool, at: Instant) -> Option<WindowTitleSnapshot> {
    let Outcome::Listening { snapshot } = run(
        r,
        g,
        999,
        Action::Listen {
            focused,
            context: Default::default(),
        },
        at,
    ) else {
        panic!()
    };
    snapshot
}
#[test]
fn retained_title_promotes_on_attach_and_focus_but_not_idle_poll() {
    let mut r = Registry::default();
    let now = Instant::now();
    assert!(matches!(
        run(&mut r, 1, 1, set("first"), now),
        Outcome::Published {
            changed: false,
            reason: Reason::NoForegroundClient,
            ..
        }
    ));
    assert_eq!(
        listen(&mut r, 2, false, now)
            .unwrap()
            .title
            .unwrap()
            .as_str(),
        "first"
    );
    assert!(listen(&mut r, 3, false, now).is_some());
    assert!(listen(&mut r, 2, false, now).is_none());
    assert!(listen(&mut r, 2, true, now).is_some());
    assert!(listen(&mut r, 3, false, now).is_none());
    assert!(matches!(
        run(&mut r, 1, 2, Action::Clear {}, now),
        Outcome::Published {
            changed: true,
            reason: Reason::Cleared,
            ..
        }
    ));
    let clear = listen(&mut r, 2, true, now).unwrap();
    assert_eq!(clear.title, None);
    assert_eq!(r.snapshot.as_ref().unwrap().revision.value(), 2);
    r.disconnect(NonZeroU64::new(2).unwrap());
    // Disabled automatic titles do not reapply an already delivered clear to a new foreground.
    assert!(listen(&mut r, 3, false, now).is_none());
}
#[test]
fn replay_never_republishes_or_uses_later_foreground_state() {
    let mut r = Registry::default();
    let now = Instant::now();
    let first = run(&mut r, 1, 1, set("first"), now);
    listen(&mut r, 2, true, now);
    run(&mut r, 1, 2, set("second"), now);
    assert_eq!(run(&mut r, 1, 1, set("first"), now), first);
    assert_eq!(
        run(&mut r, 1, 1, Action::Clear {}, now),
        Outcome::Failed {
            reason: Failure::Conflict
        }
    );
    assert_eq!(
        listen(&mut r, 2, true, now)
            .unwrap()
            .title
            .unwrap()
            .as_str(),
        "second"
    );
    assert!(matches!(
        run(
            &mut r,
            1,
            3,
            set("still connected"),
            now + Duration::from_secs(60)
        ),
        Outcome::Published { changed: true, .. }
    ));
}
#[test]
fn capacities_and_revision_overflow_fail_before_commit() {
    let mut r = Registry::default();
    let now = Instant::now();
    for i in 1..=32 {
        listen(&mut r, i, false, now);
    }
    assert_eq!(
        run(
            &mut r,
            33,
            999,
            Action::Listen {
                focused: true,
                context: Default::default()
            },
            now
        ),
        Outcome::Failed {
            reason: Failure::Busy
        }
    );
    for i in 1..=256 {
        run(&mut r, 1, i, set("same"), now);
    }
    assert_eq!(
        run(&mut r, 1, 257, Action::Clear {}, now),
        Outcome::Failed {
            reason: Failure::Busy
        }
    );
    assert_eq!(r.snapshot.as_ref().unwrap().revision.value(), 256);
    assert!(matches!(
        run(
            &mut r,
            1,
            258,
            Action::Clear {},
            now + Duration::from_secs(60)
        ),
        Outcome::Published { .. }
    ));
    r.snapshot.as_mut().unwrap().revision = WorkbenchRevision::new(u64::MAX);
    assert_eq!(
        run(
            &mut r,
            1,
            259,
            set("blocked"),
            now + Duration::from_secs(60)
        ),
        Outcome::Failed {
            reason: Failure::Unavailable
        }
    );
    assert_eq!(r.snapshot.as_ref().unwrap().title, None);
}

#[test]
fn window_title_live_template_override_clear_disable_and_context_changes() {
    let mut r = Registry::default();
    let now = Instant::now();
    r.configure("{hostname}: {workspace}/{tab}/{panel}/{terminal_title}");
    r.hostname = "SessionHost".into();
    let context = orchbee_workbench_protocol::WindowTitleContext::new(
        "Workspace",
        "Tab",
        "Manual",
        "building",
    );
    let poll = |r: &mut Registry, context: orchbee_workbench_protocol::WindowTitleContext| {
        let Outcome::Listening { snapshot } = run(
            r,
            2,
            99,
            Action::Listen {
                focused: true,
                context,
            },
            now,
        ) else {
            panic!()
        };
        snapshot
    };
    let first = poll(&mut r, context.clone()).unwrap();
    assert_eq!(
        first.title.unwrap().as_str(),
        "SessionHost: Workspace/Tab/Manual/building"
    );
    run(&mut r, 1, 1, set("Manual override"), now);
    r.configure("{tab}");
    assert_eq!(
        poll(&mut r, context.clone())
            .unwrap()
            .title
            .unwrap()
            .as_str(),
        "Manual override"
    );
    run(&mut r, 1, 2, Action::Clear {}, now);
    assert_eq!(
        poll(&mut r, context.clone())
            .unwrap()
            .title
            .unwrap()
            .as_str(),
        "Tab"
    );
    let mut changed = context.clone();
    changed.tab = "Next".into();
    let next = poll(&mut r, changed.clone()).unwrap();
    assert_eq!(next.title.as_ref().unwrap().as_str(), "Next");
    assert_eq!(poll(&mut r, changed), Some(next));
    for disabled in ["", "{unknown}"] {
        r.configure(disabled);
        assert_eq!(poll(&mut r, context.clone()), None);
    }
    run(&mut r, 1, 3, Action::Clear {}, now);
    assert_eq!(poll(&mut r, context.clone()).unwrap().title, None);
    assert_eq!(poll(&mut r, context).unwrap().title, None);
}
