//! A bounded independent projection/control lane. Canonical Pane state is never modified.
use crate::{
    local_client::{LocalClient, LocalClientOrigin, LocalOperationClock},
    terminal_events::{
        DecodedItem, DecodedKind, MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent,
        encode_cursor_arrows, encode_pane_report,
    },
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{
    ControlInput, Geometry, PasteContent, RequestId, TerminalControlAction, TerminalControlOutcome,
    TerminalControlRequest, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest,
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch, MouseTracking};
use orchbee_workbench_protocol::{
    ClientViewCompletionOutcome, ClientViewControlReason as Reason, PluginAction, PluginOutcome,
    PluginPopupSize, PluginPopupSpec, PluginRequest, ProtocolVersion, ScrollbackEditorSpec,
    SessionId, WorkbenchRequestId, WorkbenchTerminalGeometry,
};
use ratatui::layout::Rect;

use crate::background_thread::BackgroundThread;
use std::{
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

mod editor;
mod protocol;
use protocol::{PopupKind, PopupOutcome};

#[derive(Clone)]
enum PopupSpec {
    Configured(Box<orchbee_workbench_protocol::ConfiguredCommandPopupSpec>),
    Plugin(Box<PluginPopupSpec>),
    Editor(Box<ScrollbackEditorSpec>),
}
impl PopupSpec {
    fn kind(&self) -> PopupKind {
        match self {
            Self::Configured(_) => PopupKind::Configured,
            Self::Plugin(_) => PopupKind::Plugin,
            Self::Editor(_) => PopupKind::Editor,
        }
    }
    fn width(&self) -> Option<PluginPopupSize> {
        match self {
            Self::Configured(spec) => spec.width,
            Self::Plugin(spec) => spec.width,
            Self::Editor(_) => Some(PluginPopupSize::Percent(100)),
        }
    }
    fn height(&self) -> Option<PluginPopupSize> {
        match self {
            Self::Configured(spec) => spec.height,
            Self::Plugin(spec) => spec.height,
            Self::Editor(_) => Some(PluginPopupSize::Percent(100)),
        }
    }
    fn title(&self) -> String {
        match self {
            Self::Configured(_) => String::new(),
            Self::Plugin(spec) => spec.selector.entrypoint_id.clone(),
            Self::Editor(_) => String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Opening,
    Active,
    Closing,
    Closed,
    Refused(Reason),
    Failed(Reason),
}
#[derive(Clone)]
struct Latest {
    serial: u64,
    phase: Phase,
    title: String,
    projection: Option<Arc<TerminalProjection>>,
    geometry: Geometry,
}
enum Input {
    Bytes(ControlInput),
    Paste(PasteContent),
}
struct Lane {
    sender: Option<SyncSender<Input>>,
    latest: Arc<Mutex<Latest>>,
    resize: Arc<Mutex<Geometry>>,
    closing: Arc<AtomicBool>,
    stopping: Arc<AtomicBool>,
    wake: LocalStream,
    task: BackgroundThread,
}
impl Lane {
    fn interrupt(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.sender.take();
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
    }
    /// Joins the interrupted thread; only Client retirement waits here.
    fn close(mut self) {
        self.interrupt();
        let _ = self.task.join();
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        // Only close and the exit deadline wait; an unfinished thread is detached here.
        self.interrupt();
    }
}

pub(crate) struct PopupView<'a> {
    pub(crate) id: &'a WorkbenchRequestId,
    pub(crate) title: &'a str,
    pub(crate) projection: Option<&'a TerminalProjection>,
    pub(crate) width: Option<PluginPopupSize>,
    pub(crate) height: Option<PluginPopupSize>,
    pub(crate) closing: bool,
}

/// Exact independent-popup incarnation that painted one native close action.
///
/// Popup request IDs are unique opening identities. The close owner still
/// revalidates this witness against the current captured lane on release, so
/// a late click can never migrate to a replacement popup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnPopupClose(WorkbenchRequestId);

impl DrawnPopupClose {
    pub(crate) const fn id(&self) -> &WorkbenchRequestId {
        &self.0
    }
}

impl PopupView<'_> {
    pub(crate) fn close_target(&self) -> DrawnPopupClose {
        DrawnPopupClose(self.id.clone())
    }
}

/// Current exclusive-input identity for the independent popup.
///
/// Opening and closing popups still capture input, but only an exact active
/// projection may forward semantic input to the popup terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupInputContext {
    id: WorkbenchRequestId,
    interactive: bool,
}

impl PopupInputContext {
    #[cfg(test)]
    pub(crate) fn for_test(id: WorkbenchRequestId, interactive: bool) -> Self {
        Self { id, interactive }
    }

    pub(crate) fn id(&self) -> &WorkbenchRequestId {
        &self.id
    }

    pub(crate) const fn interactive(&self) -> bool {
        self.interactive
    }
}

pub(crate) fn resolve_geometry(
    width: Option<PluginPopupSize>,
    height: Option<PluginPopupSize>,
    area: Rect,
) -> Option<(Rect, Rect)> {
    if area.width < 6 || area.height < 4 {
        return None;
    }
    let dimension = |size: Option<PluginPopupSize>, available: u16, minimum| {
        let value = match size {
            Some(PluginPopupSize::Cells(value)) => value,
            Some(PluginPopupSize::Percent(percent)) => {
                (u32::from(available) * u32::from(percent) / 100) as u16
            }
            None => available / 2,
        };
        value.clamp(minimum, available)
    };
    let width = dimension(width, area.width, 6);
    let height = dimension(height, area.height, 4);
    let outer = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    Some((
        outer,
        Rect::new(outer.x + 1, outer.y + 1, width - 2, height - 2),
    ))
}

pub(crate) struct Driver {
    origin: Option<LocalClientOrigin>,
    clock: LocalOperationClock,
    lane: Option<Lane>,
    id: Option<WorkbenchRequestId>,
    spec: Option<PopupSpec>,
    latest: Option<Latest>,
    surface: Option<(Rect, u16, u16)>,
    close_had_popup: bool,
    last_mouse_cell: Option<(u16, u16)>,
    mouse_pressed: bool,
}
impl Driver {
    pub(crate) fn new(clock: LocalOperationClock) -> Self {
        Self {
            origin: None,
            clock,
            lane: None,
            id: None,
            spec: None,
            latest: None,
            surface: None,
            close_had_popup: false,
            last_mouse_cell: None,
            mouse_pressed: false,
        }
    }
    pub(crate) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.shutdown();
        self.origin = Some(origin);
    }
    /// Process exit: asks the popup thread to stop without waiting.
    pub(crate) fn request_stop(&mut self) {
        if let Some(lane) = &mut self.lane {
            lane.interrupt();
        }
    }
    /// Process exit: observes the popup thread's end against the shared deadline.
    pub(crate) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.lane.as_mut().map_or(Ok(()), |lane| {
            lane.interrupt();
            lane.task.join_before(deadline)
        })
    }
    pub(crate) fn shutdown(&mut self) {
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        self.latest = None;
        self.id = None;
        self.spec = None;
        self.origin = None;
        self.surface = None;
        self.mouse_pressed = false;
        self.last_mouse_cell = None;
    }
    pub(crate) fn busy(&self) -> bool {
        self.latest
            .as_ref()
            .is_some_and(|latest| !matches!(latest.phase, Phase::Closed | Phase::Refused(_)))
    }
    pub(crate) fn failed(&self) -> bool {
        self.latest
            .as_ref()
            .is_some_and(|latest| matches!(latest.phase, Phase::Failed(_)))
    }
    pub(crate) fn identity(&self) -> Option<&WorkbenchRequestId> {
        self.captured().then_some(self.id.as_ref()).flatten()
    }
    pub(crate) fn captured(&self) -> bool {
        self.latest.as_ref().is_some_and(|latest| {
            matches!(
                latest.phase,
                Phase::Opening | Phase::Active | Phase::Closing
            )
        })
    }
    pub(crate) fn input_context(&self) -> Option<PopupInputContext> {
        if !self.captured() {
            return None;
        }
        Some(PopupInputContext {
            id: self.id.clone()?,
            interactive: self.input_ready(),
        })
    }
    pub(crate) fn observe_surface(&mut self, area: Rect, pixels: (u16, u16)) {
        let previous = self.surface;
        self.surface = Some((area, pixels.0, pixels.1));
        if previous != self.surface {
            self.last_mouse_cell = None;
            self.mouse_pressed = false;
        }
        if let (Some(lane), Some((_, inner))) = (
            &self.lane,
            self.spec
                .as_ref()
                .and_then(|spec| resolve_geometry(spec.width(), spec.height(), area)),
        ) && let Ok(geometry) = Geometry::new(inner.width, inner.height, pixels.0, pixels.1)
            && let Ok(mut desired) = lane.resize.lock()
        {
            *desired = geometry;
        }
    }
    pub(crate) fn begin_open(
        &mut self,
        session: SessionId,
        id: WorkbenchRequestId,
        spec: PluginPopupSpec,
    ) -> Result<(), Reason> {
        self.begin_spec(session, id, PopupSpec::Plugin(Box::new(spec)))
    }
    pub(crate) fn begin_editor(
        &mut self,
        session: SessionId,
        id: WorkbenchRequestId,
        spec: ScrollbackEditorSpec,
    ) -> Result<(), Reason> {
        spec.validate().map_err(|_| Reason::StaleView)?;
        self.begin_spec(session, id, PopupSpec::Editor(Box::new(spec)))
    }
    pub(crate) fn begin_configured(
        &mut self,
        session: SessionId,
        id: WorkbenchRequestId,
        spec: orchbee_workbench_protocol::ConfiguredCommandPopupSpec,
    ) -> Result<(), Reason> {
        spec.validate().map_err(|_| Reason::StaleView)?;
        self.begin_spec(session, id, PopupSpec::Configured(Box::new(spec)))
    }
    fn begin_spec(
        &mut self,
        session: SessionId,
        id: WorkbenchRequestId,
        spec: PopupSpec,
    ) -> Result<(), Reason> {
        if self.busy() {
            return Err(Reason::Busy);
        }
        let origin = self.origin.clone().ok_or(Reason::ProjectionUnavailable)?;
        let (surface, width, height) = self.surface.ok_or(Reason::Busy)?;
        let (_, inner) =
            resolve_geometry(spec.width(), spec.height(), surface).ok_or(Reason::Busy)?;
        let geometry =
            Geometry::new(inner.width, inner.height, width, height).map_err(|_| Reason::Busy)?;
        self.lane.take();
        let latest = Arc::new(Mutex::new(Latest {
            serial: 1,
            phase: Phase::Opening,
            title: spec.title(),
            projection: None,
            geometry,
        }));
        let resize = Arc::new(Mutex::new(geometry));
        let closing = Arc::new(AtomicBool::new(false));
        let stopping = Arc::new(AtomicBool::new(false));
        let (sender, commands) = mpsc::sync_channel(16);
        let (reader, wake) = LocalStream::pair().map_err(|_| Reason::Busy)?;
        let output = Arc::clone(&latest);
        let desired = Arc::clone(&resize);
        let close = Arc::clone(&closing);
        let stop = Arc::clone(&stopping);
        let clock = self.clock.clone();
        let launch_id = id.clone();
        let launch_spec = spec.clone();
        let task = BackgroundThread::spawn("orchbee-independent-popup", move || {
            let mut workbench_client = PopupRunner {
                session,
                id: launch_id,
                clock,
                wake: reader,
                output,
                desired,
                close,
                stop,
                commands,
            };
            if let Err(reason) = workbench_client.run(origin, launch_spec, geometry) {
                workbench_client.publish(Phase::Failed(reason), None, None);
            }
        })
        .map_err(|_| Reason::Busy)?;
        self.latest = latest.lock().ok().map(|latest| latest.clone());
        self.id = Some(id);
        self.spec = Some(spec);
        self.close_had_popup = false;
        self.last_mouse_cell = None;
        self.mouse_pressed = false;
        self.lane = Some(Lane {
            sender: Some(sender),
            latest,
            resize,
            closing,
            stopping,
            wake,
            task,
        });
        Ok(())
    }
    pub(crate) fn begin_close(&mut self) -> bool {
        self.close_had_popup = self.captured();
        if let Some(lane) = &self.lane
            && self.captured()
        {
            lane.closing.store(true, Ordering::Release);
            if let Some(latest) = &mut self.latest {
                latest.phase = Phase::Closing;
            }
            return true;
        }
        false
    }
    pub(crate) fn activate_drawn_close(&mut self, target: &DrawnPopupClose) -> bool {
        if self.identity() != Some(target.id())
            || !self
                .latest
                .as_ref()
                .is_some_and(|latest| matches!(latest.phase, Phase::Opening | Phase::Active))
        {
            return false;
        }
        self.begin_close()
    }
    pub(crate) fn tick(&mut self) -> bool {
        let Some(lane) = &self.lane else {
            return false;
        };
        let Ok(latest) = lane.latest.try_lock() else {
            return false;
        };
        if self
            .latest
            .as_ref()
            .is_some_and(|current| current.serial == latest.serial)
        {
            return false;
        }
        let mut current = latest.clone();
        if lane.closing.load(Ordering::Acquire)
            && matches!(current.phase, Phase::Opening | Phase::Active)
        {
            current.phase = Phase::Closing;
        }
        self.latest = Some(current);
        true
    }
    pub(crate) fn completion(
        &self,
        operation: &WorkbenchRequestId,
        open: bool,
    ) -> Option<ClientViewCompletionOutcome> {
        let Some(latest) = &self.latest else {
            return (!open)
                .then_some(ClientViewCompletionOutcome::PopupClosed { had_popup: false });
        };
        match latest.phase {
            Phase::Active
                if open
                    && self.id.as_ref() == Some(operation)
                    && latest
                        .projection
                        .as_ref()
                        .is_some_and(|p| p.frame().is_some()) =>
            {
                Some(ClientViewCompletionOutcome::PopupOpened {
                    popup_id: operation.clone(),
                })
            }
            Phase::Closed if !open => Some(ClientViewCompletionOutcome::PopupClosed {
                had_popup: self.close_had_popup,
            }),
            Phase::Closed if open => Some(ClientViewCompletionOutcome::Incomplete {
                reason: Reason::Interrupted,
            }),
            Phase::Failed(reason) | Phase::Refused(reason) => {
                Some(ClientViewCompletionOutcome::Incomplete { reason })
            }
            _ => None,
        }
    }
    pub(crate) fn view(&self) -> Option<PopupView<'_>> {
        if !self.captured() {
            return None;
        }
        let latest = self.latest.as_ref()?;
        let spec = self.spec.as_ref()?;
        Some(PopupView {
            id: self.id.as_ref()?,
            title: &latest.title,
            projection: self
                .input_ready()
                .then_some(latest.projection.as_deref())
                .flatten(),
            width: spec.width(),
            height: spec.height(),
            closing: latest.phase == Phase::Closing,
        })
    }
    pub(crate) fn input(&mut self, id: &WorkbenchRequestId, item: &DecodedItem) {
        if self.identity() != Some(id) || !self.input_ready() {
            return;
        }
        let input = match &item.kind {
            DecodedKind::Paste { text } if !text.is_empty() => {
                PasteContent::new(text.as_bytes().to_vec())
                    .ok()
                    .map(Input::Paste)
            }
            DecodedKind::Key(key) if key.kind != crate::client_view::KeyKind::Release => {
                ControlInput::new(item.raw.to_vec()).ok().map(Input::Bytes)
            }
            DecodedKind::Nondecision => ControlInput::new(item.raw.to_vec()).ok().map(Input::Bytes),
            _ => None,
        };
        if let Some(input) = input {
            self.enqueue(input);
        }
    }
    fn input_ready(&self) -> bool {
        let Some(latest) = &self.latest else {
            return false;
        };
        if self
            .lane
            .as_ref()
            .is_none_or(|lane| lane.closing.load(Ordering::Acquire))
            || latest.phase != Phase::Active
            || latest
                .projection
                .as_ref()
                .is_none_or(|p| p.frame().is_none())
        {
            return false;
        }
        let Some((surface, width, height)) = self.surface else {
            return false;
        };
        self.spec
            .as_ref()
            .and_then(|spec| resolve_geometry(spec.width(), spec.height(), surface))
            .and_then(|(_, inner)| Geometry::new(inner.width, inner.height, width, height).ok())
            == Some(latest.geometry)
    }
    fn enqueue(&mut self, input: Input) {
        if !self
            .lane
            .as_ref()
            .and_then(|lane| lane.sender.as_ref())
            .is_some_and(|sender| sender.try_send(input).is_ok())
        {
            self.begin_close();
        }
    }
    pub(crate) fn mouse(&mut self, id: &WorkbenchRequestId, content: Rect, event: MouseEvent) {
        if self.id.as_ref() != Some(id) {
            return;
        }
        if !self.input_ready() {
            return;
        }
        let Some((surface, _, _)) = self.surface else {
            return;
        };
        let Some((_, inner)) = self
            .spec
            .as_ref()
            .and_then(|spec| resolve_geometry(spec.width(), spec.height(), surface))
        else {
            return;
        };
        if content != inner {
            return;
        }
        let Some(frame) = self
            .latest
            .as_ref()
            .and_then(|latest| latest.projection.as_ref())
            .and_then(|projection| projection.frame())
        else {
            return;
        };
        let tracking = frame.interaction.mouse_tracking();
        let format = frame.interaction.mouse_format();
        let alternate = frame.alternate_screen;
        let application = frame.interaction.application_cursor_keys();
        let local = PaneMouseEvent {
            action: event.action,
            button: Some(event.button),
            mods: event.mods,
            column: i32::from(event.column) - i32::from(content.x),
            row: i32::from(event.row) - i32::from(content.y),
        };
        if let PaneEncode::Bytes(bytes) = encode_pane_report(
            &local,
            tracking,
            format,
            (content.width, content.height),
            self.mouse_pressed,
            &mut self.last_mouse_cell,
        ) {
            if let Ok(bytes) = ControlInput::new(bytes) {
                self.enqueue(Input::Bytes(bytes));
            }
        } else if tracking == MouseTracking::None
            && alternate
            && event.action == MouseAction::Press
            && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
            && let Ok(bytes) = ControlInput::new(encode_cursor_arrows(
                event.button == MouseButton::WheelUp,
                3,
                application,
            ))
        {
            self.enqueue(Input::Bytes(bytes));
        }
        if matches!(
            event.button,
            MouseButton::Left | MouseButton::Middle | MouseButton::Right | MouseButton::Extended(_)
        ) {
            self.mouse_pressed = event.action != MouseAction::Release;
        }
    }
}

