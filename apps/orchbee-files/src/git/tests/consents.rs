//! Records follow the window: `record-samples/` keeps a sample of every format of the consent
//! record a release shipped, named `git-parent-repositories-v<version>.json`, and each keeps
//! opening through today's reader, as the local runtime's record samples describe. Each answer
//! then replaces the record under its lock.

use super::{super::consents::Consents, fixture::Fixture};
use orchbee_local_runtime::{DataRoot, PrivateCache, VersionedFile, VersionedRecord};
use std::{fs, path::Path};

#[test]
fn every_stored_sample_opens_and_each_answer_replaces_the_record() {
    let record = VersionedRecord::GitParentRepositories;
    let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/git/tests/record-samples");
    let mut current = false;
    for entry in fs::read_dir(samples).unwrap() {
        let path = entry.unwrap().path();
        let bytes = fs::read(&path).unwrap();
        let fixture = Fixture::new();
        let root = DataRoot::at(Path::new(&fixture.path("data")).to_owned());
        let file = VersionedFile::device(root.clone(), record);
        PrivateCache::acquire(file.folder(), record.name())
            .unwrap()
            .replace(record.file_name(), &bytes)
            .unwrap();
        let consents = Consents::new(Some(root));
        let stored = serde_json::from_slice::<serde_json::Value>(&bytes).unwrap();
        assert_eq!(
            serde_json::Value::from(consents.read().unwrap()),
            stored["repositories"],
            "{path:?}"
        );
        current |= stored["version"] == record.current_version();

        let added = fixture.path("added");
        consents.answer(&added, true).unwrap();
        consents.answer(&added, true).unwrap();
        let listed = consents.read().unwrap();
        assert_eq!(listed.iter().filter(|path| **path == added).count(), 1);
        consents.answer(&added, false).unwrap();
        assert!(!consents.read().unwrap().contains(&added));
    }
    assert!(current, "a sample of the current version");
}
