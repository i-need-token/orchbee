use super::without_activation;

/// Grok 1.0.77 `plugin install --trust` appends this table and `plugin uninstall` keeps it;
/// taking the name back restores the bytes install found, while user entries stay.
#[test]
fn disable_takes_back_what_install_listed() {
    let found = "[cli]\ninstaller = \"x\"\n\n[ui]\nyolo = false\n\n[privacy]\nprivacy_banner_acked = true\n";
    let installed = format!("{found}\n[plugins]\nenabled = [\"orchbee-agent-observation\"]\n");
    assert_eq!(
        without_activation(installed.as_bytes()).unwrap(),
        found.as_bytes()
    );
    let user = "[plugins]\nenabled = [\"team\", \"orchbee-agent-observation\"] # mine\ndisabled = [\"user/0123abcd/orchbee-agent-observation\"]\n";
    assert_eq!(
        String::from_utf8(without_activation(user.as_bytes()).unwrap()).unwrap(),
        "[plugins]\nenabled = [\"team\"] # mine\ndisabled = [\"user/0123abcd/orchbee-agent-observation\"]\n"
    );
    let disabled = "[plugins]\nenabled = []\ndisabled = [\"orchbee-agent-observation\"]\n";
    assert_eq!(
        without_activation(disabled.as_bytes()).unwrap(),
        b"[plugins]\nenabled = []\n"
    );
    for untouched in ["", "[plugins]\nenabled = []\n", "[ui]\nyolo = true\n"] {
        assert_eq!(
            without_activation(untouched.as_bytes()).unwrap(),
            untouched.as_bytes()
        );
    }
    assert!(without_activation(b"[plugins\n").is_none());
}
