# Windows Panel graphics

## Accepted outcome and ownership

Windows supports Panel graphics set, clear, information and framed streams. A working renderer requires end-to-end evidence. Desktop, macOS and Linux runtime additions are outside this boundary.

The Session owns ephemeral layers by canonical Panel and Presentation identity. Its existing
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
intact. A Session retains at most 64 layer slots, at most 16 per Presentation and at most 64 MiB of
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

Stream framing is a bounded JSON header followed by its declared raw byte
body. Every accepted frame returns its actual sequence/revision receipt. Damage rectangles are
bounded hints against a complete canonical frame, never patches that leave unprovided pixels
undefined. Bad framing terminates the producer and releases its layer. Private file frames admit
only exact regular files within the Session-owned producer root, with pinned size and bounded reads;
consumed leases and producer roots are released after their last consumer. External paths are not
an upload shortcut. A lost write receipt remains unknown and is not replayed.

## Windows producer file ownership

Managed Windows Sessions lazily allocate one private directory for each admitted producer. A pure
open preparation first proves canonical source, connection ownership and capacity; directory I/O
then runs outside canonical locks, and commit repeats the source and producer checks. Idempotent
open returns the same directory. Allocation failure preserves the old static layer. Unmanaged
endpoint mode advertises no file directory and refuses file frames.

The Server file owner retains at most 128 roots, including failed or retired roots, with at most
64 live producers. A preparing token prevents an uncommitted root from being collected. The
canonical layer owner retains only the directory identity, with no Windows handles or filesystem
effects. Frame preparation leases that exact root after producer/source/sequence admission;
RGBA/BGRA bytes are copied and normalized before atomic commit. No base64 round trip or partial
patch is required. The frame header remains bounded at 16 MiB and damage is only a complete-frame
hint. Info returns a directory only for an existing producer on the requesting connection; a
reader cannot obtain another producer's root.

Canonical removal, close and EOF retire the producer immediately. The existing Close maintenance
owner checks retained directories at most every 250 ms while roots exist, then reaps retired
roots outside locks only after their last preparing/reading token ends. Failed deletion retains
the exact owner record and retries; no retired root can admit a new frame. Shutdown closes write
admission, settles preparation, retires producers and reaps all owned roots before publishing
Stopped. A busy lease/root or failed cleanup cannot prove clean shutdown. Crash recovery follows
the existing Active-record refusal and never adopts unknown graphics roots.

Focused evidence maps ownership to real Windows file/handle traces and production Workbench
sessions: source changes and foreign producers cannot read, malformed files preserve pixels and
sequence, complete BGRA normalizes correctly, idempotent open is stable, EOF/topology removal
releases roots, and busy cleanup retains responsibility. Final composed/full acceptance and
physical pixel transports remain separate obligations.

## Windows private frame directories

The Server-private `frame_directory` module owns `PrivateFrameDirectory`, which retains one ephemeral `graphics-<32-hex-nonce>` leaf beneath an
already admitted private runtime. `reserve` pins its ancestors without creating the leaf;
`create` exclusively creates it and writes the application's bounded owner record. Failed
creation keeps the same explicit owner until `reap` confirms absence. An existing leaf is never
adopted, repaired or deleted. Applications own producer admission, nonce generation, retention,
retry and shutdown; this facility starts no process or cleanup thread.

Only an admitted directory reads frames. The supplied path must be one exact direct child,
with a normal Windows filename; traversal, alternate streams, reserved names, reparse points,
hard links, nonregular files and foreign ACLs are refused. A read opens without write/delete
sharing, checks the exact declared length, copies at most 16 MiB and confirms EOF. The immutable
copy has no remaining file dependency. Producers must close their writer before submitting it.
Leading-dot image names such as `.frame.rgba` are ordinary leaf names; the exact owner marker
remains reserved.

The lifecycle is Reserved → Preparing → Ready → Releasing → Released. Preparation failure moves
to Releasing under the same owner. Releasing closes the root pin, checks the saved directory
identity and removes only its derived owned leaf while retaining pinned ancestors. A busy or
redirected root remains owned for retry; dropping the object is not cleanup evidence. Application
leases keep Ready alive until the last reader finishes. Windows handle traces cover bounded reads,
private admission, writer/rename exclusion, creation refusal and failed-removal retry. Same-user
malicious ACL changes, process crashes and replacement during the final delete syscall are outside
this per-user development boundary; a crash does not prove clean runtime shutdown.

## Client rendering and truthful information

The native Windows Client must consume the installed layer catalog and complete pixels. Its
ordinary cell surface provides a color raster fallback; negotiated pixel transports can improve
resolution without changing layer ownership. Pixel transport, physical cell dimensions and pixel
mouse are reported only after actual capability evidence. Unknown dimensions remain unknown.
Information distinguishes installation from a matching Client draw; hidden or stale Clients do
not manufacture visibility or acknowledgement. Clearing, zoom, switching, resizing and leaving
the TUI retire corresponding output resources through the same renderer owner.

## Evidence and integration

Dependency order is strict image/resource contracts, Session layer/stream transactions and bounds,
correlated existing Host PNG decoding, actual native Client consumption/rendering, then public
CLI grammar/framing/receipts. Focused checks cover complete old-layer preservation, source changes,
cross-connection producer conflict, EOF cleanup, ordering/exhaustion, chunk mismatch, PNG bounds,
alpha/z/negative-placement clipping and real Windows CLI pipes. Final composed Windows rendering
and full regression run after all remaining primary capabilities are implemented. Foundation tests
alone do not mark these four methods aligned.

## Implementation checkpoint

The Windows Session layer transactions and read-only Host PNG adapter are implemented. The canonical projection owns the shared layer store and retires disappeared or rebound sources at commit, without waiting for a graphics reader. Preparation has its own serial lock; lock order is canonical owner, then layer store, and PNG I/O holds neither. Static primary selectors are checked again after decoding.

OBWP kinds 57/58 carry closed requests and results. OBNG carries a bounded viewer-scoped PNG request; existing correlated OBNR records carry its result. Terminal Host kinds 80/81 use the distinct graphics-image encoding (5), bounded at 32 MiB plus its 56-byte result header; ordinary input bounds are unchanged.

The native CLI upload/stream paths and [native TUI cell raster consumer](../../orchbee/docs/native-panel-graphics.md) are implemented. The dedicated read lane stages complete, revision-pinned resources; the authenticated Client publisher records successful uncovered draw observations. Info uses fresh matching Client facts and leaves unknown physical pixel geometry null. Managed Windows producers now own private file-frame directories and complete bounded copies, with preparation/read tokens, maintenance retry and clean-stop settlement. Unmanaged endpoint mode advertises no directory. Physical pixel transports and final composed/full acceptance remain pending; these four comparison rows still await exact reference review and Windows acceptance. The complete native cohort is versioned and restaged for final integration; no main integration or old-cohort substitution is authorized by this checkpoint.

Focused evidence includes the 62-case Rust/TypeScript/JSON Schema corpus, image and carrier bounds, eight layer-owner tests, seven production Server session/observer/file witnesses and a Client freshness/scope test, and five Host PNG/graphics tests including actual Windows PTY creation and PNG decoding. This is component evidence, not composed renderer or final acceptance.

## Consumer implementation order

Within the same authorized Windows replacement outcome, the foundation is followed by CLI upload and retained streams, then actual TUI rendering and capability facts; composed acceptance follows both. The [Windows Panel graphics CLI](../../orchbee/docs/windows-panel-graphics.md) owns grammar and input rules. This scheduling adjustment does not change alignment completion criteria.
