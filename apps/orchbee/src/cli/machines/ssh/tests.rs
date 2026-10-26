use super::*;
use serde_json::json;
#[cfg(windows)]
mod windows;
fn client_document() -> Value {
    json!({"schema":"orchbee.status.client","version":1,"result":{
        "version":env!("CARGO_PKG_VERSION"),"platform":"windows","architecture":std::env::consts::ARCH,
        "protocols":{"admission":orchbee_local_client_protocol::PROTOCOL_VERSION,
            "terminal":orchbee_client_server_protocol::PROTOCOL_VERSION},
        "cohort":{"state":"available","identity":"a".repeat(64)},
        "executable":{"path":"C:/remote/orchbee.exe"}}})
}
#[test]
fn installed_client_admission_requires_current_document_version() {
    let mut document = client_document();
    let admitted = client(&document).unwrap();
    assert_eq!(admitted.executable, "C:/remote/orchbee.exe");
    assert_eq!(admitted.cohort, "a".repeat(64));
    document["version"] = json!(2);
    assert!(client(&document).is_err());
}

fn server_document() -> Value {
    json!({"schema":"orchbee.status.server","version":1,"result":{
        "server":{"state":"running","stopping":false,"version":env!("CARGO_PKG_VERSION"),
            "cohort":"a".repeat(64),"protocols":client_document()["result"]["protocols"]},
        "localCohort":{"identity":"a".repeat(64)},"compatibility":{"compatible":true}}})
}
#[test]
fn preparation_requires_a_fresh_compatible_observation_and_retains_partial_state() {
    for failure in 0..=3 {
        let mut calls = Vec::new();
        let mut remote = Remote::NotContacted;
        let result = prepare_session(&mut remote, |path, action| {
            calls.push(action.to_owned());
            if calls.len() == failure {
                return Err(Error::Cancelled);
            }
            Ok(match action {
                "client" => {
                    assert!(path.is_none());
                    client_document()
                }
                "start" => {
                    assert_eq!(path, Some("C:/remote/orchbee.exe"));
                    json!({"schema":"orchbee.server.start","version":1,"result":{"outcome":"already-running"}})
                }
                "server" => server_document(),
                _ => panic!("unexpected call"),
            })
        });
        if failure == 0 {
            assert_eq!(result, Ok(()));
            assert_eq!(remote.token(), "ready");
            assert_eq!(calls, ["client", "start", "server"]);
        } else {
            assert_eq!(result, Err(Error::Cancelled));
            assert_eq!(calls.len(), failure);
            assert_eq!(
                remote.token(),
                if failure == 1 {
                    "unchanged"
                } else {
                    "possibly-started"
                }
            );
        }
    }
    let client = client(&client_document()).unwrap();
    for (field, value) in [
        ("state", json!("not-running")),
        ("stopping", json!(true)),
        ("cohort", json!("b".repeat(64))),
        ("version", json!("old")),
        ("protocols", json!({"admission":0,"terminal":0})),
    ] {
        let mut v = server_document();
        v["result"]["server"][field] = value;
        assert_eq!(server(&v, &client), Err(Error::RemoteNotReady));
    }
    let mut v = client_document();
    v["result"]["platform"] = json!("linux");
    assert!(matches!(super::client(&v), Err(Error::RemoteIncompatible)));
    v = client_document();
    v["result"]["executable"]["path"] = json!("relative.exe");
    assert!(matches!(super::client(&v), Err(Error::RemoteProtocol)));
}
