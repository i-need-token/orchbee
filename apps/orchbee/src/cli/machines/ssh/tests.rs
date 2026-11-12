use super::*;
use serde_json::json;
#[cfg(windows)]
mod windows;
/// The remote installation of this build's environment kind, at another product version and
/// protocol than this build.
fn remote_executable() -> String {
    format!(
        "C:/remote/{}.exe",
        orchbee_local_runtime::environment_stem()
    )
}
fn client_document() -> Value {
    json!({"schema":"orchbee.status.client","version":1,"result":{
        "version":"0.0.0-remote","platform":"windows","architecture":"aarch64",
        "protocols":{"admission":1,"terminal":255},
        "cohort":{"state":"available","identity":"a".repeat(64)},
        "executable":{"path":remote_executable()}}})
}
#[test]
fn installed_client_admission_accepts_another_product_version_and_requires_document_version() {
    let mut document = client_document();
    let admitted = client(&document).unwrap();
    assert_eq!(admitted.executable, remote_executable());
    assert_eq!(admitted.version, "0.0.0-remote");
    // This test build is not release, so a release remote belongs to another environment.
    document["result"]["executable"]["path"] = json!("C:/remote/orchbee.exe");
    assert!(matches!(
        client(&document),
        Err(Error::RemoteOtherEnvironment)
    ));
    assert_eq!(Error::RemoteOtherEnvironment.status(), 6);
    document = client_document();
    document["version"] = json!(2);
    assert!(client(&document).is_err());
}

fn server_document() -> Value {
    json!({"schema":"orchbee.status.server","version":1,"result":{
        "server":{"state":"running","restartKeepsTerminals":true,"processes":[
            {"role":"server","version":"0.0.0-remote","processId":7,"state":"ready","terminals":0},
            {"role":"worker","version":"0.0.0-remote","processId":8,"state":"ready","terminals":0}]},
        "clientVersion":"0.0.0-remote"}})
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
                    assert_eq!(path, Some(remote_executable().as_str()));
                    json!({"schema":"orchbee.server.start","version":1,"result":{"outcome":"already-running"}})
                }
                "server" => server_document(),
                _ => panic!("unexpected call"),
            })
        });
        if failure == 0 {
            assert_eq!(result, Ok("0.0.0-remote".to_owned()));
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
    for (field, value) in [("state", json!("not-running")), ("processes", json!([]))] {
        let mut v = server_document();
        v["result"]["server"][field] = value;
        assert_eq!(server(&v), Err(Error::RemoteNotReady));
    }
    let mut v = server_document();
    v["result"]["server"]["processes"][0]["state"] = json!("draining");
    assert_eq!(server(&v), Err(Error::RemoteNotReady));
    let mut v = client_document();
    v["result"]["platform"] = json!("linux");
    assert!(matches!(super::client(&v), Err(Error::RemoteUnsupported)));
    v = client_document();
    v["result"]["executable"]["path"] = json!("relative.exe");
    assert!(matches!(super::client(&v), Err(Error::RemoteProtocol)));
}
