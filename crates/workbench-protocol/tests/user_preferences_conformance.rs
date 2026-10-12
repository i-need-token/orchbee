//! The independent bindings consume the same schema-owned boundary corpus.
use orchbee_workbench_protocol::{
    PreferenceKey, UserPreferencesPatch, UserPreferencesResponse, UserPreferencesValues,
    parse_user_preferences_request,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    contract_version: u8,
    defaults: Value,
    valid: Vec<Case>,
    invalid: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    direction: String,
    document: Value,
    #[serde(default, rename = "boundary")]
    _boundary: Option<String>,
}
fn admitted(case: &Case) -> bool {
    match case.direction.as_str() {
        "request" => {
            parse_user_preferences_request(&serde_json::to_vec(&case.document).unwrap()).is_ok()
        }
        "response" => {
            serde_json::from_value::<UserPreferencesResponse>(case.document.clone()).is_ok()
        }
        "values" => serde_json::from_value::<UserPreferencesValues>(case.document.clone()).is_ok(),
        other => panic!("unknown conformance direction: {other}"),
    }
}
#[test]
fn common_preferences_obey_shared_defaults_and_every_boundary_case() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../schemas/workbench/v1/user-preferences-conformance-v1.json"
    ))
    .unwrap();
    assert_eq!(corpus.contract_version, 1);
    assert_eq!(
        serde_json::to_value(UserPreferencesValues::default()).unwrap(),
        corpus.defaults
    );
    for case in corpus.valid {
        assert!(admitted(&case), "valid case: {}", case.name);
    }
    for case in corpus.invalid {
        assert!(!admitted(&case), "invalid case: {}", case.name);
    }
}

#[test]
fn every_reset_uses_the_same_complete_defaults_and_preserves_unselected_values() {
    let changed = UserPreferencesValues {
        sound_enabled: true,
        copy_on_select: false,
        ..UserPreferencesValues::default()
    };
    let only_sound = UserPreferencesPatch::reset([PreferenceKey::SoundEnabled])
        .apply(&changed)
        .unwrap();
    assert!(!only_sound.sound_enabled);
    assert!(!only_sound.copy_on_select);
    assert_eq!(
        UserPreferencesPatch::reset(PreferenceKey::ALL)
            .apply(&changed)
            .unwrap(),
        UserPreferencesValues::default()
    );
}
