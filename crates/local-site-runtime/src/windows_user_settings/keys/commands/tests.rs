use super::*;

#[test]
fn all_configured_actions_round_trip_without_losing_literal_commands_or_bindings() {
    let profile = r#"[keys]
prefix="ctrl+a"
[[keys.command]]
command="echo space & 中文"
key=["prefix+x","alt+f9"]
description="后台命令"
[[keys.command]]
command="echo pane"
type="pane"
[[keys.command]]
command="echo popup"
type="popup"
width="75%"
height=12
key="prefix+p"
[[keys.command]]
command="tools.inspect"
type="plugin_action"
"#;
    let keys = WindowsKeySettings::from_profile(profile).unwrap();
    assert_eq!(keys.commands.len(), 4);
    assert_eq!(keys.commands[0].command, "echo space & 中文");
    assert_eq!(keys.commands[0].action, WindowsCustomCommandAction::Shell);
    assert_eq!(keys.commands[0].triggers.len(), 2);
    assert_eq!(
        keys.commands[2].width,
        Some(WindowsCommandSize::Percent(75))
    );
    assert_eq!(keys.commands[2].height, Some(WindowsCommandSize::Cells(12)));
    assert_eq!(
        keys.commands[3].action,
        WindowsCustomCommandAction::PluginAction
    );
    assert_eq!(
        WindowsKeySettings::from_profile(&keys.profile()).unwrap(),
        keys
    );
}

#[test]
fn catalog_identity_is_stable_for_the_normalized_profile_and_fences_changed_definitions() {
    let source = "[keys]\n[[keys.command]]\ncommand='echo original'\nkey='prefix+x'\n";
    let keys = WindowsKeySettings::from_profile(source).unwrap();
    let catalog = keys.command_catalog();
    assert_eq!(catalog.len(), 1);
    assert!(catalog[0].id.starts_with("cmd_"));
    assert_eq!(
        WindowsKeySettings::from_profile(&keys.profile())
            .unwrap()
            .command_catalog(),
        catalog
    );
    for change in [
        source.replace("original", "changed"),
        source.replace("prefix+x", "prefix+y"),
        source.replace("[keys]", "[keys]\nprefix='ctrl+a'"),
    ] {
        assert_ne!(
            WindowsKeySettings::from_profile(&change)
                .unwrap()
                .command_catalog()[0]
                .id,
            catalog[0].id
        );
    }
    assert!(WindowsKeySettings::default().command_catalog().is_empty());
}

#[test]
fn command_fields_types_chords_and_action_dimensions_fail_closed() {
    for body in [
        "command=' '",
        "command=4",
        "type='shell'",
        "command='echo x'\ntype='unknown'",
        "command='echo x'\nprogram='extra.exe'",
        "command='echo x'\nwidth=20",
        "command='echo x'\ntype='popup'\nwidth=0",
        "command='echo x'\ntype='popup'\nheight='101%'",
        "command='echo x'\ntype='popup'\nwidth=4097",
        "command='echo x'\nkey='x'",
        "command='echo x'\nkey='ctrl+c'",
        "command='echo x'\nkey='ctrl+b'",
        "command='echo x'\nkey='prefix+esc'",
        "command='echo x'\nkey=['prefix+x','prefix+x']",
        "command='echo x'\ndescription=4",
    ] {
        assert!(
            WindowsKeySettings::from_profile(&format!("[keys]\n[[keys.command]]\n{body}\n"))
                .is_err(),
            "{body}"
        );
    }
    for profile in [
        "[keys]\nhelp='prefix+x'\n[[keys.command]]\ncommand='echo x'\nkey='prefix+x'",
        "[keys]\n[[keys.command]]\ncommand='echo x'\nkey='prefix+x'\n[[keys.command]]\ncommand='echo y'\nkey='prefix+x'",
    ] {
        assert!(WindowsKeySettings::from_profile(profile).is_err());
    }
}

#[test]
fn configured_command_bounds_apply_before_runtime_publication() {
    let entry = "[[keys.command]]\ncommand='echo x'\n";
    assert_eq!(
        WindowsKeySettings::from_profile(&format!("[keys]\n{}", entry.repeat(64)))
            .unwrap()
            .commands
            .len(),
        64
    );
    assert!(WindowsKeySettings::from_profile(&format!("[keys]\n{}", entry.repeat(65))).is_err());
    for (field, length) in [("command", 4097), ("description", 513)] {
        let value = if field == "command" {
            format!("command='{}'", "x".repeat(length))
        } else {
            format!("command='echo x'\ndescription='{}'", "x".repeat(length))
        };
        assert!(
            WindowsKeySettings::from_profile(&format!("[keys]\n[[keys.command]]\n{value}"))
                .is_err()
        );
    }
    let keys = "'alt+f1',".repeat(17);
    assert!(
        WindowsKeySettings::from_profile(&format!(
            "[keys]\n[[keys.command]]\ncommand='echo x'\nkey=[{keys}]"
        ))
        .is_err()
    );
    let mut table: toml::Table = toml::from_str("command='echo x'").unwrap();
    table.insert(
        "command".into(),
        toml::Value::String("echo\0injected".into()),
    );
    let value = toml::Value::Array(vec![toml::Value::Table(table)]);
    assert!(parse(Some(&value), &WindowsKeySettings::default()).is_err());
}
