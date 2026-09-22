use super::*;
#[test]
fn literal_values_are_preserved_but_never_debugged() {
    let pairs = vec![
        ("API_KEY".into(), "secret-中文=$()".into()),
        ("EMPTY".into(), "".into()),
    ];
    let env = LaunchEnvironment::new(pairs.clone()).unwrap();
    assert_eq!(env.entries(), pairs);
    assert!(!format!("{env:?}").contains("secret"));
}
#[test]
fn hostile_names_duplicates_and_bounds_are_rejected() {
    for name in [
        "",
        "1A",
        "A=B",
        "A-B",
        "A B",
        "中文",
        "orchbee_runtime_base",
        "Term",
        "colorTERM",
        "term_program",
    ] {
        assert!(
            LaunchEnvironment::new(vec![(name.into(), "x".into())]).is_err(),
            "{name}"
        );
    }
    assert!(
        LaunchEnvironment::new(vec![
            ("Path".into(), "a".into()),
            ("PATH".into(), "b".into())
        ])
        .is_err()
    );
    assert!(LaunchEnvironment::new(vec![("A".into(), "a\0b".into())]).is_err());
    assert!(LaunchEnvironment::new(vec![("A".repeat(129), "".into())]).is_err());
    assert!(LaunchEnvironment::new(vec![("A".into(), "x".repeat(1023))]).is_ok());
    assert!(LaunchEnvironment::new(vec![("A".into(), "x".repeat(1024))]).is_err());
    let maximum: Vec<_> = (0..32).map(|n| (format!("K{n}"), String::new())).collect();
    assert!(LaunchEnvironment::new(maximum.clone()).is_ok());
    let mut over = maximum;
    over.push(("EXTRA".into(), String::new()));
    assert!(LaunchEnvironment::new(over).is_err());
}
#[test]
fn deserialization_cannot_bypass_environment_validation() {
    for text in [
        r#"[["PATH","one"],["Path","two"]]"#,
        r#"[["TERM","fake"]]"#,
        r#"[["A","x","extra"]]"#,
    ] {
        assert!(serde_json::from_str::<LaunchEnvironment>(text).is_err());
    }
    let env = LaunchEnvironment::new(vec![("A".into(), "literal=值".into())]).unwrap();
    assert_eq!(
        serde_json::from_str::<LaunchEnvironment>(&serde_json::to_string(&env).unwrap()).unwrap(),
        env
    );
}
