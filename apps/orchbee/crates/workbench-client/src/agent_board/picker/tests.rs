use orchbee_workbench_protocol::WorkbenchPanelId;

use super::super::start::{Phase, StartPlace, place_after};
use super::{Escape, Group, Item, Meta, Picker, Places, Sources, escape, items};
use crate::workbench_state::AgentStartRefusal;

fn panel(value: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&format!("{value:032x}")).unwrap()
}

fn all() -> Vec<&'static str> {
    crate::agent_provider::choices()
        .map(|choice| choice.id)
        .collect()
}

/// Each group's providers in display order, plus the folded count.
fn groups(rows: &[Item]) -> (Vec<(Group, Vec<&'static str>)>, Option<usize>) {
    let mut groups: Vec<(Group, Vec<&'static str>)> = Vec::new();
    let mut fold = None;
    for row in rows {
        match *row {
            Item::Heading(group) => groups.push((group, Vec::new())),
            Item::Provider { id, group, .. } => {
                let last = groups.last_mut().expect("a provider follows its heading");
                assert_eq!(last.0, group, "{id} sits under its own heading");
                last.1.push(id);
            }
            Item::Fold(count) => fold = Some(count),
        }
    }
    (groups, fold)
}

fn group(rows: &[Item], wanted: Group) -> Vec<&'static str> {
    groups(rows)
        .0
        .into_iter()
        .find(|(group, _)| *group == wanted)
        .map(|(_, ids)| ids)
        .unwrap_or_default()
}

fn sorted_by_product(ids: &[&'static str]) -> bool {
    ids.windows(2).all(|pair| {
        crate::agent_provider::product_name(pair[0]).to_lowercase()
            <= crate::agent_provider::product_name(pair[1]).to_lowercase()
    })
}

#[test]
fn groups_hold_recent_then_installed_then_the_rest_once_each() {
    struct Case {
        name: &'static str,
        recent: &'static [&'static str],
        installed: Option<&'static [&'static str]>,
        missing: &'static [&'static str],
        expect_recent: &'static [&'static str],
        expect_rest: Group,
        expect_not_found: &'static [&'static str],
    }
    let cases = [
        Case {
            name: "recent keeps only installed providers, most recent first",
            recent: &["gemini", "pi", "codex"],
            installed: Some(&["codex", "gemini", "claude-code"]),
            missing: &[],
            expect_recent: &["gemini", "codex"],
            expect_rest: Group::Installed,
            expect_not_found: &[],
        },
        Case {
            name: "recent holds at most three",
            recent: &["codex", "gemini", "claude-code", "kimi"],
            installed: Some(&["kimi", "codex", "gemini", "claude-code"]),
            missing: &[],
            expect_recent: &["codex", "gemini", "claude-code"],
            expect_rest: Group::Installed,
            expect_not_found: &[],
        },
        Case {
            name: "a provider whose start found no command leaves recent and installed",
            recent: &["codex"],
            installed: Some(&["codex", "gemini"]),
            missing: &["codex"],
            expect_recent: &[],
            expect_rest: Group::Installed,
            expect_not_found: &[],
        },
        Case {
            name: "a remote Session lists recent, then everything not checked",
            recent: &["codex", "gemini"],
            installed: None,
            missing: &["gemini"],
            expect_recent: &["codex"],
            expect_rest: Group::NotChecked,
            expect_not_found: &["gemini"],
        },
    ];
    for case in cases {
        let rows = items(
            Sources {
                recent: case.recent,
                installed: case.installed,
                missing: case.missing,
                pending: &[],
            },
            "",
            false,
        );
        let (listed, fold) = groups(&rows);
        assert_eq!(
            group(&rows, Group::Recent),
            case.expect_recent,
            "{}",
            case.name
        );
        let rest = group(&rows, case.expect_rest);
        assert!(sorted_by_product(&rest), "{}: rest is A–Z", case.name);
        if let Some(installed) = case.installed {
            let mut expected: Vec<_> = installed
                .iter()
                .copied()
                .filter(|id| !case.missing.contains(id) && !case.expect_recent.contains(id))
                .collect();
            expected.sort_unstable();
            let mut actual = rest.clone();
            actual.sort_unstable();
            assert_eq!(actual, expected, "{}: installed group", case.name);
            let available = installed
                .iter()
                .filter(|id| !case.missing.contains(id))
                .count();
            assert_eq!(
                fold,
                Some(all().len() - available),
                "{}: the rest folds",
                case.name
            );
        } else {
            assert_eq!(fold, None, "{}: a remote list never folds", case.name);
        }
        assert_eq!(
            group(&rows, Group::NotFound),
            case.expect_not_found,
            "{}",
            case.name
        );
        let mut seen: Vec<_> = listed.iter().flat_map(|(_, ids)| ids.clone()).collect();
        let count = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen.len(),
            count,
            "{}: no provider is listed twice",
            case.name
        );
        if case.installed.is_none() {
            assert_eq!(
                count,
                all().len(),
                "{}: every provider is listed",
                case.name
            );
        }
    }
}

