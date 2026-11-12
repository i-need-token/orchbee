//! `channel set` through the engine: the channel is saved before the update fails, settings text
//! survives the edit, an unreadable file is never rewritten, and no installation or Session runtime
//! appears.
use super::super::{Error, Report, document, engine, signature::TEST_TABLE};
use super::install_fixture::{Fixture, context, in_process_completion, print_probe};
use orchbee_local_runtime::{DataRoot, UpdateChannel, read_user_settings};
use serde_json::Value;
use std::{ffi::OsString, fs, path::Path};

struct Roots {
    data: std::path::PathBuf,
    runtime: std::path::PathBuf,
    install: std::path::PathBuf,
}

impl Roots {
    fn state(&self, name: &str) -> Option<OsString> {
        match name {
            "ORCHBEE_HOME" => Some(self.data.clone().into_os_string()),
            "ORCHBEE_RUNTIME_BASE" => Some(self.runtime.clone().into_os_string()),
            _ => None,
        }
    }

    fn config(&self) -> std::path::PathBuf {
        DataRoot::resolve(|name| self.state(name))
            .unwrap()
            .config_file()
    }

    fn effective_channel(&self) -> UpdateChannel {
        read_user_settings(|name| self.state(name))
            .unwrap()
            .settings
            .update_channel
    }

    /// One engine run whose manifest source is `source`, rendered as the CLI renders it.
    fn update(&self, channel: Option<UpdateChannel>, source: Option<&str>) -> (Report, Value) {
        let state = |name: &str| self.state(name);
        let inputs = engine::Inputs {
            channel,
            state: &state,
            manifest_source: source.map(str::to_owned),
            keys: TEST_TABLE,
            install_root: Ok(self.install.clone()),
            probe: print_probe,
            activation: in_process_completion,
        };
        let mut context = context();
        let mut report = Report::new();
        let error: Option<Error> = engine::run(&inputs, &mut context, &mut report).err();
        let rendered = document(channel, &report, error, false);
        let schema: Value =
            serde_json::from_str(include_str!("../../schemas/orchbee.update.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(validator.is_valid(&rendered), "{rendered}");
        (report, rendered)
    }
}

fn assert_failed_after_save(rendered: &Value, saved: bool, error: &str) {
    assert_eq!(rendered["result"]["channelSaved"], saved, "{rendered}");
    assert_eq!(rendered["result"]["error"], error, "{rendered}");
    assert_eq!(
        rendered["result"]["configurationFailure"].is_object(),
        error == "configuration",
        "{rendered}"
    );
}

#[test]
fn channel_set_persists_before_failed_update_and_never_activates_a_session() {
    let fixture = Fixture::new();
    let roots = Roots {
        data: fixture.0.join("data"),
        runtime: fixture.0.join("runtime"),
        install: fixture.0.join("installation"),
    };
    let config = roots.config();

    for source in [None, Some(""), Some("  ")] {
        let (report, rendered) = roots.update(None, source);
        assert_eq!(rendered["operation"], "update");
        assert_failed_after_save(&rendered, false, "source-unconfigured");
        assert_eq!(report.channel, Some(UpdateChannel::Stable));
        assert!(!config.exists());
    }

    let (report, rendered) = roots.update(Some(UpdateChannel::Preview), None);
    assert_eq!(rendered["operation"], "channel-set");
    assert_eq!(rendered["result"]["channel"], "preview");
    assert_failed_after_save(&rendered, true, "source-unconfigured");
    assert_eq!(report.config_path.as_deref(), Some(config.as_path()));
    assert_eq!(roots.effective_channel(), UpdateChannel::Preview);

    // Warnings never block the save: unknown sections survive and an invalid channel is replaced.
    for (warning, retained) in [
        ("[unknown]\nsecret='kept'", Some("secret='kept'")),
        ("[update]\nchannel=true", None),
    ] {
        fs::write(&config, warning).unwrap();
        let (_, rendered) = roots.update(Some(UpdateChannel::Preview), None);
        assert_eq!(rendered["result"]["channel"], "preview");
        assert_failed_after_save(&rendered, true, "source-unconfigured");
        if let Some(retained) = retained {
            assert!(fs::read_to_string(&config).unwrap().contains(retained));
        }
        assert_eq!(roots.effective_channel(), UpdateChannel::Preview);
    }

    // A file that cannot be saved names its own reason, and the syntax reason its position.
    let invalid = "not TOML";
    fs::write(&config, invalid).unwrap();
    let (_, rendered) = roots.update(Some(UpdateChannel::Preview), None);
    assert_failed_after_save(&rendered, false, "configuration");
    let failure = &rendered["result"]["configurationFailure"];
    assert_eq!(
        (&failure["field"], &failure["reason"], &failure["line"]),
        (&"file".into(), &"invalid-toml".into(), &1.into()),
        "{rendered}"
    );
    assert!(failure["column"].as_u64().is_some(), "{rendered}");
    assert_eq!(fs::read_to_string(&config).unwrap(), invalid);

    let kept = "[update]\nchannel='stable'\n";
    fs::write(&config, kept).unwrap();
    let mut permissions = fs::metadata(&config).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&config, permissions.clone()).unwrap();
    let (_, rendered) = roots.update(Some(UpdateChannel::Preview), None);
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&config, permissions).unwrap();
    assert_failed_after_save(&rendered, false, "configuration");
    assert_eq!(
        rendered["result"]["configurationFailure"],
        serde_json::json!({"field": "file", "reason": "read-only"})
    );
    assert_eq!(fs::read_to_string(&config).unwrap(), kept);

    assert!(!roots.install.exists());
    assert!(!Path::new(&roots.runtime).exists());
}
