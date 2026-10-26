//! Application cursors carry complete-source revisions and ordering; no retained snapshots.
use super::read::Checkpoint;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use orchbee_agent_observation_protocol::AgentSafeInteger;
use orchbee_workbench_protocol::{SessionId, Snapshot, WorkbenchClientId, WorkbenchRevision};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Cursor(String);
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Position {
    #[serde(rename = "session")]
    session: SessionId,
    workbench: WorkbenchRevision,
    canonical: AgentSafeInteger,
    reports: WorkbenchRevision,
    include_ended: bool,
    client: Option<WorkbenchClientId>,
    offset: usize,
    fingerprint: String,
}
impl Cursor {
    pub(crate) fn parse(value: &str) -> Result<Self, ()> {
        let encoded = value.strip_prefix("u1.").ok_or(())?;
        if value.len() > 1024 {
            return Err(());
        }
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| ())?;
        let p: Position = serde_json::from_slice(&bytes).map_err(|_| ())?;
        if p.offset == 0
            || p.offset >= super::MAXIMUM_ROWS
            || p.fingerprint.len() != 64
            || !p
                .fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || URL_SAFE_NO_PAD.encode(serde_json::to_vec(&p).map_err(|_| ())?) != encoded
        {
            return Err(());
        }
        Ok(Self(value.into()))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn offset(
        &self,
        snapshot: &Snapshot,
        read: &Checkpoint,
        include_ended: bool,
        client: Option<&WorkbenchClientId>,
    ) -> Result<usize, ()> {
        let bytes = URL_SAFE_NO_PAD
            .decode(self.0.strip_prefix("u1.").ok_or(())?)
            .map_err(|_| ())?;
        let p: Position = serde_json::from_slice(&bytes).map_err(|_| ())?;
        if p != position(snapshot, read, include_ended, client, p.offset)?
            || p.offset >= read.rows.len()
        {
            return Err(());
        }
        Ok(p.offset)
    }
    pub(crate) fn next(
        snapshot: &Snapshot,
        read: &Checkpoint,
        include_ended: bool,
        client: Option<&WorkbenchClientId>,
        offset: usize,
    ) -> Result<Self, ()> {
        let p = position(snapshot, read, include_ended, client, offset)?;
        Self::parse(&format!(
            "u1.{}",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&p).map_err(|_| ())?)
        ))
    }
}
fn position(
    snapshot: &Snapshot,
    read: &Checkpoint,
    include_ended: bool,
    client: Option<&WorkbenchClientId>,
    offset: usize,
) -> Result<Position, ()> {
    let mut hash = Sha256::new();
    for (i, row) in read.rows.iter().enumerate() {
        hash.update(serde_json::to_vec(&row.document(i + 1, None)).map_err(|_| ())?)
    }
    Ok(Position {
        session: snapshot.session_id.clone(),
        workbench: snapshot.revision,
        canonical: read.canonical_revision,
        reports: read.report_revision,
        include_ended,
        client: client.cloned(),
        offset,
        fingerprint: format!("{:x}", hash.finalize()),
    })
}
