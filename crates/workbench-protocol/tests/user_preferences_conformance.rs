//! The independent bindings consume the same schema-owned boundary corpus.
use orchbee_workbench_protocol::{
    PreferenceKey, UserLaunchCommand, UserLaunchField, UserLaunchProfile, UserLaunchProfiles,
    UserPreferencesPatch, UserPreferencesResponse, UserPreferencesValues,
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

#[test]
fn launch_field_diagnostics_follow_validation_order_and_keep_literal_argument_identity() {
    let mut values = UserPreferencesValues::default();
    values.launch_profiles.profiles.push(UserLaunchProfile {
        id: "custom".into(),
        name: Some(" leading space".into()),
        command: UserLaunchCommand::CustomExecutable {
            executable: String::new(),
            arguments: vec![String::new(), "bad\0argument".into()],
        },
    });
    for (field, correction) in [
        (UserLaunchField::Name, "Custom"),
        (UserLaunchField::Executable, "literal-program"),
        (UserLaunchField::Argument(1), "a b;$literal"),
    ] {
        assert!(values.validate().is_err());
        assert_eq!(values.launch_profiles.invalid_field(), Some((1, field)));
        assert!(
            serde_json::from_value::<UserLaunchProfiles>(
                serde_json::to_value(&values.launch_profiles).unwrap()
            )
            .is_err()
        );
        let profile = &mut values.launch_profiles.profiles[1];
        match field {
            UserLaunchField::Name => profile.name = Some(correction.into()),
            UserLaunchField::Executable => {
                let UserLaunchCommand::CustomExecutable { executable, .. } = &mut profile.command
                else {
                    panic!("custom command")
                };
                *executable = correction.into();
            }
            UserLaunchField::Argument(index) => {
                let UserLaunchCommand::CustomExecutable { arguments, .. } = &mut profile.command
                else {
                    panic!("custom command")
                };
                arguments[index] = correction.into();
            }
        }
    }
    assert!(values.validate().is_ok());
    assert_eq!(values.launch_profiles.invalid_field(), None);
    assert_eq!(
        serde_json::from_value::<UserLaunchProfiles>(
            serde_json::to_value(&values.launch_profiles).unwrap()
        )
        .unwrap(),
        values.launch_profiles
    );
}

#[test]
fn malformed_catalog_structure_never_projects_a_misleading_profile_field() {
    for defect in 0..9 {
        let mut values = UserPreferencesValues::default();
        // A field is also invalid, so a separate field-only scan would misdirect the user.
        values.launch_profiles.profiles[0].command = UserLaunchCommand::CustomExecutable {
            executable: String::new(),
            arguments: Vec::new(),
        };
        let catalog = &mut values.launch_profiles;
        match defect {
            0 => catalog.version = 2,
            1 => catalog.default_profile_id = "missing-profile".into(),
            2 => catalog.default_profile_id = "invalid id".into(),
            3 => catalog.profiles.push(catalog.profiles[0].clone()),
            4 => catalog.profiles[0].id = "invalid id".into(),
            5 => catalog.profiles.clear(),
            6 => {
                let profile = catalog.profiles[0].clone();
                catalog.profiles = (0..65)
                    .map(|index| UserLaunchProfile {
                        id: format!("profile-{index}"),
                        name: Some("Profile".into()),
                        ..profile.clone()
                    })
                    .collect();
                catalog.default_profile_id = "profile-0".into();
            }
            7 => {
                catalog.profiles[0].command = UserLaunchCommand::CustomExecutable {
                    executable: String::new(),
                    arguments: vec![String::new(); 129],
                }
            }
            8 => catalog.profiles[0].name = Some("x".repeat(1024 * 1024)),
            _ => unreachable!(),
        }
        assert_eq!(catalog.invalid_field(), None, "structural defect {defect}");
        assert!(values.validate().is_err(), "structural defect {defect}");
        assert!(
            serde_json::from_value::<UserLaunchProfiles>(
                serde_json::to_value(&values.launch_profiles).unwrap()
            )
            .is_err()
        );
    }
}
