// A bundle leaves the machine, so each class of private text a settings file or preference record
// can hold gets a refusal here; the archive test proves a standard reader can open the result.
use super::archive::crc32;
use super::{DiagnosticRedactor, diagnostic_archive, redact_preferences, redact_settings};

fn redactor() -> DiagnosticRedactor {
    DiagnosticRedactor::new(
        Some(r"C:\Users\Person"),
        [
            ("ORCHBEE_RUNTIME_BASE", r"D:\runtime-base\"),
            ("TEMP", r"C:\Users\Person\AppData\Local\Temp"),
            ("LOCALAPPDATA", "x"),
        ],
    )
}

const SETTINGS: &str = r#"
# My password is hunter2 and the build server is at C:\Users\Person\ci
api_key = "sk-live-unrecognized"

[terminal]
new_cwd = 'c:/users/person/work'

[update]
manifest_check = "https://deploy:pa55@updates.example.com/manifest.json?token=abc123#latest"

[worktrees]
directory = 'D:\runtime-base\trees'

[[keys.command]]
type = "shell"
key = "g"
command = "git push --force https://user:tok@example.com/repo"
description = "Push from C:\\Users\\Person\\src"

[secrets]
token = "ghp_unrecognized_table"
"#;

#[test]
fn settings_keep_their_keys_without_any_private_value() {
    let redacted = redact_settings(SETTINGS, &redactor()).expect("valid TOML");
    for (class, private) in [
        ("file contents in a comment", "hunter2"),
        ("comment path", "ci"),
        ("unrecognized key", "sk-live-unrecognized"),
        ("unrecognized table", "ghp_unrecognized_table"),
        ("command", "git push"),
        ("URL credential", "pa55"),
        ("URL query", "abc123"),
        ("home path", "users/person"),
        ("home path", r"Users\\Person"),
        ("environment value", "runtime-base"),
    ] {
        assert!(
            !redacted
                .to_ascii_lowercase()
                .contains(&private.to_ascii_lowercase()),
            "{class} reached the bundle:\n{redacted}"
        );
    }
    let parsed: toml::Table = redacted.parse().expect("the bundle's settings stay TOML");
    assert_eq!(parsed["api_key"].as_str(), Some("<unrecognized>"));
    assert_eq!(parsed["terminal"]["new_cwd"].as_str(), Some("<home>/work"));
    assert_eq!(
        parsed["update"]["manifest_check"].as_str(),
        Some("https://<credential>@updates.example.com/manifest.json?<query>#latest")
    );
    assert_eq!(
        parsed["worktrees"]["directory"].as_str(),
        Some(r"<env:ORCHBEE_RUNTIME_BASE>\trees")
    );
    let command = &parsed["keys"]["command"][0];
    assert_eq!(command["command"].as_str(), Some("<command>"));
    assert_eq!(command["key"].as_str(), Some("g"));
    assert_eq!(
        command["description"].as_str(),
        Some(r"Push from <home>\src")
    );
    assert!(
        parsed["secrets"]["token"]
            .as_str()
            .is_some_and(|value| value.starts_with('<') && value.ends_with('>'))
    );
    assert_eq!(redact_settings("not = [valid", &redactor()), None);
}

#[test]
fn preferences_replace_launch_commands_arguments_and_environment() {
    let record = serde_json::json!({
        "version": 1,
        "values": {
            "launchProfiles": {
                "version": 1,
                "defaultProfileId": "tool",
                "profiles": [{
                    "id": "tool",
                    "name": "Tool in C:\\Users\\Person",
                    "command": {
                        "kind": "custom-executable",
                        "executable": "C:\\Users\\Person\\bin\\tool.exe",
                        "arguments": ["--api-token", "s3cret-value"],
                        "environment": {"OPENAI_API_KEY": "sk-proj-value"}
                    }
                }]
            },
            "scrollbackBytes": 10_000_000
        },
        "claimed": ["launchProfiles"]
    });
    let bytes = serde_json::to_vec(&record).expect("encode");
    let redacted = redact_preferences(&bytes, &redactor()).expect("valid JSON");
    let text = redacted.to_string();
    for (class, private) in [
        ("command", "tool.exe"),
        ("argument", "--api-token"),
        ("argument", "s3cret-value"),
        ("environment value", "sk-proj-value"),
        ("home path", "Person"),
    ] {
        assert!(
            !text.contains(private),
            "{class} reached the bundle: {text}"
        );
    }
    let command = &redacted["values"]["launchProfiles"]["profiles"][0]["command"];
    assert_eq!(command["kind"], "custom-executable");
    assert_eq!(command["executable"], "<command>");
    assert_eq!(
        command["arguments"],
        serde_json::json!(["<argument>", "<argument>"])
    );
    assert_eq!(command["environment"]["OPENAI_API_KEY"], "<environment>");
    assert_eq!(redacted["values"]["scrollbackBytes"], 10_000_000);
    assert_eq!(redact_preferences(b"{", &redactor()), None);
}

#[test]
fn every_carried_document_loses_home_folder_and_url_secrets() {
    let mut document = serde_json::json!({
        "runtimeDirectory": {"path": "\\\\?\\C:\\USERS\\PERSON\\AppData\\Local\\Temp\\orchbee"},
        "C:\\Users\\Person\\key": ["https://a:b@host/x?q=1", "plain"],
    });
    redactor().scrub_json(&mut document);
    assert_eq!(
        document,
        serde_json::json!({
            "runtimeDirectory": {"path": "\\\\?\\<env:TEMP>\\orchbee"},
            "<home>\\key": ["https://<credential>@host/x?<query>", "plain"],
        })
    );
}

#[test]
fn archives_open_as_standard_stored_zip_files() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    let entries = vec![
        ("manifest.json".to_owned(), b"{}".to_vec()),
        ("logs/2026-11-08.jsonl".to_owned(), b"line\n".to_vec()),
    ];
    let archive = diagnostic_archive(&entries, 1_762_570_245).expect("archive");
    let u16_at = |offset: usize| u16::from_le_bytes([archive[offset], archive[offset + 1]]);
    let u32_at = |offset: usize| {
        u32::from_le_bytes([
            archive[offset],
            archive[offset + 1],
            archive[offset + 2],
            archive[offset + 3],
        ])
    };
    let end = archive.len() - 22;
    assert_eq!(u32_at(end), 0x0605_4b50);
    assert_eq!(u16_at(end + 10), 2);
    let mut central = u32_at(end + 16) as usize;
    for (name, data) in &entries {
        assert_eq!(u32_at(central), 0x0201_4b50);
        assert_eq!(u16_at(central + 10), 0, "stored");
        assert_eq!(u32_at(central + 16), crc32(data));
        let name_length = u16_at(central + 28) as usize;
        assert_eq!(
            &archive[central + 46..central + 46 + name_length],
            name.as_bytes()
        );
        let local = u32_at(central + 42) as usize;
        assert_eq!(u32_at(local), 0x0403_4b50);
        let start = local + 30 + u16_at(local + 26) as usize;
        assert_eq!(&archive[start..start + data.len()], data.as_slice());
        central += 46 + name_length;
    }
}
