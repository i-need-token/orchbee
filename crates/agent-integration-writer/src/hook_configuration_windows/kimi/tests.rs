use super::super::{HookConfigurationKind, HookConfigurationScan, scan_hook_configuration};
use super::*;
use toml_edit::DocumentMut;

fn patch(bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    super::patch(bytes, root, enable, super::super::ROOT_POLICY)
}

fn scan(bytes: &[u8]) -> HookConfigurationScan {
    scan_hook_configuration(HookConfigurationKind::Kimi, bytes)
}

#[test]
fn both_representations_preserve_foreign_values_and_repair_partial_sets() {
    let root = Path::new(r"C:\owned\0.4.17");
    for original in [
        "# user's header\nlarge = 9223372036854775807 # keep integer\nhooks = [{ event = 'PreToolUse', command = 'echo user', matcher = 'Bash' }] # keep rule\n[custom]\nvalue = '''not a [[hooks]] header'''\n",
        "# user's header\nlarge = 9223372036854775807 # keep integer\n[[hooks]] # keep rule\nevent = 'PreToolUse'\ncommand = 'echo user'\nmatcher = 'Bash'\n[custom]\nvalue = '''not a [[hooks]] header'''\n",
        "hooks = [] # empty inline\n",
        "",
    ] {
        let installed = patch(Some(original.as_bytes()), root, true).unwrap();
        assert!(matches!(scan(&installed), HookConfigurationScan::Owned(found) if found == root));
        assert_eq!(patch(Some(&installed), root, true).unwrap(), installed);
        let disabled = patch(Some(&installed), root, false).unwrap();
        assert!(matches!(scan(&disabled), HookConfigurationScan::Absent));
        let text = std::str::from_utf8(&disabled).unwrap();
        if original.contains("9223372036854775807") {
            for retained in [
                "# user's header",
                "9223372036854775807 # keep integer",
                "# keep rule",
                "'''not a [[hooks]] header'''",
                "'echo user'",
            ] {
                assert!(text.contains(retained), "{retained}: {text}");
            }
        }
        let mut doc = std::str::from_utf8(&installed)
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        remove_last_hook(&mut doc);
        let partial = doc.to_string().into_bytes();
        assert!(
            matches!(scan(&partial), HookConfigurationScan::Incomplete(found) if found == root)
        );
        let repaired = patch(Some(&partial), root, true).unwrap();
        assert!(matches!(scan(&repaired), HookConfigurationScan::Owned(_)));
    }
}
fn remove_last_hook(doc: &mut DocumentMut) {
    let hooks = doc.get_mut("hooks").unwrap();
    if let Some(array) = hooks.as_array_of_tables_mut() {
        array.remove(array.len() - 1);
    } else {
        let array = hooks.as_array_mut().unwrap();
        array.remove(array.len() - 1);
    }
}
#[test]
fn edited_duplicate_mixed_root_and_malformed_rules_refuse() {
    let root = Path::new(r"C:\owned\0.4.17");
    let installed = String::from_utf8(patch(None, root, true).unwrap()).unwrap();
    for edited in [
        installed.replace("timeout = 10", "timeout = 11"),
        installed.replace("SessionEnd", "Stop"),
        format!("{installed}{installed}"),
        installed.replace("timeout = 10", "timeout = 10\nmatcher = '*'"),
        installed.replace("timeout = 10", "timeout = 10\nunknown = true"),
    ] {
        assert!(matches!(
            scan(edited.as_bytes()),
            HookConfigurationScan::Conflict
        ));
        assert!(patch(Some(edited.as_bytes()), root, false).is_none());
    }
    let other =
        String::from_utf8(patch(None, Path::new(r"C:\other\0.4.17"), true).unwrap()).unwrap();
    let mixed = format!(
        "[[hooks]]{}[[hooks]]{}",
        installed.split("[[hooks]]").nth(1).unwrap(),
        other.split("[[hooks]]").nth(2).unwrap()
    );
    assert!(matches!(
        scan(mixed.as_bytes()),
        HookConfigurationScan::Conflict
    ));
    for invalid in [
        "hooks = 1",
        "hooks = [{}]",
        "hooks = []\nhooks = []",
        "[[hooks]",
        "hooks = [1]",
    ] {
        assert!(matches!(
            scan(invalid.as_bytes()),
            HookConfigurationScan::Malformed
        ));
    }
    assert!(matches!(scan(&[255]), HookConfigurationScan::Malformed));
}
