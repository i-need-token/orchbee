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
fn places_offered_follow_the_focused_panel_and_auto_is_the_first() {
    use StartPlace::{NewTab, SplitDown, SplitRight, ThisPanel};
    let focused = panel(7);
    let cases = [
        ("no focused Panel", None, false, vec![NewTab]),
        (
            "an idle Panel",
            Some(&focused),
            false,
            vec![ThisPanel(focused.clone()), NewTab, SplitRight, SplitDown],
        ),
        (
            "a busy Panel",
            Some(&focused),
            true,
            vec![NewTab, SplitRight, SplitDown],
        ),
    ];
    for (name, focus, busy, expected) in cases {
        let places = Places::new(focus, |_| busy);
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
