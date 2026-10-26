use super::*;
use orchbee_local_runtime::{PluginManifest, PluginPlatform, PluginRegistryError};
fn entry(id: &str) -> PluginRegistryEntry {
    PluginRegistryEntry {
        manifest_path: format!("C:/plugins/{id}/orchbee-plugin.toml"),
        plugin_root: format!("C:/plugins/{id}"),
        enabled: true,
        warning: None,
        source: None,
        manifest: PluginManifest::parse(&format!(
            r#"id="{id}"
name="tools"
version="1"
[[actions]]
id="open"
title="Open"
command=["cmd.exe","/c","exit 0"]
[[link_handlers]]
id="first"
title="First"
pattern="https://"
action="open"
[[link_handlers]]
id="second"
title="Second"
pattern="example"
action="open"
"#
        ))
        .unwrap(),
    }
}
#[test]
fn link_handlers_follow_plugin_lexical_order_and_preserve_overlapping_manifest_order() {
    let a = entry("a");
    let z = entry("z");
    let chosen = select_entries(&[z, a.clone()], "https://example", || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(
        (chosen.entry.manifest.id.as_str(), chosen.handler.as_str()),
        ("a", "first")
    );
    let mut reversed = a;
    reversed.manifest.link_handlers.reverse();
    assert_eq!(
        select_entries(&[reversed], "https://example", || Ok(()))
            .unwrap()
            .unwrap()
            .handler,
        "second"
    );
    assert!(
        select_entries(&[entry("a")], "custom://nothing", || Ok(()))
            .unwrap()
            .is_none()
    );
}
#[test]
fn link_matching_skips_disabled_unavailable_invalid_and_platform_ineligible_handlers() {
    for mode in [
        "disabled",
        "warning",
        "handler-platform",
        "action-platform",
        "inherited",
        "invalid-regex",
    ] {
        let mut a = entry("a");
        match mode {
            "disabled" => a.enabled = false,
            "warning" => a.warning = Some(PluginRegistryError::InvalidManifest),
            "handler-platform" => {
                for handler in &mut a.manifest.link_handlers {
                    handler.platforms = Some(vec![PluginPlatform::Macos]);
                }
            }
            "action-platform" => {
                a.manifest.actions[0].platforms = Some(vec![PluginPlatform::Linux])
            }
            "inherited" => a.manifest.platforms = Some(vec![PluginPlatform::Macos]),
            _ => {
                for handler in &mut a.manifest.link_handlers {
                    handler.pattern = "[".into();
                }
            }
        }
        let chosen = select_entries(&[a, entry("z")], "https://example", || Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(chosen.entry.manifest.id, "z", "{mode}");
    }
    let mut overridden = entry("a");
    overridden.manifest.platforms = Some(vec![PluginPlatform::Macos]);
    overridden.manifest.actions[0].platforms = Some(vec![PluginPlatform::Windows]);
    overridden.manifest.link_handlers[0].platforms = Some(vec![PluginPlatform::Windows]);
    assert!(
        select_entries(&[overridden], "https://example", || Ok(()))
            .unwrap()
            .is_some()
    );
}
#[test]
fn link_matching_obeys_cancellation_before_compilation_or_selection() {
    let result = select_entries(&[entry("a")], "https://example", || {
        Err(Failure::new(FailureKind::Cancelled))
    });
    assert!(matches!(
        result,
        Err(Failure {
            kind: FailureKind::Cancelled,
            ..
        })
    ));
}
