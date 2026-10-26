use super::*;
use orchbee_workbench_protocol::{PresentationBinding, TerminalId};
fn rows() -> Value {
    json!([
        {"presentationId":identity(11), "path":"C:\\primary space\\项目"},
        {"presentationId":identity(15), "path":"D:\\child"}
    ])
}
#[test]
fn checkpoint_requires_strict_directory_rows() {
    for fault in 0..9 {
        let bad = changed(|v| {
            v["terminalDirectories"] = rows();
            match fault {
                0 => {
                    v.as_object_mut().unwrap().remove("terminalDirectories");
                }
                1 => v["terminalDirectories"] = Value::Null,
                2 => v["version"] = json!(2),
                3 => {
                    let row = v["terminalDirectories"][0].clone();
                    v["terminalDirectories"].as_array_mut().unwrap().push(row);
                }
                4 => v["terminalDirectories"][0]["presentationId"] = json!(identity(999)),
                5 => v["terminalDirectories"][0]["path"] = json!("relative"),
                6 => {
                    v["terminalDirectories"][0]["path"] = json!(format!("C:/{}", "x".repeat(4096)))
                }
                7 => v["terminalDirectories"][0]["extra"] = json!(true),
                _ => v["terminalDirectories"][0]["path"] = json!("C:/bad\0path"),
            }
        });
        assert!(WorkbenchCheckpoint::parse(&bad).is_err(), "fault {fault}");
    }
}
#[test]
fn primary_child_directories_override_defaults_without_changing_topology_and_reject_stale_targets()
{
    let saved = changed(|v| v["terminalDirectories"] = rows());
    let mut a = restore(&saved).unwrap();
    let before = a.snapshot().clone();
    let targets = a.restoration_targets(&worker(400)).unwrap();
    assert_eq!(
        targets[0].working_directory().unwrap().as_str(),
        "C:\\primary space\\项目"
    );
    assert_eq!(
        targets[1].working_directory().unwrap().as_str(),
        "D:\\child"
    );
    assert_eq!(
        targets[2].working_directory().unwrap().as_str(),
        "C:\\work space\\项目"
    );
    assert_eq!(a.snapshot(), &before);
    let b = PresentationBinding {
        worker_id: worker(400),
        terminal_id: TerminalId::parse(&identity(999)).unwrap(),
    };
    a.restart_directories.insert(
        targets[0].presentation_id().clone(),
        WorkbenchDirectoryPath::parse("C:/changed").unwrap(),
    );
    assert!(a.prepare_restored_terminal(&targets[0], b).is_err());
    assert_eq!(a.snapshot(), &before);
}
#[test]
fn fresh_observations_replace_intent_only_for_current_members_and_capture_is_pure() {
    let mut a = restore(&changed(|v| v["terminalDirectories"] = rows())).unwrap();
    let targets = a.restoration_targets(&worker(400)).unwrap();
    let binding = PresentationBinding {
        worker_id: worker(400),
        terminal_id: TerminalId::parse(&identity(999)).unwrap(),
    };
    a.prepare_restored_terminal(&targets[0], binding.clone())
        .unwrap()
        .commit();
    let before = a.snapshot().clone();
    let observed = (
        binding.clone(),
        WorkbenchDirectoryPath::parse("E:/new").unwrap(),
    );
    let bytes = a
        .checkpoint_with_terminal_directories(&worker(400), &[], std::slice::from_ref(&observed))
        .unwrap();
    let fresh = restore(&bytes).unwrap();
    let restored = fresh.restoration_targets(&worker(400)).unwrap();
    assert_eq!(restored[0].working_directory().unwrap().as_str(), "E:/new");
    assert_eq!(
        restored[1].working_directory().unwrap().as_str(),
        "D:\\child"
    );
    assert_eq!(a.snapshot(), &before);
    assert!(
        a.checkpoint_with_terminal_directories(
            &worker(400),
            &[],
            &[observed.clone(), observed.clone()]
        )
        .is_err()
    );
    let mut foreign = observed;
    foreign.0.worker_id = worker(500);
    assert!(
        a.checkpoint_with_terminal_directories(&worker(400), &[], &[foreign])
            .is_err()
    );
    a.restart_directories.insert(
        orchbee_workbench_protocol::TerminalPresentationId::parse(&identity(1000)).unwrap(),
        WorkbenchDirectoryPath::parse("C:/removed").unwrap(),
    );
    let v: Value = serde_json::from_slice(&a.checkpoint(&worker(400)).unwrap()).unwrap();
    assert_eq!(v["terminalDirectories"], rows());
}