struct PopupRunner {
    session: SessionId,
    id: WorkbenchRequestId,
    clock: LocalOperationClock,
    wake: LocalStream,
    output: Arc<Mutex<Latest>>,
    desired: Arc<Mutex<Geometry>>,
    close: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    commands: Receiver<Input>,
}
impl PopupRunner {
    fn deadline(&self) -> Instant {
        self.clock.now() + Duration::from_secs(2)
    }
    fn publish(
        &self,
        phase: Phase,
        title: Option<String>,
        projection: Option<(TerminalProjection, Geometry)>,
    ) {
        if let Ok(mut latest) = self.output.lock() {
            latest.serial = latest.serial.saturating_add(1);
            latest.phase = phase;
            if let Some(title) = title {
                latest.title = title;
            }
            if let Some((projection, geometry)) = projection {
                latest.projection = Some(Arc::new(projection));
                latest.geometry = geometry;
            }
            if matches!(phase, Phase::Closed | Phase::Refused(_) | Phase::Failed(_)) {
                latest.projection = None;
            }
        }
    }
    fn plugin(
        &self,
        client: &mut LocalClient,
        action: PluginAction,
    ) -> Result<PluginOutcome, Reason> {
        let deadline = self.clock.now()
            + if matches!(action, PluginAction::PopupOpen { .. }) {
                Duration::from_secs(20)
            } else {
                Duration::from_secs(2)
            };
        client
            .plugin(
                PluginRequest {
                    protocol_version: ProtocolVersion,
                    session_id: self.session.clone(),
                    request_id: request_id()?,
                    action,
                },
                deadline,
                Some(self.wake.as_local_io()),
            )
            .map(|result| result.outcome)
            .map_err(|_| Reason::ProjectionUnavailable)
    }
    fn viewer(
        &self,
        client: &mut LocalClient,
        action: TerminalViewerAction,
    ) -> Result<TerminalViewerOutcome, Reason> {
        client
            .submit_terminal_viewer_cancellable(
                TerminalViewerRequest::new(client_id()?, action),
                Some(self.deadline()),
                Some(self.wake.as_local_io()),
            )
            .map(|result| result.outcome().clone())
            .map_err(|_| Reason::ProjectionUnavailable)
    }
    fn control(
        &self,
        client: &mut LocalClient,
        action: TerminalControlAction,
    ) -> Result<TerminalControlOutcome, Reason> {
        client
            .submit_terminal_control_cancellable(
                TerminalControlRequest::new(client_id()?, action),
                Some(self.deadline()),
                Some(self.wake.as_local_io()),
            )
            .map(|result| result.outcome().clone())
            .map_err(|_| Reason::Interrupted)
    }
    fn run(
        &mut self,
        origin: LocalClientOrigin,
        spec: PopupSpec,
        mut geometry: Geometry,
    ) -> Result<(), Reason> {
        let mut client = origin
            .lane_before_cancellable(
                self.deadline(),
                self.clock.clone(),
                Some(self.wake.as_local_io()),
            )
            .map_err(|_| Reason::ProjectionUnavailable)?;
        if self.stop.load(Ordering::Acquire) {
            return Err(Reason::Interrupted);
        }
        let launch_geometry = WorkbenchTerminalGeometry::new(
            geometry.columns(),
            geometry.rows(),
            geometry.cell_pixel_width(),
            geometry.cell_pixel_height(),
        )
        .map_err(|_| Reason::Busy)?;
        let kind = spec.kind();
        let opened = match spec {
            PopupSpec::Configured(spec) => self.configured(
                &mut client,
                orchbee_workbench_protocol::ConfiguredCommandAction::PopupOpen {
                    popup_id: self.id.clone(),
                    spec,
                    geometry: launch_geometry,
                },
            ),
            PopupSpec::Plugin(spec) => self
                .plugin(
                    &mut client,
                    PluginAction::PopupOpen {
                        popup_id: self.id.clone(),
                        spec,
                        geometry: launch_geometry,
                    },
                )
                .and_then(PopupOutcome::plugin),
            PopupSpec::Editor(spec) => self.open_editor(&mut client, spec, launch_geometry),
        }?;
        let title = match opened {
            PopupOutcome::Opened(title) => title,
            PopupOutcome::Refused(reason) => {
                self.publish(Phase::Refused(reason), None, None);
                return Ok(());
            }
            PopupOutcome::Closed => {
                self.publish(Phase::Closed, None, None);
                return Ok(());
            }
            _ => return Err(Reason::Interrupted),
        };
        let TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
        } = self.viewer(
            &mut client,
            TerminalViewerAction::Attach {
                terminal_create_request_id: RequestId::parse(self.id.as_str())
                    .map_err(|_| Reason::TargetUnavailable)?,
            },
        )?
        else {
            return Err(Reason::ProjectionUnavailable);
        };
        let mut projection = TerminalProjection::new(
            AttachmentId::parse(attachment_id.as_str())
                .map_err(|_| Reason::ProjectionUnavailable)?,
            HostEpoch::parse(host_epoch.as_str()).map_err(|_| Reason::ProjectionUnavailable)?,
        )
        .with_host_terminal(host_terminal_id);
        let TerminalControlOutcome::Acquired { controller_id } = self.control(
            &mut client,
            TerminalControlAction::Acquire {
                attachment_id: attachment_id.clone(),
            },
        )?
        else {
            return Err(Reason::TargetUnavailable);
        };
        self.publish(Phase::Opening, Some(title), None);
        let mut sequence = 1u64;
        // Child process completion is currently exposed only by the Host status read.
        let mut observe_at = self.clock.now() + Duration::from_secs(1);
        while !self.close.load(Ordering::Acquire) {
            if self.stop.load(Ordering::Acquire) {
                return Err(Reason::Interrupted);
            }
            let desired = *self.desired.lock().map_err(|_| Reason::Busy)?;
            let next_sequence = |sequence: &mut u64| {
                let current = NonZeroU64::new(*sequence).ok_or(Reason::Interrupted)?;
                *sequence = sequence.checked_add(1).ok_or(Reason::Interrupted)?;
                Ok::<_, Reason>(current)
            };
            if desired != geometry {
                let action = TerminalControlAction::Resize {
                    attachment_id: attachment_id.clone(),
                    controller_id: controller_id.clone(),
                    sequence: next_sequence(&mut sequence)?,
                    geometry: desired,
                };
                if !matches!(self.control(&mut client, action)?, TerminalControlOutcome::Resized { geometry: accepted } if accepted == desired)
                {
                    return Err(Reason::Interrupted);
                }
                geometry = desired;
                projection.clear_frame();
                self.publish(Phase::Opening, None, Some((projection.clone(), geometry)));
            }
            // One queued input per turn, so projection and close are never starved.
            if let Ok(input) = self.commands.try_recv() {
                let action = match input {
                    Input::Bytes(bytes) => TerminalControlAction::Input {
                        attachment_id: attachment_id.clone(),
                        controller_id: controller_id.clone(),
                        sequence: next_sequence(&mut sequence)?,
                        bytes,
                    },
                    Input::Paste(content) => TerminalControlAction::Paste {
                        attachment_id: attachment_id.clone(),
                        controller_id: controller_id.clone(),
                        sequence: next_sequence(&mut sequence)?,
                        content,
                    },
                };
                if !matches!(
                    self.control(&mut client, action)?,
                    TerminalControlOutcome::InputAccepted {}
                        | TerminalControlOutcome::Pasted { .. }
                ) {
                    return Err(Reason::Interrupted);
                }
            }
            match self.viewer(
                &mut client,
                TerminalViewerAction::Poll {
                    attachment_id: attachment_id.clone(),
                },
            )? {
                TerminalViewerOutcome::Record(record) => {
                    let feedback = projection
                        .receive_for_geometry(&record, Some(geometry))
                        .map_err(|_| Reason::ProjectionUnavailable)?;
                    if let Some(feedback) = feedback
                        && !matches!(
                            self.viewer(&mut client, feedback)?,
                            TerminalViewerOutcome::FeedbackAccepted {}
                        )
                    {
                        return Err(Reason::ProjectionUnavailable);
                    }
                    self.publish(
                        if projection.frame().is_some() {
                            Phase::Active
                        } else {
                            Phase::Opening
                        },
                        None,
                        Some((projection.clone(), geometry)),
                    );
                }
                TerminalViewerOutcome::Empty {} => {}
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::FeedbackAccepted { .. }
                | TerminalViewerOutcome::Detached { .. }
                | TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::RowsRejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => {
                    return Err(Reason::ProjectionUnavailable);
                }
            }
            if self.clock.now() >= observe_at {
                match self.observe_popup(&mut client, kind, false)? {
                    PopupOutcome::State(true) | PopupOutcome::Closed => {
                        self.close.store(true, Ordering::Release)
                    }
                    PopupOutcome::State(false) => {}
                    _ => return Err(Reason::Interrupted),
                }
                observe_at = self.clock.now() + Duration::from_secs(1);
            }
            // The Host viewer protocol supplies pull frames and has no output-ready event.
            thread::sleep(Duration::from_millis(20));
        }
        self.publish(Phase::Closing, None, None);
        // Only a known Detach lets the same connection continue toward an acknowledged Close.
        if !matches!(
            self.viewer(&mut client, TerminalViewerAction::Detach { attachment_id })?,
            TerminalViewerOutcome::Detached {}
        ) {
            return Err(Reason::Interrupted);
        }
        match self.observe_popup(&mut client, kind, true)? {
            PopupOutcome::Closed => {}
            PopupOutcome::Closing => {
                let deadline = self.clock.now() + Duration::from_secs(30);
                loop {
                    if self.stop.load(Ordering::Acquire) || self.clock.now() >= deadline {
                        return Err(Reason::Expired);
                    }
                    match self.observe_popup(&mut client, kind, false)? {
                        PopupOutcome::Closed => break,
                        // Host termination completion has no event source in this protocol.
                        PopupOutcome::State(_) => thread::sleep(Duration::from_millis(100)),
                        _ => return Err(Reason::Interrupted),
                    }
                }
            }
            _ => return Err(Reason::Interrupted),
        }
        self.publish(Phase::Closed, None, None);
        Ok(())
    }
}
fn request_id() -> Result<WorkbenchRequestId, Reason> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Reason::Busy)?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Reason::Busy)
}
fn client_id() -> Result<RequestId, Reason> {
    RequestId::parse(request_id()?.as_str()).map_err(|_| Reason::Busy)
}

#[cfg(test)]
mod tests;
