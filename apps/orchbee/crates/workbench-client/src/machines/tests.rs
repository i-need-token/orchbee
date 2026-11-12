use super::*;
fn id(n: u64) -> ProfileId {
    ProfileId::parse(&format!("{n:032x}")).unwrap()
}
fn draft() -> Draft {
    Draft::parse("工作", "user@work-host", "work").unwrap()
}
#[test]
fn destinations_reject_option_shell_password_and_port_confusion() {
    for target in [
        "dev",
        "user@host",
        "domain\\user@host",
        "ssh://user@host:2222",
        "[::1]",
        "ssh://u@[2001:db8::1]:22",
    ] {
        assert!(valid_target(target), "{target}");
    }
    for target in [
        "",
        "-oProxyCommand=x",
        "u:p@host",
        "ssh://u:p@host",
        "host:22",
        "ssh://host:0",
        "ssh://host:65536",
        "ssh://host/path",
        "u@h@x",
        "host;calc",
        "$(calc)",
        "host\n",
        "host x",
        "ssh://[::1]:",
        "[bad]",
        "ssh://host:22:23",
        "ssh://",
        "用户@host",
        "u\u{60}@host",
        "host%22",
        "ssh://-host",
        "ssh://u@host?x",
    ] {
        assert!(!valid_target(target), "{target:?}");
    }
    assert!(Draft::parse(" ", "host", "default").is_none());
    assert!(Draft::parse("a\nb", "host", "default").is_none());
    assert!(Draft::parse(&"中".repeat(43), "host", "default").is_none());
    assert!(Draft::parse("name", "host", "../session").is_none());
}
#[test]
fn catalog_trace_retains_opaque_identity_noop_revision_and_duplicate_endpoints() {
    let empty = Catalog::default();
    let (one, changed) = empty
        .edit(&Edit::Add {
            id: id(1),
            draft: draft(),
        })
        .unwrap();
    assert!(changed);
    assert_eq!(one.revision, 1);
    let (two, _) = one
        .edit(&Edit::Add {
            id: id(2),
            draft: draft(),
        })
        .unwrap();
    assert_eq!(two.profiles.len(), 2);
    let (renamed, _) = two
        .edit(&Edit::Rename {
            id: id(1),
            label: "renamed".into(),
        })
        .unwrap();
    let (disabled, _) = renamed
        .edit(&Edit::Enable {
            id: id(2),
            enabled: false,
        })
        .unwrap();
    let (unchanged, changed) = disabled
        .edit(&Edit::Enable {
            id: id(2),
            enabled: false,
        })
        .unwrap();
    assert!(!changed);
    assert_eq!(unchanged, disabled);
    let (last, _) = disabled.edit(&Edit::Remove(id(1))).unwrap();
    assert_eq!(last.profiles[0].id, id(2));
    assert!(!last.profiles[0].enabled);
    assert_eq!(last.revision, 5);
    assert_eq!(
        last.edit(&Edit::Remove(id(1))).unwrap_err(),
        Error::NotFound
    );
    assert_eq!(
        Catalog::parse(&serde_json::to_vec(&last.json()).unwrap()),
        Some(last)
    );
    assert_eq!(empty.revision, 0);
    assert!(empty.profiles.is_empty());
}
/// A later release's connection kind must survive this build's edits byte-for-byte in meaning,
/// or adding a kind would corrupt older readers' catalogs.
#[test]
fn unknown_connection_kind_is_kept_and_leaves_other_machines_usable() {
    let later = json!({"kind":"pairing-code","code":"x","nested":{"a":[1]}});
    let mut value = Catalog::default()
        .edit(&Edit::Add {
            id: id(1),
            draft: draft(),
        })
        .unwrap()
        .0
        .json();
    value["profiles"].as_array_mut().unwrap().push(json!({
        "id":id(2).as_str(),"label":"later","connection":later,"session":"default",
        "enabled":true,"remoteInstall":"managed","extensions":{"x":1}
    }));
    let catalog = Catalog::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(catalog.profiles[0].draft.connection.supported());
    assert!(!catalog.profiles[1].draft.connection.supported());
    assert_eq!(
        catalog.profiles[1].json()["connection"],
        json!({"kind":"unsupported"})
    );
    let (edited, _) = catalog
        .edit(&Edit::Rename {
            id: id(2),
            label: "renamed".into(),
        })
        .unwrap();
    let written = edited.json();
    assert_eq!(written["profiles"][1]["connection"], later);
    assert_eq!(written["profiles"][1]["extensions"], json!({"x":1}));
    value["profiles"][1]["remoteInstall"] = json!("later");
    assert!(Catalog::parse(&serde_json::to_vec(&value).unwrap()).is_none());
}
#[test]
fn invalid_catalog_and_exhaustion_never_mutate_the_source() {
    let (mut full, _) = Catalog::default()
        .edit(&Edit::Add {
            id: id(1),
            draft: draft(),
        })
        .unwrap();
    full.revision = MAX_REVISION;
    assert!(
        !full
            .edit(&Edit::Enable {
                id: id(1),
                enabled: true
            })
            .unwrap()
            .1
    );
    assert_eq!(
        full.edit(&Edit::Remove(id(1))).unwrap_err(),
        Error::RevisionExhausted
    );
    let mut value = full.json();
    value["profiles"]
        .as_array_mut()
        .unwrap()
        .push(full.profiles[0].json());
    assert!(Catalog::parse(&serde_json::to_vec(&value).unwrap()).is_none());
    value = full.json();
    value["unknown"] = json!(1);
    assert!(Catalog::parse(&serde_json::to_vec(&value).unwrap()).is_none());
    value = full.json();
    value["revision"] = json!(MAX_REVISION + 1);
    assert!(Catalog::parse(&serde_json::to_vec(&value).unwrap()).is_none());
    let full = Catalog {
        revision: 10,
        profiles: (1..=64).map(|n| Profile::new(id(n), draft())).collect(),
        ..Catalog::default()
    };
    assert_eq!(
        full.edit(&Edit::Add {
            id: id(65),
            draft: draft()
        })
        .unwrap_err(),
        Error::Capacity
    );
}
