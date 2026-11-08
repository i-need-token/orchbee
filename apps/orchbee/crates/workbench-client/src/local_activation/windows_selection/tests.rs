use super::*;

#[test]
fn nested_tui_guard_is_route_specific_and_requires_product_terminal_identity() {
    let current = SessionName::parse("work").unwrap();
    let spelled = SessionName::parse("WORK").unwrap();
    let other = SessionName::parse("other").unwrap();
    for selected in [None, Some(&current), Some(&spelled), Some(&other)] {
        for hosted in [false, true] {
            for _ in 0..2 {
                let guarded = inside_session(
                    selected,
                    |key| match key {
                        "ORCHBEE_RUNTIME_SCOPE" => Some("work".into()),
                        "ORCHBEE_TERMINAL_ID" if hosted => {
                            Some("11111111111111111111111111111111".into())
                        }
                        "ORCHBEE_WORKER_ID" if hosted => {
                            Some("22222222222222222222222222222222".into())
                        }
                        _ => None,
                    },
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
    let default = SessionName::parse("default").unwrap();
    let location = resolve(
        Some(&default),
        |key| {
            assert_eq!(key, "ORCHBEE_RUNTIME_BASE");
            Some(OsString::from("C:/runtime"))
        },
        PathBuf::from("C:/fallback"),
    )
    .unwrap();
    assert_eq!(location.scope(), "default");
    assert_eq!(location.base, PathBuf::from("C:/runtime"));
    assert!(!location.default_base);
}

#[test]
fn inherited_route_precedes_name_and_invalid_selected_values_do_not_fall_back() {
    let inherited = resolve(
        None,
        |key| match key {
            "ORCHBEE_RUNTIME_SCOPE" => Some(OsString::from("owning-session")),
            "ORCHBEE_SESSION" => panic!("inherited route must win"),
            _ => None,
        },
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(inherited.scope(), "owning-session");
    assert!(inherited.default_base);
    for variable in ["ORCHBEE_RUNTIME_SCOPE", "ORCHBEE_SESSION"] {
        for bad in ["", "..", "some/name", "con"] {
            let error = resolve(
                None,
                |key| (key == variable).then(|| bad.into()),
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
        |key| (key == "ORCHBEE_SESSION").then(|| "Work".into()),
        PathBuf::from("C:/base"),
    )
    .unwrap();
    assert_eq!(named.scope(), "Work");
    assert_eq!(named.name, SessionName::parse("work").unwrap());
}
