//! One bounded Windows graphics surface shares the fixed backend's frame and restoration lease.
use crate::{
    terminal_graphics::{Facts, Reply},
    terminal_lifecycle::{SynchronizedUpdateLease, graphics::delete},
    workbench_client::{PaneGraphicsCanvas, PaneGraphicsDraw},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use orchbee_workbench_protocol::{SessionId, WorkbenchRevision};
use ratatui::layout::{Position, Rect};
use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
mod geometry;
use geometry::{CANVAS_BOUND, Scene};
const IMAGE_COUNT: usize = 64;
const CHUNKS_PER_FRAME: usize = 32;
const RAW_CHUNK: usize = 3072;
const REPLY_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub(crate) struct Surface(Rc<RefCell<State>>);
pub(crate) struct Choice {
    pub(crate) revision: Option<WorkbenchRevision>,
}
struct State {
    lease: Arc<SynchronizedUpdateLease>,
    probe_id: u32,
    next_id: u64,
    facts: Facts,
    now: Instant,
    disabled: bool,
    frame_allowed: bool,
    replan: bool,
    staged: Vec<Scene>,
    active: Vec<Ready>,
    candidate: Option<Candidate>,
    retired: Vec<u32>,
    cursor: usize,
}
struct Ready {
    scene: Scene,
    images: Vec<PlacedImage>,
}
struct PlacedImage {
    id: u32,
    state: Placement,
}
enum Placement {
    Unsent,
    Awaiting(Instant),
    Confirmed,
}
struct Candidate {
    scene: Scene,
    images: Vec<Upload>,
}
struct Upload {
    id: u32,
    state: Transfer,
}
enum Transfer {
    Sending(usize),
    Awaiting(Instant),
    Ready,
}

impl Surface {
    pub(crate) fn invalidate(&self) {
        let mut state = self.0.borrow_mut();
        state.cancel_candidate();
        let active = std::mem::take(&mut state.active);
        state.retired.extend(
            active
                .into_iter()
                .flat_map(|ready| ready.images.into_iter().map(|image| image.id)),
        );
        state.staged.clear();
    }
    pub(crate) fn new(probe_id: u32, lease: Arc<SynchronizedUpdateLease>) -> Self {
        Self(Rc::new(RefCell::new(State {
            lease,
            probe_id,
            next_id: 1,
            facts: Facts::default(),
            now: Instant::now(),
            disabled: false,
            frame_allowed: false,
            replan: false,
            staged: Vec::new(),
            active: Vec::new(),
            candidate: None,
            retired: Vec::new(),
            cursor: 0,
        })))
    }
    pub(crate) fn update(
        &self,
        facts: Facts,
        now: Instant,
        replies: Vec<Reply>,
        overflow: bool,
    ) -> bool {
        let mut state = self.0.borrow_mut();
        let mut changed = state.facts != facts;
        state.facts = facts;
        state.now = now;
        if overflow {
            state.fail();
            return true;
        }
        for reply in replies {
            changed |= state.reply(reply);
        }
        if state.timed_out() {
            state.fail();
            changed = true;
        }
        if state.candidate.as_ref().is_some_and(|candidate| {
            candidate
                .images
                .iter()
                .all(|image| matches!(image.state, Transfer::Ready))
        }) {
            state.promote();
            changed = true;
        }
        changed || state.needs_frame()
    }
    pub(crate) fn begin_frame(&self, covered: bool) {
        let mut state = self.0.borrow_mut();
        state.replan = false;
        state.staged.clear();
        state.frame_allowed =
            !covered && !state.disabled && state.facts.kitty && state.facts.measurement.is_some();
    }
    pub(crate) fn cover(&self) {
        let mut state = self.0.borrow_mut();
        state.frame_allowed = false;
        state.staged.clear();
    }
    pub(crate) fn select(
        &self,
        session: &SessionId,
        canvas: &PaneGraphicsCanvas,
        body: Rect,
    ) -> Option<Choice> {
        let mut state = self.0.borrow_mut();
        if !state.frame_allowed {
            return None;
        }
        let cells = state.facts.cell_capabilities().cell_pixels?;
        let scene = Scene::new(session, canvas, body, cells)?;
        if state.staged.len() >= IMAGE_COUNT
            || scene.images.len()
                + state
                    .staged
                    .iter()
                    .map(|scene| scene.images.len())
                    .sum::<usize>()
                > IMAGE_COUNT
            || scene.canvas.bytes()
                + state
                    .staged
                    .iter()
                    .map(|scene| scene.canvas.bytes())
                    .sum::<usize>()
                > CANVAS_BOUND
            || scene.visible_bytes
                + state
                    .staged
                    .iter()
                    .map(|scene| scene.visible_bytes)
                    .sum::<usize>()
                > CANVAS_BOUND
        {
            return None;
        }
        let choice = state
            .active
            .iter_mut()
            .find(|ready| ready.scene.scope_matches(&scene))
            .map(|ready| {
                if ready.scene.canvas.same_content(&scene.canvas) {
                    ready.scene.canvas = scene.canvas.clone();
                }
                Choice {
                    revision: ready
                        .images
                        .iter()
                        .all(|image| matches!(image.state, Placement::Confirmed))
                        .then(|| ready.scene.canvas.revision()),
                }
            });
        state.staged.push(scene);
        choice
    }
    /// Called inside the render closure, before the backend's ordinary flush.
    pub(crate) fn finish_frame(&self, draws: &[PaneGraphicsDraw], covered: bool) {
        let mut state = self.0.borrow_mut();
        if covered {
            state.staged.clear();
        } else {
            state
                .staged
                .retain(|scene| draws.iter().any(|draw| scene.canvas.matches_draw(draw)));
        }
        let mut active = std::mem::take(&mut state.active);
        active.retain(|ready| {
            if state
                .staged
                .iter()
                .any(|scene| scene.scope_matches(&ready.scene))
            {
                true
            } else {
                state
                    .retired
                    .extend(ready.images.iter().map(|image| image.id));
                false
            }
        });
        state.active = active;
        if state.candidate.as_ref().is_some_and(|candidate| {
            !state
                .staged
                .iter()
                .any(|scene| scene.scope_matches(&candidate.scene))
        }) {
            state.cancel_candidate();
        }
        // A delete aborts an incomplete Kitty transfer; cancel explicitly before emitting it.
        if !state.retired.is_empty() {
            state.cancel_candidate();
        }
        if state.candidate.is_none() && state.frame_allowed {
            state.start_candidate();
        }
    }
    pub(crate) fn write_frame(
        &self,
        writer: &mut impl Write,
        cursor: Position,
    ) -> io::Result<Vec<u32>> {
        let mut state = self.0.borrow_mut();
        if state.lease.frame_output_blocked() {
            return Ok(Vec::new());
        }
        state.retired.sort_unstable();
        state.retired.dedup();
        for id in &state.retired {
            state.write_control(writer, &delete(*id))?;
        }
        let deleted = std::mem::take(&mut state.retired);
        let lease = Arc::clone(&state.lease);
        let now = state.now;
        let transfer_open = state.candidate.as_ref().is_some_and(|candidate| {
            candidate
                .images
                .iter()
                .any(|image| matches!(image.state, Transfer::Sending(offset) if offset > 0))
        });
        if !transfer_open {
            for ready in &mut state.active {
                for (image, plan) in ready.images.iter_mut().zip(&ready.scene.images) {
                    if !matches!(image.state, Placement::Unsent) {
                        continue;
                    }
                    let rect = plan.visible;
                    let z = ready.scene.canvas.layers()[plan.index].metadata().z_index;
                    lease.graphics().control_started();
                    write!(
                        writer,
                        "\x1b[{};{}H\x1b_Ga=p,i={},p=1,c={},r={},z={z},C=1,q=0;\x1b\\\x1b[{};{}H",
                        u32::from(rect.y) + 1,
                        u32::from(rect.x) + 1,
                        image.id,
                        rect.width,
                        rect.height,
                        u32::from(cursor.y) + 1,
                        u32::from(cursor.x) + 1
                    )?;
                    image.state = Placement::Awaiting(now + REPLY_DEADLINE);
                }
            }
        }
        if let Some(candidate) = &mut state.candidate {
            let mut chunks = CHUNKS_PER_FRAME;
            for (image, plan) in candidate.images.iter_mut().zip(&candidate.scene.images) {
                let Transfer::Sending(mut offset) = image.state else {
                    continue;
                };
                while chunks > 0 && offset < plan.bytes {
                    let count = RAW_CHUNK.min(plan.bytes - offset);
                    let mut raw = [0; RAW_CHUNK];
                    plan.fill(&candidate.scene.canvas, offset, &mut raw[..count]);
                    let payload = STANDARD.encode(&raw[..count]);
                    let more = u8::from(offset + count < plan.bytes);
                    lease.graphics().control_started();
                    if offset == 0 {
                        write!(
                            writer,
                            "\x1b_Ga=t,t=d,f=32,s={},v={},i={},q=0,m={more};{payload}\x1b\\",
                            plan.width, plan.height, image.id
                        )?;
                    } else {
                        write!(writer, "\x1b_Gm={more};{payload}\x1b\\")?;
                    }
                    offset += count;
                    chunks -= 1;
                    image.state = if offset == plan.bytes {
                        Transfer::Awaiting(now + REPLY_DEADLINE)
                    } else {
                        Transfer::Sending(offset)
                    };
                }
                if chunks == 0 {
                    break;
                }
            }
        }
        Ok(deleted)
    }
    pub(crate) fn flushed(&self, deleted: &[u32]) {
        let mut state = self.0.borrow_mut();
        state.lease.graphics().flushed(deleted);
        state.replan = !state.disabled
            && state.candidate.is_none()
            && state
                .staged
                .iter()
                .any(|scene| !state.active.iter().any(|ready| ready.scene.same(scene)));
        state.staged.clear();
    }
}

impl State {
    fn write_control(&self, writer: &mut impl Write, sequence: &str) -> io::Result<()> {
        self.lease.graphics().control_started();
        writer.write_all(sequence.as_bytes())
    }
    fn cancel_candidate(&mut self) {
        if let Some(candidate) = self.candidate.take() {
            self.retired
                .extend(candidate.images.into_iter().map(|image| image.id));
        }
    }
    fn fail(&mut self) {
        self.disabled = true;
        self.frame_allowed = false;
        self.cancel_candidate();
        for ready in self.active.drain(..) {
            self.retired
                .extend(ready.images.into_iter().map(|image| image.id));
        }
    }
    fn needs_frame(&self) -> bool {
        !self.retired.is_empty()
            || self.active.iter().any(|ready| {
                ready
                    .images
                    .iter()
                    .any(|image| matches!(image.state, Placement::Unsent))
            })
            || self.candidate.as_ref().is_some_and(|candidate| {
                candidate
                    .images
                    .iter()
                    .any(|image| matches!(image.state, Transfer::Sending(_)))
            })
            || self.replan
    }
    fn timed_out(&self) -> bool {
        self.candidate.as_ref().is_some_and(|candidate| candidate.images.iter().any(|image| matches!(image.state, Transfer::Awaiting(deadline) if self.now >= deadline)))
            || self.active.iter().any(|ready| ready.images.iter().any(|image| matches!(image.state, Placement::Awaiting(deadline) if self.now >= deadline)))
    }
    fn reply(&mut self, reply: Reply) -> bool {
        let Reply::Kitty {
            image_id,
            placement_id,
            success,
        } = reply
        else {
            return false;
        };
        if placement_id.is_none()
            && let Some(image) = self.candidate.as_mut().and_then(|candidate| {
                candidate
                    .images
                    .iter_mut()
                    .find(|image| image.id == image_id)
            })
        {
            if matches!(image.state, Transfer::Sending(0)) {
                return false;
            }
            if !success {
                self.fail();
                return true;
            }
            if matches!(image.state, Transfer::Awaiting(deadline) if self.now < deadline) {
                image.state = Transfer::Ready;
                return true;
            }
        }
        if placement_id == Some(1)
            && let Some(image) = self
                .active
                .iter_mut()
                .flat_map(|ready| &mut ready.images)
                .find(|image| image.id == image_id)
        {
            if matches!(image.state, Placement::Unsent) {
                return false;
            }
            if !success {
                self.fail();
                return true;
            }
            if matches!(image.state, Placement::Awaiting(deadline) if self.now < deadline) {
                image.state = Placement::Confirmed;
                return true;
            }
        }
        false
    }
    fn start_candidate(&mut self) {
        let count = self.staged.len();
        if count == 0 {
            return;
        }
        let scene = (0..count)
            .map(|step| (self.cursor + step) % count)
            .find(|index| {
                !self
                    .active
                    .iter()
                    .any(|ready| ready.scene.same(&self.staged[*index]))
            })
            .map(|index| {
                self.cursor = (index + 1) % count;
                self.staged[index].clone()
            });
        let Some(scene) = scene else {
            return;
        };
        if self.lease.graphics().capacity() < scene.images.len() {
            return;
        }
        let mut images = Vec::with_capacity(scene.images.len());
        for _ in &scene.images {
            if self.next_id == u64::from(self.probe_id) {
                self.next_id += 1;
            }
            let Ok(id) = u32::try_from(self.next_id) else {
                self.retired
                    .extend(images.into_iter().map(|image: Upload| image.id));
                self.fail();
                return;
            };
            self.next_id += 1;
            if !self.lease.graphics().register(id) {
                self.retired
                    .extend(images.into_iter().map(|image: Upload| image.id));
                self.fail();
                return;
            }
            images.push(Upload {
                id,
                state: Transfer::Sending(0),
            });
        }
        self.candidate = Some(Candidate { scene, images });
    }
    fn promote(&mut self) {
        let Some(candidate) = self.candidate.take() else {
            return;
        };
        let mut active = std::mem::take(&mut self.active);
        active.retain(|ready| {
            if ready.scene.scope_matches(&candidate.scene) {
                self.retired
                    .extend(ready.images.iter().map(|image| image.id));
                false
            } else {
                true
            }
        });
        while !active.is_empty()
            && (candidate.scene.images.len()
                + active.iter().map(|ready| ready.images.len()).sum::<usize>()
                > IMAGE_COUNT
                || candidate.scene.canvas.bytes()
                    + active
                        .iter()
                        .map(|ready| ready.scene.canvas.bytes())
                        .sum::<usize>()
                    > CANVAS_BOUND
                || candidate.scene.visible_bytes
                    + active
                        .iter()
                        .map(|ready| ready.scene.visible_bytes)
                        .sum::<usize>()
                    > CANVAS_BOUND)
        {
            let removed = active.remove(0);
            self.retired
                .extend(removed.images.into_iter().map(|image| image.id));
        }
        active.push(Ready {
            scene: candidate.scene,
            images: candidate
                .images
                .into_iter()
                .map(|image| PlacedImage {
                    id: image.id,
                    state: Placement::Unsent,
                })
                .collect(),
        });
        self.active = active;
    }
}

#[cfg(test)]
mod tests;
