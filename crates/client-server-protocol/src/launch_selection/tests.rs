use super::*;

#[test]
fn launch_selection_round_trips() {
    let selection = LaunchSelection::new("/bin/zsh".to_owned(), vec!["-l".to_owned()]).unwrap();
    let encoded = serde_json::to_vec(&selection).unwrap();
    let decoded: LaunchSelection = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, selection);
}

#[test]
fn launch_selection_rejects_relative_executable() {
    assert_eq!(
        LaunchSelection::new("zsh".to_owned(), Vec::new()),
        Err(LaunchSelectionError::ExecutableNotAbsolute)
    );
}

#[test]
fn launch_selection_rejects_empty_executable() {
    assert_eq!(
        LaunchSelection::new(String::new(), Vec::new()),
        Err(LaunchSelectionError::EmptyExecutable)
    );
}

#[test]
fn launch_selection_rejects_nul() {
    assert_eq!(
        LaunchSelection::new("/bin/zsh\0".to_owned(), Vec::new()),
        Err(LaunchSelectionError::ContainsNul)
    );
    assert_eq!(
        LaunchSelection::new("/bin/zsh".to_owned(), vec!["a\0b".to_owned()]),
        Err(LaunchSelectionError::ContainsNul)
    );
}

#[test]
fn launch_selection_rejects_argument_overflow() {
    let arguments = vec![String::new(); MAXIMUM_LAUNCH_SELECTION_ARGUMENTS + 1];
    assert_eq!(
        LaunchSelection::new("/bin/zsh".to_owned(), arguments),
        Err(LaunchSelectionError::TooManyArguments {
            actual: MAXIMUM_LAUNCH_SELECTION_ARGUMENTS + 1,
            maximum: MAXIMUM_LAUNCH_SELECTION_ARGUMENTS,
        })
    );
}

#[test]
fn launch_selection_rejects_byte_overflow() {
    let arguments = vec!["x".repeat(MAXIMUM_LAUNCH_SELECTION_BYTES)];
    assert_eq!(
        LaunchSelection::new("/bin/zsh".to_owned(), arguments),
        Err(LaunchSelectionError::TooLong {
            actual: 8 + MAXIMUM_LAUNCH_SELECTION_BYTES,
            maximum: MAXIMUM_LAUNCH_SELECTION_BYTES,
        })
    );
}

#[test]
fn launch_selection_deserialize_rejects_malformed() {
    assert!(serde_json::from_str::<LaunchSelection>(r#"{"exec":"zsh","args":[]}"#).is_err());
    assert!(serde_json::from_str::<LaunchSelection>(r#"{"exec":"/bin/zsh"}"#).is_err());
    assert!(
        serde_json::from_str::<LaunchSelection>(r#"{"exec":"/bin/zsh","args":[],"extra":1}"#)
            .is_err()
    );
}
