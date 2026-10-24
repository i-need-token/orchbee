//! Immutable CLI artifacts shared while their test threads or fixtures are alive.

use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};

const UNUSED_ARTIFACT: &[u8] = b"unused cohort fixture";

thread_local! {
    // A thread retains preparation across its table-driven cases. The weak process-wide
    // index shares overlapping readers without keeping a directory alive after them.
    static COHORT: Arc<Cohort> = {
        static LIVE: Mutex<Weak<Cohort>> = Mutex::new(Weak::new());
        let mut live = LIVE.lock().unwrap();
        live.upgrade().unwrap_or_else(|| {
            let cohort = Arc::new(Cohort::private());
            *live = Arc::downgrade(&cohort);
            cohort
        })
    };
}

pub(super) struct Cohort(PathBuf);

impl Cohort {
    pub(super) fn shared() -> Arc<Self> {
        COHORT.with(Arc::clone)
    }

    pub(super) fn private() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-cli-cohort-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        let cohort = Self(root);
        // Native cohort admission requires private regular files with one link.
        fs::copy(env!("CARGO_BIN_EXE_orchbee"), cohort.executable()).unwrap();
        for name in ["orchbee-server", "orchbee-worker", "orchbee-terminal-host"] {
            fs::write(cohort.0.join(format!("{name}.exe")), UNUSED_ARTIFACT).unwrap();
        }
        fs::write(cohort.0.join("cohort.json"), manifest()).unwrap();
        cohort
    }

    #[allow(
        dead_code,
        reason = "Only cohort admission and relay suites inspect artifact paths."
    )]
    pub(super) fn directory(&self) -> &Path {
        &self.0
    }

    pub(super) fn executable(&self) -> PathBuf {
        self.0.join("orchbee.exe")
    }
}

impl Drop for Cohort {
    fn drop(&mut self) {
        super::remove_fixture_directory(&self.0, "orchbee-cli-cohort-");
    }
}

fn manifest() -> &'static [u8] {
    static MANIFEST: OnceLock<Vec<u8>> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let mut artifacts = serde_json::Map::new();
        for name in [
            "orchbee",
            "orchbee-server",
            "orchbee-worker",
            "orchbee-terminal-host",
        ] {
            let digest = if name == "orchbee" {
                Sha256::digest(fs::read(env!("CARGO_BIN_EXE_orchbee")).unwrap())
            } else {
                Sha256::digest(UNUSED_ARTIFACT)
            };
            artifacts.insert(name.to_owned(), json!(format!("{digest:x}")));
        }
        artifacts.insert(
            "agent-observation-adapters".to_owned(),
            json!(format!("{:x}", Sha256::digest(b"unused adapters"))),
        );
        serde_json::to_vec(&json!({"schemaVersion":1,"artifacts":artifacts})).unwrap()
    })
}
