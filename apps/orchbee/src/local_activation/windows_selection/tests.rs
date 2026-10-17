use super::*;

#[test]
fn nested_tui_guard_is_route_specific_and_requires_product_terminal_identity() {
    let current = WindowsSiteName::parse("work").unwrap();
    let other = WindowsSiteName::parse("other").unwrap();
    let scope = current.scope_for_user("sid");
    for selected in [None, Some(&current), Some(&other)] {
        for hosted in [false, true] {
            for _ in 0..2 {
                let guarded = inside_session(
                    selected,
                    |key| match key {
                        "ORCHBEE_RUNTIME_SCOPE" => Some(scope.clone().into()),
                        "ORCHBEE_TERMINAL_ID" if hosted => {
                            Some("11111111111111111111111111111111".into())
                        }
                        "ORCHBEE_WORKER_ID" if hosted => {
                            Some("22222222222222222222222222222222".into())
                        }
                        _ => None,
                    },
                    "sid",
                    PathBuf::from("C:/runtime"),
                )
                .unwrap();
                assert_eq!(guarded, hosted && selected != Some(&other));
            }
        }
    }
}

#[test]
fn explicit_default_overrides_inherited_scope_and_name_without_reading_them() {
    let default = WindowsSiteName::parse("default").unwrap();
    let location = resolve(
        Some(&default),
        |key| {
            assert_eq!(key, "ORCHBEE_RUNTIME_BASE");
            Some(OsString::from("C:/runtime"))
        },
        "sid",
        PathBuf::from("C:/fallback"),
    )
    .unwrap();
    assert_eq!(location.scope, default.scope_for_user("sid"));
    assert_eq!(location.base, PathBuf::from("C:/runtime"));
}

#[test]
fn inherited_route_precedes_name_and_invalid_selected_values_do_not_fall_back() {
    let inherited = resolve(
        None,
        |key| match key {
            "ORCHBEE_RUNTIME_SCOPE" => Some(OsString::from("owning-site")),
            "ORCHBEE_SESSION" => panic!("inherited route must win"),
            _ => None,
        },
        "sid",
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(inherited.scope, "owning-site");
    assert!(inherited.name.is_none());
    for variable in ["ORCHBEE_RUNTIME_SCOPE", "ORCHBEE_SESSION", "ORCHBEE_SITE"] {
        for bad in ["", "..", "some/name"] {
            let error = resolve(
                None,
                |key| (key == variable).then(|| bad.into()),
                "sid",
                PathBuf::from("C:/base"),
            )
            .err()
            .unwrap();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains(variable));
        }
    }
    let named = resolve(
        None,
        |key| (key == "ORCHBEE_SESSION").then(|| "work".into()),
        "sid",
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(
        named.scope,
        WindowsSiteName::parse("work")
            .unwrap()
            .scope_for_user("sid")
    );
}

#[test]
fn legacy_environment_preserves_the_existing_route_and_canonical_selection_wins() {
    let expected = WindowsSiteName::parse("work")
        .unwrap()
        .scope_for_user("sid");
    let legacy = resolve(
        None,
        |key| (key == "ORCHBEE_SITE").then(|| "work".into()),
        "sid",
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(legacy.scope, expected);
    assert_eq!(legacy.name.unwrap().as_str(), "work");

    let canonical = resolve(
        None,
        |key| match key {
            "ORCHBEE_SESSION" => Some("work".into()),
            "ORCHBEE_SITE" => panic!("canonical selection must win without reading legacy input"),
            _ => None,
        },
        "sid",
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(canonical.scope, expected);

    let invalid_canonical = resolve(
        None,
        |key| match key {
            "ORCHBEE_SESSION" => Some("..".into()),
            "ORCHBEE_SITE" => Some("work".into()),
            _ => None,
        },
        "sid",
        PathBuf::from("C:/base"),
    );
    assert_eq!(
        invalid_canonical.err().unwrap().kind(),
        io::ErrorKind::InvalidInput
    );
}