#[test]
fn not_found_rows_say_why_and_generic_commands_cannot_be_checked() {
    let rows = items(
        Sources {
            recent: &[],
            installed: Some(&["codex"]),
            missing: &["gemini"],
            pending: &[],
        },
        "",
        true,
    );
    let meta = |wanted: &str| {
        rows.iter().find_map(|row| match *row {
            Item::Provider { id, group, meta } if id == wanted => Some((group, meta)),
            _ => None,
        })
    };
    assert_eq!(meta("codex"), Some((Group::Installed, Meta::Command)));
    assert_eq!(meta("gemini"), Some((Group::NotFound, Meta::Missing)));
    assert_eq!(meta("claude-code"), Some((Group::NotFound, Meta::Missing)));
    for generic in crate::agent_provider::choices().filter(|choice| !choice.checkable) {
        assert_eq!(
            meta(generic.id),
            Some((Group::NotFound, Meta::Generic)),
            "{}",
            generic.id
        );
    }
}

#[test]
fn query_matches_name_command_or_id_and_searches_the_folded_group() {
    let sources = Sources {
        recent: &[],
        installed: Some(&["codex"]),
        missing: &[],
        pending: &[],
    };
    let cases: [(&str, &str, &str); 5] = [
        ("by product name, any case", "CLAUDE", "claude-code"),
        ("by command", "agy", "antigravity"),
        ("by fixed launch argument", "openclaw chat", "openclaw"),
        ("by id", "github-copilot", "github-copilot"),
        ("installed and not found alike", "codex", "codex"),
    ];
    for (name, query, wanted) in cases {
        let rows = items(sources, query, false);
        let listed = groups(&rows);
        assert!(
            listed.0.iter().any(|(_, ids)| ids.contains(&wanted)),
            "{name}: {query} lists {wanted}"
        );
        assert_eq!(listed.1, None, "{name}: a query never folds");
        for (_, ids) in &listed.0 {
            for id in ids {
                assert!(super::matches(id, query.trim()), "{name}: {id} matches");
            }
        }
    }
    let unfolded = items(sources, "", true);
    assert_eq!(groups(&unfolded).1, None, "an opened fold stays open");
    assert!(!group(&unfolded, Group::NotFound).is_empty());
    assert!(items(sources, "no such agent", false).is_empty());
}

#[test]
fn the_cursor_rests_on_the_most_recent_provider_else_the_first_installed() {
    let cases: [(&[&str], &str); 2] = [(&["gemini"], "gemini"), (&[], "claude-code")];
    for (recent, expected) in cases {
        let rows = items(
            Sources {
                recent,
                installed: Some(&["codex", "gemini", "claude-code"]),
                missing: &[],
                pending: &[],
            },
            "",
            false,
        );
        let picker = Picker::new(false, Places::new(None, |_| false));
        let index = picker.cursor_index(&rows).unwrap();
        assert!(
            matches!(rows[index], Item::Provider { id, .. } if id == expected),
            "{recent:?}"
        );
    }
}

