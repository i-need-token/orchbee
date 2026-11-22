use super::*;
use std::{fs, path::PathBuf};

/// A private folder of its own under the temporary folder, removed afterwards.
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let id = ProfileId::generate().unwrap();
        let path =
            std::env::temp_dir().join(format!("orchbee-machine-spaces-test-{}", id.as_str()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-machine-spaces-test-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

/// Records follow the window: `tests/record-samples/` keeps a sample of every format of the list
/// a release shipped, named `machine-spaces-v<version>.json`, and each keeps opening through the
/// reader, as the local runtime's record samples describe.
#[test]
fn every_stored_sample_opens_through_the_reader() {
    let samples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/machine_spaces/last_seen/tests/record-samples");
    let mut current = false;
    for entry in fs::read_dir(samples).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        let root = Root::new();
        let data = DataRoot::at(root.0.join("data"));
        let file = VersionedFile::device(data.clone(), VersionedRecord::MachineSpaces);
        orchbee_local_runtime::PrivateCache::acquire(
            file.folder(),
            VersionedRecord::MachineSpaces.name(),
        )
        .unwrap()
        .replace(VersionedRecord::MachineSpaces.file_name(), &bytes)
        .unwrap();
        let machines = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["machines"]
            .as_object()
            .unwrap()
            .keys()
            .map(|id| ProfileId::parse(id).unwrap())
            .collect::<Vec<_>>();
        let listed = LastSeen::in_data_root(data).checkpoints(&machines);
        assert!(
            listed.len() == machines.len()
                && listed
                    .iter()
                    .all(|(_, checkpoint)| !checkpoint.rows.is_empty()),
            "{path:?}"
        );
        current |= serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["version"]
            == VersionedRecord::MachineSpaces.current_version();
    }
    assert!(current, "a sample of the current version");
}
