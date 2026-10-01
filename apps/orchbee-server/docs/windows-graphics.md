# Windows Panel graphics

## Accepted outcome and ownership

The standing Windows CLI replacement authorization includes real Panel graphics set, clear,
information and framed streams (R078–R081). Reference behavior is pinned to Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34`, `app/api/pane_graphics.rs`,
`api/server/pane_graphics_stream.rs` and `app/pane_graphics.rs`. Public wrappers alone do not
establish a working renderer. Desktop, macOS and Linux runtime additions are outside this change.

The Site owns ephemeral layers by canonical Panel and Presentation identity. Its existing
navigation-management grant admits visual writes; its read grant admits information and bounded
asset reads. A graphic never writes PTY input or changes terminal scrollback. The Client owns the
actual render surface, clipping, capability observations and draw acknowledgement. Caller-supplied
cell size, viewport visibility or render success is not authoritative.

A primary selector resolves to one current Presentation before preparation. Replacement or removal
of that identity or its terminal binding invalidates preparation; rename and stable-identity moves
do not invent replacement. Source admission is repeated before commit. Layers disappear when their
Presentation disappears. They are not durable checkpoint data.

## Images and bounds

The wire admits PNG, RGB, RGBA and BGRA with explicit dimensions and canonical padded base64.
One-shot input is at most 512 KiB; a stream frame is at most 16 MiB. Dimensions are 1–4096 on each
axis and decoded RGBA is at most 32 MiB. Length arithmetic is checked before allocation. PNG must
decode completely and agree with declared dimensions. Existing Host-owned Wuffs PNG decoding is
reused through a correlated read-only adapter; no shell helper, PTY escape injection, new decoder
dependency or persistent service is added. The existing temporary-viewer owner retains uncertain
physical cleanup, and a failed decode or cleanup cannot install a layer.

Canonical stored pixels are RGBA. Replacement stages complete pixels before releasing the old
layer. Malformed input, source changes, budget exhaustion and stream conflicts leave the old layer
intact. A Site retains at most 64 layer slots, at most 16 per Presentation and at most 64 MiB of
stored pixels. A single preparing image and decoder scratch are bounded separately. Decoding is
serialized by the graphics owner without holding the canonical topology lock.

Catalog reads contain metadata, never complete image copies. Pixel reads carry at most 256 KiB
and pin the exact resource revision, dimensions, offset and total length. A Client stages a whole
asset and publishes it only after all chunks agree and a current catalog still identifies it.
Unknown, stale, truncated or oversized chunks never produce partially updated pixels.

## Layer and producer lifecycle

Layer IDs contain 1–80 ASCII letters, digits, dot, underscore or hyphen. Omitted CLI layer IDs
select `primary`. Placement uses signed viewport row/column, optional explicit grid dimensions
(zero requests natural fit) and a signed z-index. Clip and alpha composition operate inside the
actual Panel surface. Neither a negative coordinate nor a large placement permits drawing chrome
or another Panel.

| State           | Input                      | Guard                                                 | Result                                       |
| --------------- | -------------------------- | ----------------------------------------------------- | -------------------------------------------- |
| Vacant / static | Set                        | Current source, valid complete image, capacity        | Install static layer                         |
| Vacant / static | Stream open                | Current source, free producer slot                    | One connection owns stream                   |
| Streaming       | Set / clear / another open | Producer is active                                    | Refuse without changing pixels               |
| Streaming       | Frame                      | Exact owner, increasing nonzero sequence, valid image | Atomically replace frame                     |
| Streaming       | Duplicate / older sequence | Sequence does not advance                             | Refuse without replacement                   |
| Streaming       | Close / EOF / cancellation | Owning connection ends                                | Release stream and layer                     |
| Static / vacant | Clear                      | No active producer                                    | Remove layer; absent is an unchanged success |
| Any             | Presentation removed       | Canonical identity no longer exists                   | Retire owned layers and producer             |

Stream framing follows the reference: a bounded JSON header followed by its declared raw byte
body. Every accepted frame returns its actual sequence/revision receipt. Damage rectangles are
bounded hints against a complete canonical frame, never patches that leave unprovided pixels
undefined. Bad framing terminates the producer and releases its layer. Private file frames admit
only exact regular files within the Site-owned producer root, with pinned size and bounded reads;
consumed leases and producer roots are released after their last consumer. External paths are not
an upload shortcut. A lost write receipt remains unknown and is not replayed.

## Client rendering and truthful information

The native Windows Client must consume the installed layer catalog and complete pixels. Its
ordinary cell surface provides a color raster fallback; negotiated pixel transports can improve
resolution without changing layer ownership. Pixel transport, physical cell dimensions and pixel
mouse are reported only after actual capability evidence. Unknown dimensions remain unknown.
Information distinguishes installation from a matching Client draw; hidden or stale Clients do
not manufacture visibility or acknowledgement. Clearing, zoom, switching, resizing and leaving
the TUI retire corresponding output resources through the same renderer owner.

## Evidence and integration

Dependency order is strict image/resource contracts, Site layer/stream transactions and bounds,
correlated existing Host PNG decoding, actual native Client consumption/rendering, then public
CLI grammar/framing/receipts. Focused checks cover complete old-layer preservation, source changes,
cross-connection producer conflict, EOF cleanup, ordering/exhaustion, chunk mismatch, PNG bounds,
alpha/z/negative-placement clipping and real Windows CLI pipes. Final composed Windows rendering
and full regression run after all remaining primary capabilities are implemented. Foundation tests
alone do not mark these four methods aligned.

## Implementation checkpoint

The Windows Site layer transactions and read-only Host PNG adapter are implemented. The canonical projection owns the shared layer store and retires disappeared or rebound sources at commit, without waiting for a graphics reader. Preparation has its own serial lock; lock order is canonical owner, then layer store, and PNG I/O holds neither. Static primary selectors are checked again after decoding.

OBWP kinds 57/58 carry closed requests and results. OBNG carries a bounded viewer-scoped PNG request; existing correlated OBNR records carry its result. Terminal Host kinds 80/81 use the distinct graphics-image encoding (5), bounded at 32 MiB plus its 56-byte result header; ordinary input bounds are unchanged.

The native CLI upload/stream paths and [native TUI cell raster consumer](../../orchbee/docs/native-panel-graphics.md) are implemented. The dedicated read lane stages complete, revision-pinned resources; the authenticated Client publisher records successful uncovered draw observations. Info uses fresh matching Client facts and leaves unknown physical pixel geometry null. Private file-frame producer leases remain pending, no directory is advertised, and file-frame requests are refused. These four comparison rows remain missing until an actual consumer and Windows acceptance establish the behavior. The complete native cohort is versioned and restaged for final integration; no main integration or old-cohort substitution is authorized by this checkpoint.

Focused evidence includes the 62-case Rust/TypeScript/JSON Schema corpus, image and carrier bounds, six layer-owner tests, three production Server session/observer witnesses and a Client freshness/scope test, and five Host PNG/graphics tests including actual Windows PTY creation and PNG decoding. This is component evidence, not composed renderer or final acceptance.

## Consumer implementation order

Within the same authorized Windows replacement outcome, the foundation is followed by CLI upload and retained streams, then actual TUI rendering and capability facts; composed acceptance follows both. The [Windows Panel graphics CLI](../../orchbee/docs/windows-panel-graphics.md) owns grammar and input rules. This scheduling adjustment does not change alignment completion criteria.
