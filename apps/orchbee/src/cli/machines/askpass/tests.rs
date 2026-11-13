use super::question::{changed_key, classify, prompt_text};
use orchbee_machine_protocol::hub::QuestionKind;

/// The host-key confirmation as OpenSSH 9.5 words it.
const NEW_HOST: &str = "The authenticity of host 'host.invalid (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:abcDEF0123456789+/abcDEF0123456789abcdefgh.\nThis key is not known by any other names.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";

#[test]
fn each_recorded_prompt_reads_as_its_kind() {
    assert_eq!(
        classify("user@host.invalid's password: "),
        QuestionKind::Password
    );
    assert_eq!(
        classify("(user@host.invalid) Password:"),
        QuestionKind::Password
    );
    assert_eq!(
        classify("Enter passphrase for key 'C:\\Users\\someone/.ssh/id_ed25519': "),
        QuestionKind::KeyPassphrase
    );
    assert_eq!(classify("Verification code: "), QuestionKind::Other);
    assert_eq!(
        classify("(user@host.invalid) One-time code:"),
        QuestionKind::Other
    );
}

#[test]
fn a_new_host_carries_the_offered_key_exactly_as_ssh_printed_it() {
    let QuestionKind::HostKey { host_key } = classify(NEW_HOST) else {
        panic!("a host-key confirmation reads as host-key");
    };
    assert_eq!(host_key.algorithm.as_str(), "ed25519");
    assert_eq!(
        host_key.fingerprint.as_str(),
        "SHA256:abcDEF0123456789+/abcDEF0123456789abcdefgh"
    );
}

#[test]
fn a_confirmation_without_a_readable_key_is_shown_as_other() {
    let unreadable = "The authenticity of host 'host.invalid' can't be established.\nAre you sure you want to continue connecting (yes/no)? ";
    assert_eq!(classify(unreadable), QuestionKind::Other);
}

#[test]
fn a_changed_key_refusal_names_the_offered_key() {
    let refusal = b"@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\r\n\
@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\r\n\
The fingerprint for the ED25519 key sent by the remote host is\r\n\
SHA256:changedKEY0123456789abcdefghijklmnopqrstuv.\r\n\
Host key verification failed.\r\n";
    let host_key = changed_key(refusal).expect("the refusal names a key");
    assert_eq!(host_key.algorithm.as_str(), "ed25519");
    assert_eq!(
        host_key.fingerprint.as_str(),
        "SHA256:changedKEY0123456789abcdefghijklmnopqrstuv"
    );
    assert_eq!(changed_key(b"Host key verification failed.\r\n"), None);
}

#[test]
fn a_prompt_is_cut_on_a_character_boundary_within_its_bound() {
    let long = "\u{4e2d}".repeat(2000);
    let prompt = prompt_text(&long).expect("a long prompt is cut, not refused");
    assert!(prompt.as_str().len() <= 4096);
    assert!(long.starts_with(prompt.as_str()));
    assert!(prompt_text("").is_none());
}
