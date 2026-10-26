//! Client admission adapter: only the managed Windows composition supplies a stop owner.
use crate::agent_service::AgentService;
use orchbee_local_client_protocol::{SessionStopRequest, SessionStopResult, SessionStopStatus};

pub(super) struct StopControl<'a> {
    pub(super) agents: &'a AgentService,
    #[cfg(windows)]
    pub(super) shutdown: Option<&'a crate::session_shutdown::SessionShutdown>,
}

pub(super) struct Lease<'a> {
    #[cfg(windows)]
    _record: Option<crate::session_shutdown::RecordLease<'a>>,
    _control: &'a StopControl<'a>,
}

// Keep the same explicit admission-release boundary on adapters with no lifecycle owner.
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        #[cfg(windows)]
        self._record.take();
    }
}

impl<'a> StopControl<'a> {
    #[cfg(windows)]
    pub(super) fn metadata_mode(&self) -> orchbee_local_client_protocol::ServerMode {
        use orchbee_local_client_protocol::ServerMode;
        match self.shutdown {
            None => ServerMode::Unmanaged,
            Some(owner) if owner.stopping() => ServerMode::ManagedStopping,
            Some(_) => ServerMode::ManagedReady,
        }
    }

    pub(super) fn unmanaged(agents: &'a AgentService) -> Self {
        Self {
            agents,
            #[cfg(windows)]
            shutdown: None,
        }
    }

    pub(super) fn request(&self, request: &SessionStopRequest) -> SessionStopResult {
        #[cfg(windows)]
        if let Some(owner) = self.shutdown {
            return owner.request(request);
        }
        SessionStopResult {
            id: request.id,
            status: SessionStopStatus::Unmanaged,
        }
    }

    pub(super) fn enter(&'a self) -> Option<Lease<'a>> {
        #[cfg(windows)]
        let record = match self.shutdown {
            Some(owner) => Some(owner.enter()?),
            None => None,
        };
        Some(Lease {
            #[cfg(windows)]
            _record: record,
            _control: self,
        })
    }
}
