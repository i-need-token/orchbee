//! Ephemeral canonical Presentation layers with one connection-owned producer and revision-pinned pixels.
use crate::semantic_events::semantic_pane;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_CHUNK_BYTES, MAXIMUM_GRAPHICS_LAYERS, MAXIMUM_GRAPHICS_LAYERS_PER_PANE,
    MAXIMUM_GRAPHICS_RESIDENT_BYTES, PaneGraphicsFailure as Failure, PaneGraphicsLayer,
    PaneGraphicsOutcome as Outcome, PaneGraphicsTarget, SemanticEventSource, SemanticPane,
    Snapshot, WorkbenchRequestId, WorkbenchRevision,
};
use std::{collections::BTreeMap, num::NonZeroU64};
mod image;
pub use image::DecodedGraphicsImage;
type Key = (String, String, String);
#[derive(Clone, Debug)]
struct Producer {
    connection: NonZeroU64,
    id: WorkbenchRequestId,
    sequence: u64,
    file_directory: Option<String>,
}
#[derive(Debug)]
struct Layer {
    source: SemanticPane,
    z: i32,
    image: Option<DecodedGraphicsImage>,
    resource_revision: WorkbenchRevision,
    producer: Option<Producer>,
}
/// The sole Site layer owner; callers serialize preparation and revalidate canonical membership at commit.
#[derive(Debug, Default)]
pub struct PaneGraphicsAuthority {
    revision: u64,
    exhausted: bool,
    bytes: usize,
    layers: BTreeMap<Key, Layer>,
}
/// One complete, source-admitted frame and its exact connection/sequence identity.
pub struct PaneGraphicsFrameCommit<'a> {
    pub connection: NonZeroU64,
    pub stream_id: &'a WorkbenchRequestId,
    pub source: &'a SemanticPane,
    pub sequence: WorkbenchRevision,
    pub frame_revision: WorkbenchRevision,
    pub image: DecodedGraphicsImage,
}
/// Immutable producer intent; platform preparation cannot change its canonical source or identity.
pub struct PaneGraphicsOpen<'a> {
    pub connection: NonZeroU64,
    pub stream_id: &'a WorkbenchRequestId,
    pub target: &'a PaneGraphicsTarget,
    pub layer_id: &'a str,
    pub z_index: i32,
}
/// Pure admission result, before any file allocation or replacement of an existing image.
pub enum PaneGraphicsOpenPreparation {
    Existing(Outcome),
    Ready(SemanticPane),
}
impl PaneGraphicsAuthority {
    /// Installed Site revision; exhausted owners cannot publish renderer facts.
    pub fn revision(&self) -> Result<WorkbenchRevision, Failure> {
        if self.exhausted {
            Err(Failure::RevisionExhausted)
        } else {
            Ok(WorkbenchRevision::new(self.revision))
        }
    }
    /// Resolves a selector to the actual current Presentation, including primary selection.
    pub fn resolve(
        snapshot: &Snapshot,
        target: &PaneGraphicsTarget,
    ) -> Result<SemanticPane, Failure> {
        semantic_pane(
            snapshot,
            &SemanticEventSource {
                panel_id: target.panel_id.clone(),
                presentation_id: target.presentation_id.clone(),
            },
        )
        .ok_or(Failure::NotFound)
    }
    /// Returns current metadata; no stored pixels are copied.
    pub fn catalog(
        &mut self,
        snapshot: &Snapshot,
        target: &PaneGraphicsTarget,
    ) -> Result<Outcome, Failure> {
        self.retain(snapshot)?;
        let source = Self::resolve(snapshot, target)?;
        let prefix = prefix(&source);
        let mut layers = self
            .layers
            .iter()
            .filter(|(key, _)| key.0 == prefix.0 && key.1 == prefix.1)
            .filter_map(|(key, layer)| {
                layer.image.as_ref().map(|image| PaneGraphicsLayer {
                    layer_id: key.2.clone(),
                    z_index: layer.z,
                    width: image.width(),
                    height: image.height(),
                    placement: image.placement(),
                    resource_revision: layer.resource_revision,
                    stream_id: layer.producer.as_ref().map(|owner| owner.id.clone()),
                })
            })
            .collect::<Vec<_>>();
        layers.sort_by(|a, b| {
            a.z_index
                .cmp(&b.z_index)
                .then_with(|| a.layer_id.cmp(&b.layer_id))
        });
        Ok(Outcome::Catalog {
            target: resolved(&source),
            revision: WorkbenchRevision::new(self.revision),
            layers,
        })
    }
    /// Copies one bounded pixel chunk only after matching current source and exact resource revision.
    pub fn pixels(
        &mut self,
        snapshot: &Snapshot,
        target: &PaneGraphicsTarget,
        layer_id: &str,
        revision: WorkbenchRevision,
        offset: u32,
        maximum: u32,
    ) -> Result<Outcome, Failure> {
        use base64::Engine;
        if maximum == 0
            || maximum as usize > MAXIMUM_GRAPHICS_CHUNK_BYTES
            || !maximum.is_multiple_of(4)
            || !offset.is_multiple_of(4)
        {
            return Err(Failure::InvalidIntent);
        }
        self.retain(snapshot)?;
        let source = Self::resolve(snapshot, target)?;
        let layer = self
            .layers
            .get(&key(&source, layer_id))
            .ok_or(Failure::NotFound)?;
        if layer.resource_revision != revision {
            return Err(Failure::StaleResource);
        }
        let image = layer.image.as_ref().ok_or(Failure::NotFound)?;
        let start = offset as usize;
        if start >= image.bytes().len() {
            return Err(Failure::InvalidIntent);
        }
        let end = start
            .saturating_add(maximum as usize)
            .min(image.bytes().len());
        Ok(Outcome::Pixels {
            target: resolved(&source),
            layer_id: layer_id.into(),
            resource_revision: revision,
            width: image.width(),
            height: image.height(),
            offset,
            total_bytes: image.bytes().len() as u32,
            data_base64: base64::engine::general_purpose::STANDARD
                .encode(&image.bytes()[start..end]),
            complete: end == image.bytes().len(),
        })
    }
    /// Installs a complete static image; conflicts, changed sources and capacity refusal preserve the old layer.
    pub fn set(
        &mut self,
        snapshot: &Snapshot,
        prepared: &SemanticPane,
        layer_id: &str,
        z: i32,
        image: DecodedGraphicsImage,
    ) -> Result<Outcome, Failure> {
        self.retain(snapshot)?;
        let source = self.current(snapshot, prepared)?;
        let key = key(&source, layer_id);
        if self
            .layers
            .get(&key)
            .is_some_and(|layer| layer.producer.is_some())
        {
            return Err(Failure::Conflict);
        }
        let changed = self.install(source.clone(), key, z, image, None)?;
        Ok(Outcome::Applied {
            target: resolved(&source),
            layer_id: layer_id.into(),
            revision: WorkbenchRevision::new(self.revision),
            changed,
        })
    }
    /// Removes a static layer; an absent layer is unchanged, and an active producer is never stolen.
    pub fn clear(
        &mut self,
        snapshot: &Snapshot,
        target: &PaneGraphicsTarget,
        layer_id: &str,
    ) -> Result<Outcome, Failure> {
        self.retain(snapshot)?;
        let source = Self::resolve(snapshot, target)?;
        let key = key(&source, layer_id);
        if self
            .layers
            .get(&key)
            .is_some_and(|layer| layer.producer.is_some())
        {
            return Err(Failure::Conflict);
        }
        let changed = if self.layers.contains_key(&key) {
            self.advance()?;
            self.remove(&key);
            true
        } else {
            false
        };
        Ok(Outcome::Applied {
            target: resolved(&source),
            layer_id: layer_id.into(),
            revision: WorkbenchRevision::new(self.revision),
            changed,
        })
    }
    /// Opens an inline-only producer; platform adapters use prepare/commit for private files.
    pub fn open(
        &mut self,
        snapshot: &Snapshot,
        connection: NonZeroU64,
        id: &WorkbenchRequestId,
        target: &PaneGraphicsTarget,
        layer_id: &str,
        z: i32,
    ) -> Result<Outcome, Failure> {
        let intent = PaneGraphicsOpen {
            connection,
            stream_id: id,
            target,
            layer_id,
            z_index: z,
        };
        match self.prepare_open(snapshot, &intent)? {
            PaneGraphicsOpenPreparation::Existing(outcome) => Ok(outcome),
            PaneGraphicsOpenPreparation::Ready(source) => {
                self.commit_open(snapshot, &intent, &source, None)
            }
        }
    }
    /// Proves source, producer conflict, capacity and revision before platform file effects.
    pub fn prepare_open(
        &mut self,
        snapshot: &Snapshot,
        intent: &PaneGraphicsOpen<'_>,
    ) -> Result<PaneGraphicsOpenPreparation, Failure> {
        self.retain(snapshot)?;
        let source = Self::resolve(snapshot, intent.target)?;
        let key = key(&source, intent.layer_id);
        if let Some((owned_key, owned)) = self.layers.iter().find(|(_, layer)| {
            layer.producer.as_ref().is_some_and(|owner| {
                owner.connection == intent.connection && &owner.id == intent.stream_id
            })
        }) {
            if *owned_key == key && owned.z == intent.z_index {
                return Ok(PaneGraphicsOpenPreparation::Existing(
                    Outcome::StreamOpened {
                        stream_id: intent.stream_id.clone(),
                        target: resolved(&source),
                        layer_id: intent.layer_id.into(),
                        z_index: intent.z_index,
                        revision: WorkbenchRevision::new(self.revision),
                        file_frame_directory: owned
                            .producer
                            .as_ref()
                            .and_then(|owner| owner.file_directory.clone()),
                    },
                ));
            }
            return Err(Failure::Conflict);
        }
        if self.layers.values().any(|layer| {
            layer
                .producer
                .as_ref()
                .is_some_and(|owner| owner.connection == intent.connection)
        }) || self
            .layers
            .get(&key)
            .is_some_and(|layer| layer.producer.is_some())
        {
            return Err(Failure::Conflict);
        }
        self.capacity(&key, 0)?;
        if self.revision == u64::MAX {
            return Err(Failure::RevisionExhausted);
        }
        Ok(PaneGraphicsOpenPreparation::Ready(source))
    }
    /// Commits a prepared producer only if source and ownership still match; platform handles stay outside this owner.
    pub fn commit_open(
        &mut self,
        snapshot: &Snapshot,
        intent: &PaneGraphicsOpen<'_>,
        prepared: &SemanticPane,
        file_directory: Option<String>,
    ) -> Result<Outcome, Failure> {
        let preparation = self.prepare_open(snapshot, intent)?;
        let current = Self::resolve(snapshot, intent.target)?;
        if !same_source(&current, prepared) {
            return Err(Failure::SourceChanged);
        }
        if let PaneGraphicsOpenPreparation::Existing(outcome) = preparation {
            return Ok(outcome);
        }
        let key = key(&current, intent.layer_id);
        let revision = self.advance()?;
        self.remove(&key);
        self.layers.insert(
            key,
            Layer {
                source: current.clone(),
                z: intent.z_index,
                image: None,
                resource_revision: revision,
                producer: Some(Producer {
                    connection: intent.connection,
                    id: intent.stream_id.clone(),
                    sequence: 0,
                    file_directory: file_directory.clone(),
                }),
            },
        );
        Ok(Outcome::StreamOpened {
            stream_id: intent.stream_id.clone(),
            target: resolved(&current),
            layer_id: intent.layer_id.into(),
            z_index: intent.z_index,
            revision,
            file_frame_directory: file_directory,
        })
    }
    /// Returns only the exact retained producer's directory, never a caller-selected file root.
    pub fn producer_file_directory(
        &self,
        connection: NonZeroU64,
        id: &WorkbenchRequestId,
    ) -> Result<Option<String>, Failure> {
        Ok(self
            .producer(connection, id)?
            .1
            .producer
            .as_ref()
            .and_then(|owner| owner.file_directory.clone()))
    }
    /// A connection can discover its own existing producer root on the selected canonical surface.
    pub fn connection_file_directory(
        &mut self,
        snapshot: &Snapshot,
        target: &PaneGraphicsTarget,
        connection: NonZeroU64,
    ) -> Result<Option<String>, Failure> {
        self.retain(snapshot)?;
        let source = Self::resolve(snapshot, target)?;
        Ok(self.layers.values().find_map(|layer| {
            layer
                .producer
                .as_ref()
                .filter(|owner| {
                    owner.connection == connection && same_source(&layer.source, &source)
                })
                .and_then(|owner| owner.file_directory.clone())
        }))
    }
    /// Bounded root identities for platform maintenance; canonical retirement contains no filesystem effects.
    pub fn retained_file_directories(&self) -> Vec<String> {
        self.layers
            .values()
            .filter_map(|layer| {
                layer
                    .producer
                    .as_ref()
                    .and_then(|owner| owner.file_directory.clone())
            })
            .collect()
    }
    /// Shutdown retires every producer even after revision exhaustion; static images remain Site-owned.
    pub fn retire_streams(&mut self) {
        let keys = self
            .layers
            .iter()
            .filter(|(_, layer)| layer.producer.is_some())
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.release(&key);
        }
    }
    /// Returns the pinned producer source before decoding; a foreign connection cannot address it.
    pub fn producer_source(
        &mut self,
        snapshot: &Snapshot,
        connection: NonZeroU64,
        id: &WorkbenchRequestId,
        sequence: WorkbenchRevision,
    ) -> Result<SemanticPane, Failure> {
        self.retain(snapshot)?;
        let (_, layer) = self.producer(connection, id)?;
        let producer = layer.producer.as_ref().ok_or(Failure::StreamClosed)?;
        if sequence.value() == 0 || sequence.value() <= producer.sequence {
            return Err(Failure::InvalidSequence);
        }
        Ok(layer.source.clone())
    }
    /// Applies a complete frame to its retained producer; failed preparation never consumes its sequence.
    pub fn frame(
        &mut self,
        snapshot: &Snapshot,
        commit: PaneGraphicsFrameCommit<'_>,
    ) -> Result<Outcome, Failure> {
        let PaneGraphicsFrameCommit {
            connection,
            stream_id: id,
            source: prepared,
            sequence,
            frame_revision,
            image,
        } = commit;
        self.retain(snapshot)?;
        let source = self.current(snapshot, prepared)?;
        let (key, layer) = self.producer(connection, id)?;
        if !same_source(&source, &layer.source) {
            return Err(Failure::SourceChanged);
        }
        let key = key.clone();
        let z = layer.z;
        let mut producer = layer.producer.clone().ok_or(Failure::StreamClosed)?;
        if sequence.value() == 0 || sequence.value() <= producer.sequence {
            return Err(Failure::InvalidSequence);
        }
        producer.sequence = sequence.value();
        let changed = self.install(source.clone(), key.clone(), z, image, Some(producer))?;
        Ok(Outcome::FrameApplied {
            stream_id: id.clone(),
            target: resolved(&source),
            layer_id: key.2,
            sequence,
            frame_revision,
            revision: WorkbenchRevision::new(self.revision),
            changed,
        })
    }
    /// Releases the owning stream and layer; duplicate cleanup cannot remove a replacement producer.
    pub fn close(
        &mut self,
        connection: NonZeroU64,
        id: &WorkbenchRequestId,
    ) -> Result<Outcome, Failure> {
        let key = self.producer(connection, id)?.0.clone();
        self.release(&key);
        Ok(Outcome::Closed {
            stream_id: id.clone(),
            revision: WorkbenchRevision::new(self.revision),
            removed: true,
        })
    }
    /// Connection EOF always frees its own pixels, even if the revision counter has exhausted.
    pub fn disconnect(&mut self, connection: NonZeroU64) {
        let keys = self
            .layers
            .iter()
            .filter(|(_, layer)| {
                layer
                    .producer
                    .as_ref()
                    .is_some_and(|owner| owner.connection == connection)
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in keys {
            self.release(&key);
        }
    }
    /// Retires removed or replaced Presentations; unavailable topology is not passed as an empty snapshot.
    pub fn retain(&mut self, snapshot: &Snapshot) -> Result<(), Failure> {
        let retired = self
            .layers
            .iter()
            .filter(|(_, layer)| {
                let target = resolved(&layer.source);
                !Self::resolve(snapshot, &target)
                    .is_ok_and(|current| same_source(&current, &layer.source))
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in retired {
            self.release(&key);
        }
        if self.exhausted {
            Err(Failure::RevisionExhausted)
        } else {
            Ok(())
        }
    }
    fn producer(
        &self,
        connection: NonZeroU64,
        id: &WorkbenchRequestId,
    ) -> Result<(&Key, &Layer), Failure> {
        self.layers
            .iter()
            .find(|(_, layer)| {
                layer
                    .producer
                    .as_ref()
                    .is_some_and(|owner| owner.connection == connection && &owner.id == id)
            })
            .ok_or(Failure::StreamClosed)
    }
    fn current(
        &self,
        snapshot: &Snapshot,
        prepared: &SemanticPane,
    ) -> Result<SemanticPane, Failure> {
        let current =
            Self::resolve(snapshot, &resolved(prepared)).map_err(|_| Failure::SourceChanged)?;
        if !same_source(&current, prepared) {
            return Err(Failure::SourceChanged);
        }
        Ok(current)
    }
    fn capacity(&self, key: &Key, new_bytes: usize) -> Result<(), Failure> {
        if !orchbee_workbench_protocol::graphics_layer_id(&key.2) {
            return Err(Failure::InvalidIntent);
        }
        if !self.layers.contains_key(key)
            && (self.layers.len() >= MAXIMUM_GRAPHICS_LAYERS
                || self
                    .layers
                    .keys()
                    .filter(|other| other.0 == key.0 && other.1 == key.1)
                    .count()
                    >= MAXIMUM_GRAPHICS_LAYERS_PER_PANE)
        {
            return Err(Failure::LayerLimit);
        }
        let old = self
            .layers
            .get(key)
            .and_then(|layer| layer.image.as_ref())
            .map_or(0, |image| image.bytes().len());
        if self
            .bytes
            .checked_sub(old)
            .and_then(|bytes| bytes.checked_add(new_bytes))
            .is_none_or(|bytes| bytes > MAXIMUM_GRAPHICS_RESIDENT_BYTES)
        {
            return Err(Failure::MemoryLimit);
        }
        Ok(())
    }
    fn install(
        &mut self,
        source: SemanticPane,
        key: Key,
        z: i32,
        image: DecodedGraphicsImage,
        producer: Option<Producer>,
    ) -> Result<bool, Failure> {
        self.capacity(&key, image.bytes().len())?;
        let same = self
            .layers
            .get(&key)
            .is_some_and(|layer| layer.z == z && layer.image.as_ref() == Some(&image));
        if same {
            self.layers.get_mut(&key).ok_or(Failure::NotFound)?.producer = producer;
            return Ok(false);
        }
        let revision = self.advance()?;
        self.remove(&key);
        self.bytes += image.bytes().len();
        self.layers.insert(
            key,
            Layer {
                source,
                z,
                image: Some(image),
                resource_revision: revision,
                producer,
            },
        );
        Ok(true)
    }
    fn advance(&mut self) -> Result<WorkbenchRevision, Failure> {
        if self.exhausted {
            return Err(Failure::RevisionExhausted);
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        Ok(WorkbenchRevision::new(self.revision))
    }
    fn remove(&mut self, key: &Key) {
        if let Some(layer) = self.layers.remove(key) {
            self.bytes -= layer.image.as_ref().map_or(0, |image| image.bytes().len());
        }
    }
    fn release(&mut self, key: &Key) {
        if self.advance().is_err() {
            self.exhausted = true;
        }
        self.remove(key);
    }
}
fn same_source(left: &SemanticPane, right: &SemanticPane) -> bool {
    left.panel_id == right.panel_id
        && left.presentation_id == right.presentation_id
        && left.binding == right.binding
}
fn prefix(source: &SemanticPane) -> (String, String) {
    (
        source.panel_id.as_str().into(),
        source.presentation_id.as_str().into(),
    )
}
fn key(source: &SemanticPane, layer: &str) -> Key {
    let (panel, presentation) = prefix(source);
    (panel, presentation, layer.into())
}
fn resolved(source: &SemanticPane) -> PaneGraphicsTarget {
    PaneGraphicsTarget {
        panel_id: source.panel_id.clone(),
        presentation_id: Some(source.presentation_id.clone()),
    }
}
#[cfg(test)]
mod tests;