#[test]
fn a_provider_counts_as_not_found_only_after_its_check_completed() {
    struct Case {
        name: &'static str,
        found: &'static [&'static str],
        /// Whether the checks of the providers not found yet are still running.
        rest_pending: bool,
        missing: &'static [&'static str],
        expect_fold: Option<usize>,
        /// Not found besides the generic commands, which are never checked.
        expect_not_found: &'static [&'static str],
    }
    let checkable: Vec<&'static str> = crate::agent_provider::choices()
        .filter(|choice| choice.checkable)
        .map(|choice| choice.id)
        .collect();
    let generic: Vec<&'static str> = crate::agent_provider::choices()
        .filter(|choice| !choice.checkable)
        .map(|choice| choice.id)
        .collect();
    let cases = [
        Case {
            name: "nothing checked yet",
            found: &[],
            rest_pending: true,
            missing: &[],
            expect_fold: None,
            expect_not_found: &[],
        },
        Case {
            name: "results arriving",
            found: &["gemini", "codex"],
            rest_pending: true,
            missing: &[],
            expect_fold: None,
            expect_not_found: &[],
        },
        Case {
            name: "a start that found no command is not found while checks run",
            found: &[],
            rest_pending: true,
            missing: &["claude-code"],
            expect_fold: None,
            expect_not_found: &["claude-code"],
        },
        Case {
            name: "every check completed",
            found: &["codex"],
            rest_pending: false,
            missing: &[],
            expect_fold: Some(all().len() - 1),
            expect_not_found: &[],
        },
    ];
    for case in cases {
        let pending: Vec<&'static str> = checkable
            .iter()
            .copied()
            .filter(|id| case.rest_pending && !case.found.contains(id))
            .collect();
        let sources = Sources {
            recent: &[],
            installed: Some(case.found),
            missing: case.missing,
            pending: &pending,
        };
        let folded = items(sources, "", false);
        let mut installed = group(&folded, Group::Installed);
        installed.sort_unstable();
        let mut expected: Vec<_> = case.found.to_vec();
        expected.sort_unstable();
        assert_eq!(installed, expected, "{}: found are installed", case.name);
        assert_eq!(groups(&folded).1, case.expect_fold, "{}: fold", case.name);
        let mut not_found = group(&items(sources, "", true), Group::NotFound);
        not_found.sort_unstable();
        let mut expected: Vec<&'static str> = if case.rest_pending {
            generic
                .iter()
                .chain(case.expect_not_found)
                .copied()
                .collect()
        } else {
            all()
                .into_iter()
                .filter(|id| !case.found.contains(id))
                .collect()
        };
        expected.sort_unstable();
        assert_eq!(not_found, expected, "{}: not found", case.name);
        for id in pending.iter().filter(|id| !case.missing.contains(id)) {
            for rows in [
                &folded,
                &items(sources, "", true),
                &items(sources, id, false),
            ] {
                assert!(
                    !rows.iter().any(
                        |row| matches!(row, Item::Provider { id: listed, .. } if listed == id)
                    ),
                    "{}: pending {id} is listed nowhere",
                    case.name
                );
            }
        }
    }
}

#[test]
fn a_late_lookup_result_never_moves_the_cursor() {
    let mut board = super::super::AgentBoard::new(true);
    board.open_picker(None, None, false);
    board.start.begin_lookup_finding(&["gemini"]);
    let cursor = |board: &super::super::AgentBoard| {
        let rows = board.picker_items();
        let index = board.picker().unwrap().cursor_index(&rows).unwrap();
        match rows[index] {
            Item::Provider { id, .. } => id,
            other => panic!("cursor on {other:?}"),
        }
    };
    assert_eq!(cursor(&board), "gemini");
    // Codex sorts above Gemini once its check finds it.
    let changed = board.drain_start(std::time::Instant::now(), |state, _| {
        super::super::start::Drain {
            changed: state.find("codex"),
            started: false,
        }
    });
    assert!(changed);
    assert_eq!(
        group(&board.picker_items(), Group::Installed),
        ["codex", "gemini"]
    );
    assert_eq!(cursor(&board), "gemini");
}

