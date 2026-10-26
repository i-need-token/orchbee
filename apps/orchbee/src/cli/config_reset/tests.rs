use super::*;
use orchbee_local_runtime::SettingsError;

#[test]
fn reset_receipts_keep_backup_and_recovery_evidence_in_both_locales() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.config.reset-keys.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for case in [
            "missing", "no-keys", "reset", "failed", "recovery", "cleanup",
        ] {
            let outcome = match case {
                "missing" => KeysResetOutcome::Missing,
                "no-keys" => KeysResetOutcome::NoKeys,
                "reset" => KeysResetOutcome::Reset {
                    backup_path: "C:/settings/config.toml.bak-keys-1".into(),
                },
                _ => KeysResetOutcome::Failed {
                    error: SettingsError::new("file", "write-failed"),
                    backup_path: (case != "failed")
                        .then(|| "C:/settings/config.toml.bak-keys-1".into()),
                    recovery_required: case == "recovery",
                },
            };
            let answer = emit(
                KeysResetReport {
                    path: Some("C:/settings/config.toml".into()),
                    outcome,
                },
                OutputFormat::Json,
                locale,
            );
            let document: serde_json::Value = serde_json::from_slice(&answer.stdout).unwrap();
            assert!(validator.is_valid(&document), "{document}");
            assert_eq!(
                answer.status,
                u8::from(matches!(case, "failed" | "recovery" | "cleanup"))
            );
            assert_eq!(document["result"]["recoveryRequired"], case == "recovery");
            if case == "recovery" {
                assert!(answer.stderr.contains("config.toml.bak-keys-1"));
            }
            let mut invalid = document.clone();
            invalid["result"]["unknown"] = true.into();
            assert!(!validator.is_valid(&invalid));
            if case == "reset" || case == "recovery" {
                let mut invalid = document;
                invalid["result"]["backupPath"] = serde_json::Value::Null;
                assert!(!validator.is_valid(&invalid));
            }
        }
    }
}
