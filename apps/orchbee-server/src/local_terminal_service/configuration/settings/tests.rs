//! Actual file/publication decision traces use the existing isolated configuration fixture.
use super::*;
use crate::local_terminal_service::configuration::tests::{Fixture, current};
use orchbee_workbench_protocol::{NativeSettingsChanges, SiteId, WorkbenchRequestId};

fn request(operation: Operation) -> NativeSettingsRequest {
    NativeSettingsRequest {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("00000000000000000000000000000001").unwrap(),
        site_id: SiteId::parse("00000000000000000000000000000002").unwrap(),
        operation,
    }
}
fn open(registry: &Registry) -> (NativeSettingsSource, ConfigurationSnapshot) {
    let Outcome::Opened { source, snapshot } = registry
        .execute_settings(&request(Operation::Open {}), |_| {
            panic!("opening cannot apply")
        })
    else {
        panic!("opened settings");
    };
    (source, snapshot)
}
fn save(source: NativeSettingsSource, snapshot: &ConfigurationSnapshot) -> NativeSettingsRequest {
    request(Operation::Save {
        source,
        expected_revision: snapshot.revision,
        changes: NativeSettingsChanges {
            theme_name: Some(ConfigurationThemeName::OneDark),
            ..Default::default()
        },
    })
}

#[test]
fn saved_settings_publish_once_and_second_window_refuses_its_stale_revision() {
    let fixture = Fixture::new();
    fixture.write("# preserve\n[theme]\nname='one-light'\n[ui.toast]\ndelay_seconds=19\n");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    let (source, before) = open(&registry);
    let operation = save(source, &before);
    let mut applied = 0;
    let Outcome::SavedApplied { snapshot } = registry.execute_settings(&operation, |settings| {
        applied += 1;
        assert_eq!(settings.theme, WindowsThemeName::OneDark);
        true
    }) else {
        panic!("saved and applied");
    };
    assert_eq!(applied, 1);
    assert_eq!(snapshot.revision.value(), before.revision.value() + 1);
    assert_eq!(current(&registry), snapshot);
    assert!(
        std::fs::read_to_string(fixture.file())
            .unwrap()
            .contains("# preserve")
    );
    assert_eq!(
        registry.execute_settings(&operation, |_| panic!("stale request cannot apply")),
        refused(Failure::ConfigurationChanged)
    );
    let (source, before) = open(&registry);
    assert_eq!(
        registry.execute_settings(&save(source, &before), |_| panic!("no change cannot apply")),
        Outcome::NoChange { snapshot: before }
    );
}

#[test]
fn external_comment_edit_refuses_save_and_unpublished_value_edit_refuses_open() {
    let fixture = Fixture::new();
    fixture.write("[theme]\nname='one-light'\n");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    let (source, before) = open(&registry);
    fixture.write("# external\n[theme]\nname='one-light'\n");
    let bytes = std::fs::read(fixture.file()).unwrap();
    assert_eq!(
        registry.execute_settings(&save(source, &before), |_| panic!("conflict cannot apply")),
        refused(Failure::SourceChanged)
    );
    assert_eq!(std::fs::read(fixture.file()).unwrap(), bytes);
    assert_eq!(current(&registry), before);
    fixture.write("[theme]\nname='terminal-native'\n");
    assert_eq!(
        registry.execute_settings(&request(Operation::Open {}), |_| panic!(
            "open cannot apply"
        )),
        refused(Failure::ConfigurationChanged)
    );
}

#[test]
fn live_failure_reports_the_saved_file_and_preserves_the_complete_old_publication() {
    let fixture = Fixture::new();
    fixture.write("[theme]\nname='one-light'\n");
    let registry = Registry::default();
    registry.initialize(&fixture.settings(true)).unwrap();
    let (source, before) = open(&registry);
    assert_eq!(
        registry.execute_settings(&save(source, &before), |_| false),
        Outcome::SavedNotApplied {
            snapshot: before.clone()
        }
    );
    assert_eq!(fixture.settings(true).theme, WindowsThemeName::OneDark);
    assert_eq!(current(&registry), before);
    assert_eq!(
        registry.execute_settings(&request(Operation::Open {}), |_| panic!(
            "open cannot apply"
        )),
        refused(Failure::ConfigurationChanged)
    );
    assert!(matches!(
        registry.execute(
            &crate::local_terminal_service::configuration::tests::request(
                super::super::Action::Reload
            ),
            |_| true
        ),
        super::super::Outcome::Reloaded { .. }
    ));
    assert_eq!(
        current(&registry).theme_name,
        ConfigurationThemeName::OneDark
    );
}
