use super::question::{attributed, changed_key, classify, prompt_text};
use super::{Start, start};
use orchbee_machine_protocol::{HostKey, KeyFingerprint, Token, hub::QuestionKind};
use std::ffi::OsString;

/// The host-key confirmation as OpenSSH 9.5 words it.
const NEW_HOST: &str = "The authenticity of host 'host.invalid (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:abcDEF0123456789+/abcDEF0123456789abcdefgh.\nThis key is not known by any other names.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
/// The Machine's saved target in these cases.
const TARGET: &str = "user@host.invalid";

fn offered() -> HostKey {
    HostKey {
        algorithm: Token::new("ed25519").unwrap(),
        fingerprint: KeyFingerprint::new("SHA256:abcDEF0123456789+/abcDEF0123456789abcdefgh")
            .unwrap(),
    }
}

/// Text a server or a jump host can put in a prompt, each written to pass for one of `ssh`'s own
/// sentences or to hide what it asks.
const HOSTILE: [&str; 12] = [
    "(user@host.invalid) Enter passphrase for key 'C:\\Users\\someone/.ssh/id_ed25519': ",
    "Please enter passphrase for key 'id_ed25519': ",
    "Enter passphrase for key 'id_ed25519':\nand then some",
    "(user@host.invalid) The authenticity of host 'host.invalid (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:serverChosenKEY0123456789abcdefghijklmnopq.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ",
    "Before you continue connecting, note: ED25519 key fingerprint is SHA256:serverChosen.\nAre you sure you want to continue connecting (yes/no)? ",
    "(user@host.invalid) One-time password:",
    "One-time password:",
    "(user@host.invalid) user@host.invalid's password:",
    "user@host.invalid's password:\nsecond line",
    "\u{1b}[2K\rEnter passphrase for key 'id_ed25519': ",
    "\u{202e}user@host.invalid's password: ",
    "Verification code: ",
];

#[test]
fn a_start_with_the_variable_never_selects_a_command() {
    let arguments = |words: &[&str]| words.iter().map(OsString::from).collect::<Vec<_>>();
    let helper = |prompt: &str| Start::Helper(OsString::from(prompt));
    for release in [false, true] {
        let cases: [(&[&str], Start); 9] = [
            (
                &["--installer-uninstall-v1"],
                helper("--installer-uninstall-v1"),
            ),
            (
                &["--installer-uninstall-v1", "--delete-data"],
                helper("--installer-uninstall-v1 --delete-data"),
            ),
            (&["--complete-install-v1"], helper("--complete-install-v1")),
            (&["--version"], helper("--version")),
            (&["--help"], helper("--help")),
            (&["machine", "list"], helper("machine list")),
            (
                &["user@host.invalid's", "password: "],
                helper("user@host.invalid's password: "),
            ),
            (
                &["--machine-link-v1", "--help"],
                helper("--machine-link-v1 --help"),
            ),
            (&[], helper("")),
        ];
        for (words, expected) in cases {
            assert_eq!(start(&arguments(words), release), expected, "{words:?}");
        }
    }
    // Only a build that is not a release lets the stand-in's own entry through, and only alone.
    let entry = arguments(&["--machine-link-v1"]);
    assert_eq!(start(&entry, false), Start::StandIn);
    assert_eq!(start(&entry, true), helper("--machine-link-v1"));
}

#[test]
fn only_ssh_s_own_sentences_read_as_their_kind() {
    for (prompt, kind) in [
        ("user@host.invalid's password: ", QuestionKind::Password),
        ("user@2001:db8::1's password: ", QuestionKind::Password),
        (
            "Enter passphrase for key 'C:\\Users\\someone/.ssh/id_ed25519': ",
            QuestionKind::KeyPassphrase,
        ),
        (
            NEW_HOST,
            QuestionKind::HostKey {
                host_key: offered(),
            },
        ),
        // A keyboard-interactive prompt carries `(user@host) ` and is the server's text.
        ("(user@host.invalid) Password:", QuestionKind::Other),
        ("(user@host.invalid) One-time code:", QuestionKind::Other),
        (
            "The authenticity of host 'host.invalid' can't be established.\nAre you sure you want to continue connecting (yes/no)? ",
            QuestionKind::Other,
        ),
    ] {
        assert_eq!(classify(prompt), kind, "{prompt:?}");
    }
    for prompt in HOSTILE {
        assert_eq!(classify(prompt), QuestionKind::Other, "{prompt:?}");
    }
}

#[test]
fn only_the_machine_s_own_host_keeps_a_password_or_a_new_host() {
    let new_host =
        |named: &str| NEW_HOST.replace("'host.invalid (192.0.2.1)'", &format!("'{named}'"));
    let host_key = QuestionKind::HostKey {
        host_key: offered(),
    };
    let ported = new_host("[host.invalid]:2222 ([192.0.2.1]:2222)");
    let jump = new_host("jump.invalid (192.0.2.9)");
    let longer = new_host("host.invalid.jump.invalid (192.0.2.9)");
    let cases: [(&str, &str, QuestionKind); 8] = [
        (
            "user@host.invalid's password: ",
            TARGET,
            QuestionKind::Password,
        ),
        (
            "user@host.invalid's password: ",
            "ssh://user@HOST.invalid:2222",
            QuestionKind::Password,
        ),
        // A jump host's inner `ssh` names the jump host.
        (
            "user@jump.invalid's password: ",
            TARGET,
            QuestionKind::Other,
        ),
        (NEW_HOST, TARGET, host_key.clone()),
        (&ported, "ssh://user@host.invalid:2222", host_key),
        (&jump, TARGET, QuestionKind::Other),
        (&longer, TARGET, QuestionKind::Other),
        // A key passphrase unlocks a local key, whichever host asks for it.
        (
            "Enter passphrase for key 'C:\\Users\\\u{5f20}\u{4e09}/.ssh/id_ed25519': ",
            TARGET,
            QuestionKind::KeyPassphrase,
        ),
    ];
    for (prompt, target, kind) in cases {
        assert_eq!(
            attributed(classify(prompt), prompt, target),
            kind,
            "{prompt:?}"
        );
    }
}

#[test]
fn only_the_local_new_host_sentence_turns_echo_on_or_takes_yes() {
    // The client reads with echo and answers `yes` only for a new host, so no other prompt may
    // keep that kind, whatever kind a record claims for it.
    let claims = [
        QuestionKind::Password,
        QuestionKind::KeyPassphrase,
        QuestionKind::HostKey {
            host_key: offered(),
        },
        QuestionKind::Other,
    ];
    let foreign_host = NEW_HOST.replace("'host.invalid (", "'jump.invalid (");
    let prompts = HOSTILE
        .iter()
        .copied()
        .chain(["user@host.invalid's password: ", foreign_host.as_str()]);
    for prompt in prompts {
        for claim in claims.clone() {
            let kind = attributed(claim, prompt, TARGET);
            assert!(
                !matches!(kind, QuestionKind::HostKey { .. }),
                "{prompt:?} read as {kind:?}"
            );
        }
    }
    assert_eq!(
        attributed(classify(NEW_HOST), NEW_HOST, TARGET),
        QuestionKind::HostKey {
            host_key: offered()
        }
    );
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