#[test]
fn places_offered_follow_the_focused_panel_and_auto_is_the_first() {
    use StartPlace::{NewTab, NewWorktree, SplitDown, SplitRight, ThisPanel};
    let focused = panel(7);
    let cases = [
        ("no focused Panel", None, false, false, vec![NewTab]),
        (
            "an idle Panel",
            Some(&focused),
            false,
            false,
            vec![ThisPanel(focused.clone()), NewTab, SplitRight, SplitDown],
        ),
        (
            "a busy Panel",
            Some(&focused),
            true,
            false,
            vec![NewTab, SplitRight, SplitDown],
        ),
        (
            "a worktree can be created",
            Some(&focused),
            false,
            true,
            vec![
                ThisPanel(focused.clone()),
                NewTab,
                SplitRight,
                SplitDown,
                NewWorktree,
            ],
        ),
        (
            "a worktree but no focused Panel",
            None,
            false,
            true,
            vec![NewTab, NewWorktree],
        ),
    ];
    for (name, focus, busy, worktree, expected) in cases {
        let places = Places::new(focus, |_| busy).with_worktree(worktree);
        assert_eq!(places.offered(), expected, "{name}");
        assert_eq!(places.automatic(), expected[0], "{name}");
    }
}

#[test]
fn a_typed_start_refused_at_its_prompt_moves_to_a_new_tab_once() {
    let settled = [
        (
            "became busy",
            true,
            Phase::Refused(AgentStartRefusal::NotAtPrompt),
            Some(StartPlace::NewTab),
        ),
        (
            "prompt never marked",
            true,
            Phase::Refused(AgentStartRefusal::Unmarked),
            Some(StartPlace::NewTab),
        ),
        (
            "a new Tab never moves again",
            false,
            Phase::Refused(AgentStartRefusal::NotAtPrompt),
            None,
        ),
        ("started", true, Phase::Started, None),
        ("not found", true, Phase::NotFound("Codex"), None),
        ("outcome unknown", true, Phase::Unknown, None),
    ];
    for (name, fallback, phase, expected) in settled {
        assert_eq!(place_after(fallback, phase), expected, "{name}");
    }
}

#[test]
fn tab_cycles_the_places_forward_and_shift_tab_backward_wrapping() {
    use StartPlace::{NewTab, SplitDown, SplitRight, ThisPanel};
    let focused = panel(7);
    let cases = [
        (
            "idle",
            Places::new(Some(&focused), |_| false),
            vec![ThisPanel(focused.clone()), NewTab, SplitRight, SplitDown],
        ),
        ("no Panel", Places::new(None, |_| false), vec![NewTab]),
    ];
    for (name, places, order) in cases {
        let len = order.len();
        for (index, place) in order.iter().enumerate() {
            assert_eq!(places.next(place, true), order[(index + 1) % len], "{name}");
            assert_eq!(
                places.next(place, false),
                order[(index + len - 1) % len],
                "{name}"
            );
        }
    }
}

#[test]
fn esc_clears_the_query_then_leaves_the_picker() {
    let cases = [
        (false, false, Escape::ClearQuery),
        (false, true, Escape::ClearQuery),
        (true, true, Escape::BackToBoard),
        (true, false, Escape::Close),
    ];
    for (query_empty, from_board, expected) in cases {
        assert_eq!(
            escape(query_empty, from_board),
            expected,
            "{query_empty} {from_board}"
        );
    }
}

#[test]
fn recent_puts_own_starts_first_then_the_sessions_agents_newest_first_once_each() {
    use super::super::session_agents::{SessionAgents, recent};
    let agents = SessionAgents::with(&[
        ("amp", panel(1), 4),
        ("codex", panel(2), 9),
        ("", panel(3), 12),
        ("amp", panel(4), 7),
    ]);
    for (own, expected) in [
        (&[][..], &["codex", "amp"][..]),
        (&["amp"][..], &["amp", "codex"][..]),
        (&["grok", "codex"][..], &["grok", "codex", "amp"][..]),
    ] {
        assert_eq!(recent(own, &agents), expected, "{own:?}");
    }
    assert!(
        agents.holds(&panel(3)),
        "an unnamed Agent still holds its Panel"
    );
    assert!(!agents.holds(&panel(5)));
}
