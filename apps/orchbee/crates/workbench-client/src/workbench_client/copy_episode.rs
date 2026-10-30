//! The Workbench copy-mode episode: the pane and admitted content source that keyboard copy is
//! bound to, its entry fence, and the one query whose outcome it still expects.
//!
//! The composition root holds the copy query driver and cancels it whenever an episode begins or
//! ends; this owner never reaches the driver.

use crate::{
    terminal_content::ContentSource,
    terminal_copy::TerminalTextPoint,
    terminal_copy_mode::{CopyModeStatus, CopyModeView, EpisodeId, Mode, Surface},
    terminal_copy_query::QueryIdentity,
    workbench_client::state::PaneRef,
};
use std::num::NonZeroU64;

/// A query the driver admitted, matched exactly once against its outcome.
#[derive(Clone)]
pub(super) struct PendingQuery {
    pub(super) identity: QueryIdentity,
    pub(super) source: ContentSource,
    pub(super) search: Option<Box<str>>,
}

/// The content fence an entering episode waits on.
#[derive(Clone)]
pub(super) struct Entry {
    fence: ContentSource,
    needs_viewport_enter: bool,
}

impl Entry {
    pub(super) const fn fence(&self) -> &ContentSource {
        &self.fence
    }

    pub(super) const fn needs_viewport_enter(&self) -> bool {
        self.needs_viewport_enter
    }
}

enum EpisodeState {
    Entering(Entry),
    Active(Mode),
}

struct Episode {
    key: PaneRef,
    id: EpisodeId,
    source: Option<ContentSource>,
    state: EpisodeState,
    pending_query: Option<PendingQuery>,
}

/// The single copy-mode episode slot and its identity sequence.
#[derive(Default)]
pub(super) struct CopyModeEpisodes {
    next_episode: u64,
    episode: Option<Episode>,
    surface_pending: bool,
}

impl CopyModeEpisodes {
    /// Ends any episode without cleanup, as connection close does.
    pub(super) fn close(&mut self) {
        self.episode = None;
        self.surface_pending = false;
    }

    fn next_episode(&mut self) -> Option<EpisodeId> {
        self.next_episode = self.next_episode.checked_add(1)?;
        NonZeroU64::new(self.next_episode).map(EpisodeId::new)
    }

    /// Begins entering copy mode against one content fence; `None` means the identity
    /// sequence is exhausted and nothing changed.
    pub(super) fn begin_entering(
        &mut self,
        key: PaneRef,
        fence: ContentSource,
        needs_viewport_enter: bool,
    ) -> Option<EpisodeId> {
        let id = self.next_episode()?;
        self.surface_pending = false;
        self.episode = Some(Episode {
            key,
            id,
            source: None,
            state: EpisodeState::Entering(Entry {
                fence,
                needs_viewport_enter,
            }),
            pending_query: None,
        });
        Some(id)
    }

    /// Records that the entering episode's history viewport was entered.
    pub(super) fn viewport_entered(&mut self) {
        if let Some(Episode {
            state: EpisodeState::Entering(entry),
            ..
        }) = self.episode.as_mut()
        {
            entry.needs_viewport_enter = false;
        }
    }

    pub(super) fn activate(
        &mut self,
        source: ContentSource,
        surface: Surface,
        cursor: TerminalTextPoint,
    ) -> bool {
        let Some(episode) = self.episode.as_mut() else {
            return false;
        };
        let Some(mode) = Mode::new(episode.id, surface, cursor) else {
            return false;
        };
        episode.source = Some(source);
        episode.state = EpisodeState::Active(mode);
        self.surface_pending = false;
        true
    }

    pub(super) const fn active(&self) -> bool {
        self.episode.is_some()
    }

    pub(super) fn key(&self) -> Option<&PaneRef> {
        self.episode.as_ref().map(|episode| &episode.key)
    }

    pub(super) fn id(&self) -> Option<EpisodeId> {
        self.episode.as_ref().map(|episode| match &episode.state {
            EpisodeState::Entering(_) => episode.id,
            EpisodeState::Active(mode) => mode.episode(),
        })
    }

    pub(super) fn mode(&self) -> Option<&Mode> {
        match &self.episode.as_ref()?.state {
            EpisodeState::Entering(_) => None,
            EpisodeState::Active(mode) => Some(mode),
        }
    }

    pub(super) fn mode_mut(&mut self) -> Option<&mut Mode> {
        match &mut self.episode.as_mut()?.state {
            EpisodeState::Entering(_) => None,
            EpisodeState::Active(mode) => Some(mode),
        }
    }

    pub(super) fn entry(&self) -> Option<&Entry> {
        match &self.episode.as_ref()?.state {
            EpisodeState::Entering(entry) => Some(entry),
            EpisodeState::Active(_) => None,
        }
    }

    /// The content source the active episode last admitted.
    pub(super) fn source(&self) -> Option<&ContentSource> {
        self.episode.as_ref()?.source.as_ref()
    }

    pub(super) fn set_source(&mut self, source: ContentSource) {
        if let Some(episode) = self.episode.as_mut() {
            episode.source = Some(source);
        }
    }

    pub(super) fn set_pending_query(&mut self, pending: PendingQuery) {
        if let Some(episode) = self.episode.as_mut() {
            episode.pending_query = Some(pending);
        }
    }

    pub(super) fn take_pending_query(&mut self) -> Option<PendingQuery> {
        self.episode.as_mut()?.pending_query.take()
    }

    pub(super) fn view(&self) -> Option<CopyModeView<'_>> {
        let episode = self.episode.as_ref()?;
        let mut view = match &episode.state {
            EpisodeState::Entering(_) => CopyModeView::entering(episode.id),
            EpisodeState::Active(mode) => mode.view(),
        };
        if self.surface_pending && matches!(&episode.state, EpisodeState::Active(_)) {
            view.status = CopyModeStatus::Moving;
            view.cursor = None;
            if let Some(token) = &mut view.pointer {
                token.pending();
            }
        }
        Some(view)
    }

    pub(super) const fn surface_pending(&self) -> bool {
        self.episode.is_some() && self.surface_pending
    }

    pub(super) fn set_surface_pending(&mut self, pending: bool) -> bool {
        if self.episode.is_none() {
            return false;
        }
        let changed = self.surface_pending != pending;
        self.surface_pending = pending;
        changed
    }

    /// Ends the episode; the caller cancels the query driver with it.
    pub(super) fn cancel_episode(&mut self) -> Option<PaneRef> {
        self.surface_pending = false;
        self.episode.take().map(|episode| episode.key)
    }

    pub(super) fn effect_current(
        &self,
        key: &PaneRef,
        generation: NonZeroU64,
        export: NonZeroU64,
        episode: NonZeroU64,
    ) -> bool {
        self.key() == Some(key)
            && self.id().is_some_and(|id| id.get() == episode)
            && self.mode().is_some_and(|mode| {
                mode.surface().generation == generation && mode.clipboard_identity() == Some(export)
            })
    }
}

#[cfg(test)]
impl CopyModeEpisodes {
    /// An active episode for identity checks that need no entry fence.
    pub(super) fn active_for_test(key: PaneRef, mode: Mode) -> Self {
        Self {
            next_episode: 0,
            episode: Some(Episode {
                key,
                id: mode.episode(),
                source: None,
                state: EpisodeState::Active(mode),
                pending_query: None,
            }),
            surface_pending: false,
        }
    }

    /// The episode's query slot, None without an episode.
    pub(super) fn pending_query_slot(&self) -> Option<Option<&PendingQuery>> {
        self.episode
            .as_ref()
            .map(|episode| episode.pending_query.as_ref())
    }
}
