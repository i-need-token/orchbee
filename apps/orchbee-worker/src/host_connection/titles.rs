//! Connection-owned projection of exact physical OSC titles. Never Agent semantic state.
use orchbee_capability_node_protocol::{HostTerminalIdentity, TerminalTitleObservation};
use orchbee_terminal_protocol::TerminalMetadataChanged;
use std::{collections::BTreeMap, io, num::NonZeroU64};
const MAXIMUM_ROWS: usize = crate::terminal_authorization::MAXIMUM_RETAINED_TERMINAL_BINDINGS;
const MAXIMUM_BYTES: usize = 64 * 1024 * 1024;
#[derive(Default)]
pub(super) struct Titles {
    rows: BTreeMap<[u8; 16], (HostTerminalIdentity, NonZeroU64, Option<String>)>,
    bytes: usize,
}
impl Titles {
    pub(super) fn observe(&mut self, event: TerminalMetadataChanged) -> io::Result<()> {
        let identity = crate::terminal_host_launch_adapter::capability_identity(event.identity)
            .map_err(io::Error::other)?;
        let key = identity.terminal_id().as_bytes();
        let previous = self.rows.get(&key);
        if let Some((prior, revision, title)) = previous {
            if *prior != identity
                || event.revision < *revision
                || (event.revision == *revision && event.title != *title)
            {
                return Err(io::Error::other(
                    "invalid physical terminal title revision or identity",
                ));
            }
            if event.revision == *revision {
                return Ok(());
            }
        }
        let n = event.title.as_ref().map_or(0, String::len);
        if n > orchbee_capability_node_protocol::MAXIMUM_TERMINAL_TITLE_BYTES {
            return Err(io::Error::other("physical terminal title is too large"));
        }
        let old = previous
            .and_then(|(_, _, t)| t.as_ref())
            .map_or(0, String::len);
        let bytes = self
            .bytes
            .checked_sub(old)
            .and_then(|v| v.checked_add(n))
            .filter(|v| *v <= MAXIMUM_BYTES);
        if (previous.is_none() && self.rows.len() >= MAXIMUM_ROWS) || bytes.is_none() {
            return Err(io::Error::other(
                "physical terminal title capacity exhausted",
            ));
        }
        self.rows
            .insert(key, (identity, event.revision, event.title));
        self.bytes = bytes.expect("checked title bound");
        Ok(())
    }
    pub(super) fn read(&self, identity: HostTerminalIdentity) -> TerminalTitleObservation {
        match self
            .rows
            .get(&identity.terminal_id().as_bytes())
            .filter(|(i, _, _)| *i == identity)
        {
            Some((_, revision, title)) => TerminalTitleObservation::Observed {
                revision: *revision,
                title: title.clone(),
            },
            None => TerminalTitleObservation::Unavailable,
        }
    }
    pub(super) fn remove(&mut self, identity: HostTerminalIdentity) {
        let key = identity.terminal_id().as_bytes();
        if self.rows.get(&key).is_some_and(|(i, _, _)| *i == identity)
            && let Some((_, _, title)) = self.rows.remove(&key)
        {
            self.bytes -= title.as_ref().map_or(0, String::len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_terminal_protocol::{CreationId, HostEpoch, TerminalId, TerminalIdentity};
    fn event(revision: u64, title: Option<&str>) -> TerminalMetadataChanged {
        TerminalMetadataChanged {
            identity: TerminalIdentity {
                creation_id: CreationId::from_bytes([1; 16]).unwrap(),
                host_epoch: HostEpoch::from_bytes([2; 16]).unwrap(),
                terminal_id: TerminalId::from_bytes([3; 16]).unwrap(),
            },
            revision: NonZeroU64::new(revision).unwrap(),
            title: title.map(str::to_owned),
            current_directory: None,
        }
    }
    #[test]
    fn title_absence_clear_replay_revision_and_disposal_are_distinct() {
        let mut cache = Titles::default();
        let id = crate::terminal_host_launch_adapter::capability_identity(event(1, None).identity)
            .unwrap();
        assert_eq!(cache.read(id), TerminalTitleObservation::Unavailable);
        cache.observe(event(1, Some("⠋ 修复🙂标题"))).unwrap();
        cache.observe(event(1, Some("⠋ 修复🙂标题"))).unwrap();
        assert!(cache.observe(event(1, Some("changed"))).is_err());
        cache.observe(event(2, Some("⠙ 修复🙂标题"))).unwrap();
        assert!(cache.observe(event(1, None)).is_err());
        cache.observe(event(3, None)).unwrap();
        assert_eq!(
            cache.read(id),
            TerminalTitleObservation::Observed {
                revision: NonZeroU64::new(3).unwrap(),
                title: None
            }
        );
        assert_eq!(cache.bytes, 0);
        cache.remove(id);
        assert_eq!(cache.read(id), TerminalTitleObservation::Unavailable);
    }
    #[test]
    fn reused_host_terminal_cannot_inherit_another_creation_or_epoch() {
        let mut cache = Titles::default();
        cache.observe(event(1, Some("old"))).unwrap();
        let mut other = event(2, Some("new"));
        other.identity.creation_id = CreationId::from_bytes([9; 16]).unwrap();
        let id = crate::terminal_host_launch_adapter::capability_identity(other.identity).unwrap();
        assert_eq!(cache.read(id), TerminalTitleObservation::Unavailable);
        cache.remove(id);
        assert_eq!(cache.bytes, 3);
        assert!(cache.observe(other).is_err());
    }
    #[test]
    fn title_capacity_refusal_preserves_prior_observations_and_disposal_releases_it() {
        let mut cache = Titles::default();
        for n in 1..=MAXIMUM_ROWS {
            let mut next = event(1, Some("title"));
            next.identity.terminal_id = TerminalId::from_bytes((n as u128).to_be_bytes()).unwrap();
            cache.observe(next).unwrap();
        }
        let before = cache.bytes;
        let mut extra = event(1, Some("extra"));
        extra.identity.terminal_id = TerminalId::from_bytes([8; 16]).unwrap();
        assert!(cache.observe(extra.clone()).is_err());
        assert_eq!(cache.bytes, before);
        let mut first = event(1, None);
        first.identity.terminal_id = TerminalId::from_bytes(1_u128.to_be_bytes()).unwrap();
        cache.remove(
            crate::terminal_host_launch_adapter::capability_identity(first.identity).unwrap(),
        );
        cache.observe(extra).unwrap();
        assert_eq!(cache.rows.len(), MAXIMUM_ROWS);
        let mut huge = first;
        huge.title = Some("x".repeat(65537));
        assert!(cache.observe(huge).is_err());
    }
    #[cfg(windows)]
    fn windows_host() -> Result<
        (
            super::super::HostBrokerConnection,
            orchbee_local_ipc::LocalStream,
            orchbee_local_ipc::LocalStream,
            orchbee_local_ipc::LocalStream,
        ),
        Box<dyn std::error::Error>,
    > {
        use super::super::*;
        let (stream, peer) = UnixStream::pair()?;
        let (server, server_peer) = UnixStream::pair()?;
        let host = HostBrokerConnection {
            host_epoch: HostEpoch::from_bytes([2; 16])?,
            negotiated_limits: NegotiatedHostLimits {
                frame_bytes: 1048576,
                input_bytes: 4096,
                row_window_bytes: 4096,
            },
            stream,
            viewers: RefCell::new(HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: RefCell::new(Titles::default()),
            drain: Rc::new(RefCell::new(AgentDrain::new())),
        };
        Ok((host, peer, server, server_peer))
    }
    #[cfg(windows)]
    #[test]
    fn queued_windows_metadata_is_read_without_any_viewer_or_host_request()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::super::*;
        use orchbee_terminal_protocol::{
            Envelope, encode_terminal_effect_event, encode_transport_envelope,
        };
        use std::io::{Read, Write};
        let (host, mut peer, server, _server_peer) = windows_host()?;
        let id = crate::terminal_host_launch_adapter::capability_identity(event(1, None).identity)?;
        for (revision, title) in [
            (1, Some("⠋ 修复🙂标题")),
            (2, Some("⠙ 修复🙂标题")),
            (3, None),
        ] {
            let metadata = encode_terminal_effect_event(&TerminalEffectEvent::MetadataChanged(
                event(revision, title),
            ))?;
            let bytes = encode_transport_envelope(&Envelope {
                correlation_id: None,
                host_epoch: Some(host.host_epoch()),
                kind: metadata.kind,
                optional_message: false,
                payload: &metadata.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
            })?;
            peer.write_all(&bytes)?;
            HostExchange::new(&host, &server, Duration::from_secs(1))
                .unwrap()
                .prepare()
                .map_err(|_| std::io::Error::other("prepare failed"))?;
            assert_eq!(
                host.terminal_title(id),
                TerminalTitleObservation::Observed {
                    revision: NonZeroU64::new(revision).unwrap(),
                    title: title.map(str::to_owned)
                }
            );
        }
        peer.set_nonblocking(true)?;
        assert_eq!(
            peer.read(&mut [0; 1]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        Ok(())
    }
    #[cfg(windows)]
    #[test]
    fn title_cache_read_yields_after_a_bounded_event_batch_without_dropping_the_tail()
    -> Result<(), Box<dyn std::error::Error>> {
        use super::super::*;
        use orchbee_terminal_protocol::{
            Envelope, encode_terminal_effect_event, encode_transport_envelope,
        };
        use std::io::Write;
        let (host, mut peer, server, _server_peer) = windows_host()?;
        for revision in 1..=6 {
            let value = encode_terminal_effect_event(&TerminalEffectEvent::MetadataChanged(
                event(revision, Some("⠋ busy")),
            ))?;
            peer.write_all(&encode_transport_envelope(&Envelope {
                correlation_id: None,
                host_epoch: Some(host.host_epoch()),
                kind: value.kind,
                optional_message: false,
                payload: &value.payload,
                payload_encoding: PayloadEncoding::ControlJson,
                protocol: CURRENT_PROTOCOL_VERSION,
                role: MessageRole::Event,
            })?)?;
        }
        let exchange = HostExchange::new(&host, &server, Duration::from_secs(1)).unwrap();
        exchange
            .observe_pending(2)
            .map_err(|_| std::io::Error::other("bounded title read failed"))?;
        let id = crate::terminal_host_launch_adapter::capability_identity(event(1, None).identity)?;
        assert!(
            matches!(host.terminal_title(id),TerminalTitleObservation::Observed {revision,..} if revision.get()==2)
        );
        exchange
            .prepare()
            .map_err(|_| std::io::Error::other("remaining events failed"))?;
        assert!(
            matches!(host.terminal_title(id),TerminalTitleObservation::Observed {revision,..} if revision.get()==6)
        );
        Ok(())
    }
}
