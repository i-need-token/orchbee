use super::*;

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
            "ORCHBEE_SITE" => panic!("inherited route must win"),
            _ => None,
        },
        "sid",
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(inherited.scope, "owning-site");
    assert!(inherited.name.is_none());
    for variable in ["ORCHBEE_RUNTIME_SCOPE", "ORCHBEE_SITE"] {
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
        |key| (key == "ORCHBEE_SITE").then(|| "work".into()),
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
