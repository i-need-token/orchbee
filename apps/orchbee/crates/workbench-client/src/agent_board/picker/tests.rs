use orchbee_workbench_protocol::{AgentCatalogEntryState, WorkbenchAgentCatalog, WorkbenchPanelId};
use serde::Deserialize;

use super::super::start::{Phase, StartPlace, place_after, test_catalog};
use super::{
    Escape, Group, Holds, Item, Meta, Picker, Places, Sources, Status, escape, items, menu_agents,
    status,
};
use crate::workbench_state::AgentStartRefusal;

fn panel(value: u128) -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&format!("{value:032x}")).unwrap()
}

/// Each group's providers in display order.
fn group(rows: &[Item], wanted: Group) -> Vec<&'static str> {
    rows.iter()
        .filter_map(|row| match *row {
            Item::Provider { id, group, .. } if group == wanted => Some(id),
            _ => None,
        })
        .collect()
}

/// `catalog` with every provider in `checking` still being checked by the running lookup.
fn checking(mut catalog: WorkbenchAgentCatalog, checking: &[&str]) -> WorkbenchAgentCatalog {
    for entry in &mut catalog.agents {
        if checking.contains(&entry.provider_id.agent().id()) {
            entry.state = AgentCatalogEntryState::Checking;
        }
    }
    catalog
}

/// The shared Agent start rows corpus: the rows and status row both Clients show for a catalog,
/// a query and the fold's state.
#[test]
fn the_picker_lists_every_corpus_case_as_the_shared_rule_says() {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Corpus {
        contract_version: u32,
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        name: String,
        catalog: WorkbenchAgentCatalog,
        query: String,
        expanded: bool,
        expected: Expected,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        status: Option<String>,
        rows: Vec<Row>,
    }
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    #[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
    enum Row {
        Heading {
            group: String,
            holds: Option<String>,
        },
        Provider {
            #[serde(rename = "providerId")]
            provider_id: String,
            group: String,
            detail: String,
        },
        Fold {
            count: usize,
            holds: String,
        },
    }
    let group_name = |group: Group| match group {
        Group::Recent => "recent",
        Group::Installed => "installed",
        Group::NotChecked => "not-checked",
        Group::NotFound => "not-found",
    };
    let holds_name = |holds: Holds| match holds {
        Holds::NotFound => "not-found",
        Holds::CantCheck => "cant-check",
        Holds::Both => "both",
    };
    let shown = |item: Item| match item {
        Item::Heading { group, holds } => Row::Heading {
            group: group_name(group).to_owned(),
            holds: holds.map(|holds| holds_name(holds).to_owned()),
        },
        Item::Provider { id, group, meta } => Row::Provider {
            provider_id: id.to_owned(),
            group: group_name(group).to_owned(),
            detail: match meta {
                Meta::Command => "command",
                Meta::NotFound => "not-found",
                Meta::CantCheck => "cant-check",
            }
            .to_owned(),
        },
        Item::Fold { count, holds } => Row::Fold {
            count,
            holds: holds_name(holds).to_owned(),
        },
    };

    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../../../../../schemas/workbench/v1/agent-start-rows-conformance-v1.json"
    ))
    .unwrap();
    assert_eq!(corpus.contract_version, 1);
    assert!(!corpus.cases.is_empty());
    for case in corpus.cases {
        let sources = Sources::new(&case.catalog, &[]);
        let rows: Vec<Row> = items(&sources, &case.query, case.expanded)
            .into_iter()
            .map(shown)
            .collect();
        assert_eq!(rows, case.expected.rows, "{}", case.name);
        let status = status(&sources).map(|status| match status {
            Status::Looking => "looking",
            Status::CouldntCheck => "couldnt-check",
            Status::Unsupported => "unsupported",
        });
        assert_eq!(status, case.expected.status.as_deref(), "{}", case.name);
    }
}

#[test]
fn the_cursor_rests_on_the_most_recent_provider_else_the_first_installed() {
    let cases: [(&[&str], &str); 2] = [(&["gemini"], "gemini"), (&[], "claude-code")];
    for (recent, expected) in cases {
        let catalog = test_catalog(&["codex", "gemini", "claude-code"], recent);
        let rows = items(&Sources::new(&catalog, &[]), "", false);
        let picker = Picker::new(false, Places::new(None, |_| false));
        let index = picker.cursor_index(&rows).unwrap();
        assert!(
            matches!(rows[index], Item::Provider { id, .. } if id == expected),
            "{recent:?}"
        );
    }
}

#[test]
fn a_late_lookup_result_never_moves_the_cursor() {
    let mut board = super::super::AgentBoard::new(true);
    board.open_picker(None, false);
    board.install_start_catalog_t(&["gemini"]);
    let cursor = |board: &super::super::AgentBoard| {
        let rows = board.picker_items();
        let index = board.picker().unwrap().cursor_index(&rows).unwrap();
        match rows[index] {
            Item::Provider { id, .. } => id,
            other => panic!("cursor on {other:?}"),
        }
    };
    assert_eq!(cursor(&board), "gemini");
    // Codex sorts above Gemini once the next catalog finds it.
    board.install_start_catalog_t(&["codex", "gemini"]);
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

/// The new-Tab menu lists what the picker lists first: Recent, then Installed A–Z, at most four,
/// and never a provider that is still being checked, not found or not checked.
#[test]
fn new_tab_menu_lists_the_pickers_found_providers_up_to_four() {
    struct Case {
        name: &'static str,
        recent: &'static [&'static str],
        installed: &'static [&'static str],
        missing: &'static [&'static str],
        pending: &'static [&'static str],
        expect: &'static [&'static str],
    }
    let cases = [
        Case {
            name: "recent leads, then installed by product name",
            recent: &["gemini"],
            installed: &["codex", "gemini", "claude-code"],
            missing: &[],
            pending: &[],
            expect: &["gemini", "claude-code", "codex"],
        },
        Case {
            name: "installed alone, by product name",
            recent: &[],
            installed: &["pi", "amp"],
            missing: &[],
            pending: &[],
            expect: &["amp", "pi"],
        },
        Case {
            name: "more than four keeps the first four in the picker's order",
            recent: &["pi"],
            installed: &["pi", "opencode", "kimi", "codex", "amp", "claude-code"],
            missing: &[],
            pending: &[],
            expect: &["pi", "amp", "claude-code", "codex"],
        },
        Case {
            name: "a provider still being checked waits; found so far is listed",
            recent: &["codex"],
            installed: &["gemini"],
            missing: &[],
            pending: &["codex", "amp"],
            expect: &["gemini"],
        },
        Case {
            name: "a provider not found is never listed",
            recent: &["codex"],
            installed: &["codex", "gemini"],
            missing: &["codex"],
            pending: &[],
            expect: &["gemini"],
        },
        Case {
            name: "nothing found lists nothing",
            recent: &[],
            installed: &[],
            missing: &[],
            pending: &[],
            expect: &[],
        },
    ];
    for case in cases {
        let catalog = checking(test_catalog(case.installed, case.recent), case.pending);
        let listed = menu_agents(&Sources::new(&catalog, case.missing));
        assert_eq!(listed, case.expect, "{}", case.name);
    }
}
