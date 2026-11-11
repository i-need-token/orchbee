use orchbee_workbench_protocol::*;
#[test]
fn agent_catalog_shared_corpus_and_carrier_kinds() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/workbench-agent-catalog-conformance-v1.json"
    ))
    .unwrap();
    for (key, expected) in [("valid", true), ("invalid", false)] {
        for case in corpus[key].as_array().unwrap() {
            let bytes = serde_json::to_vec(&case["document"]).unwrap();
            let actual = if case["direction"] == "request" {
                parse_workbench_request(&bytes).is_ok()
            } else {
                parse_workbench_response(&bytes).is_ok()
            };
            assert_eq!(actual, expected, "{}", case["name"]);
        }
    }
    let document = |name: &str| {
        let case = corpus["valid"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap();
        serde_json::to_vec(&case["document"]).unwrap()
    };
    let request = parse_workbench_request(&document("refresh")).unwrap();
    let response = parse_workbench_response(&document("complete-catalog")).unwrap();
    let mut bytes = Vec::new();
    write_workbench_request(&mut bytes, &request).unwrap();
    assert_eq!(&bytes[6..8], &75_u16.to_be_bytes());
    assert_eq!(
        read_workbench_request(&mut bytes.as_slice()).unwrap(),
        request
    );
    bytes.clear();
    write_workbench_response(&mut bytes, &response).unwrap();
    assert_eq!(&bytes[6..8], &76_u16.to_be_bytes());
    assert_eq!(
        read_workbench_response(&mut bytes.as_slice()).unwrap(),
        response
    );
}

/// The Agent start rows parity corpus is well formed: closed shapes and vocabularies, case
/// catalogs holding a subset of rows in catalog order, and expected rows that name the case's own
/// providers under their group's heading. The row models that must produce the rows read it too.
#[test]
fn agent_start_rows_parity_corpus_is_well_formed() {
    use orchbee_agent_observation_protocol::STARTABLE_AGENT_PROVIDER_IDS;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Corpus {
        contract_version: u32,
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
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
        #[serde(deserialize_with = "Option::deserialize")]
        status: Option<Status>,
        rows: Vec<Row>,
    }
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Status {
        Looking,
        CouldntCheck,
    }
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
    #[serde(rename_all = "kebab-case")]
    enum Group {
        Recent,
        Installed,
        NotChecked,
        NotFound,
    }
    #[derive(Clone, Copy, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Detail {
        Command,
        NotFound,
        CantCheck,
    }
    #[derive(Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "kebab-case",
        rename_all_fields = "camelCase",
        deny_unknown_fields
    )]
    enum Row {
        Heading {
            group: Group,
        },
        Provider {
            provider_id: AgentStartSelector,
            group: Group,
            detail: Detail,
        },
        Fold {
            count: std::num::NonZeroUsize,
        },
    }

    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/agent-start-rows-conformance-v1.json"
    ))
    .unwrap();
    assert_eq!(corpus.contract_version, 1);
    assert!(!corpus.cases.is_empty());
    let position = |provider: AgentStartSelector| {
        STARTABLE_AGENT_PROVIDER_IDS
            .iter()
            .position(|id| *id == provider.agent().id())
    };
    for (index, case) in corpus.cases.iter().enumerate() {
        let context = format!(
            "{} (query {:?}, expanded {}, status {:?})",
            case.name, case.query, case.expanded, case.expected.status
        );
        assert!(
            !case.name.is_empty() && corpus.cases[..index].iter().all(|c| c.name != case.name),
            "{context}"
        );
        let catalog = &case.catalog;
        let providers: Vec<_> = catalog.agents.iter().map(|row| row.provider_id).collect();
        assert!(catalog.revision.value() > 0, "{context}");
        assert!(
            providers
                .windows(2)
                .all(|pair| position(pair[0]) < position(pair[1])),
            "{context}: rows out of catalog order"
        );
        assert!(
            catalog.recent.len() <= MAXIMUM_RECENT_AGENTS
                && catalog.recent.iter().enumerate().all(|(at, provider)| {
                    providers.contains(provider) && !catalog.recent[..at].contains(provider)
                }),
            "{context}: recent"
        );
        let mut headings: Vec<Group> = Vec::new();
        for (at, row) in case.expected.rows.iter().enumerate() {
            match row {
                Row::Heading { group } => {
                    assert!(!headings.contains(group), "{context}: heading {group:?}");
                    headings.push(*group);
                }
                Row::Provider {
                    provider_id,
                    group,
                    detail,
                } => {
                    let detail_fits = match detail {
                        Detail::Command => *group != Group::NotFound,
                        Detail::NotFound | Detail::CantCheck => *group == Group::NotFound,
                    };
                    assert!(
                        headings.last() == Some(group)
                            && providers.contains(provider_id)
                            && detail_fits,
                        "{context}: row {at}"
                    );
                }
                Row::Fold { count } => {
                    assert!(
                        at + 1 == case.expected.rows.len() && count.get() <= providers.len(),
                        "{context}: fold"
                    );
                }
            }
        }
    }
}
