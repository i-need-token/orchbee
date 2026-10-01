# Windows Panel graphics CLI

## Commands and input

`panel graphics set PANEL --spec SPEC`, `clear PANEL [--spec SPEC]`, `info PANEL [--client ID]`, and `stream PANEL [--spec SPEC]` join an existing local Windows Site. All four accept `--presentation ID`, `--timeout MS`, and `--format text|json`. An omitted Presentation selects the current primary; explicit IDs must belong to that Panel. No command starts a Site, changes focus, injects terminal input, or replays an uncertain write.

Set SPEC is a closed JSON object with `image` and optional `layerId` (default `primary`) and `zIndex` (default zero). The image is the shared camelCase PNG/RGB/RGBA/BGRA frame including dimensions, canonical `dataBase64` and placement. `@PATH` reads a bounded regular file containing that object or a PNG; `-` reads a noninteractive inherited pipe or regular stdin. A PNG uses its actual IHDR dimensions and default layer/placement. Input is acquired before Site discovery and validated before submission. One-shot raw image input is at most 512 KiB; an encoded JSON document is at most 768 KiB. Inline argv retains the existing 64 KiB argument budget. Clear SPEC admits only optional `layerId`; stream SPEC additionally admits `zIndex`. Info admits no SPEC.

Stream reads a JSON header terminated by LF, then exactly `dataLength` raw bytes. Header keys are `format`, `width`, `height`, `dataLength`, optional `sequence`, `revision`, `placement`, and `damage`; reference snake_case image/data/placement keys are admitted aliases. Headers are at most 8 KiB and each body at most 16 MiB. Missing sequence advances the last accepted sequence; explicit sequence must increase and is a decimal string or safe JSON integer. Revision is a full-width decimal string or safe JSON integer, default zero. Damage is validated against a complete supplied frame. Blank headers are ignored; truncated headers/bodies, duplicate/unknown fields, invalid sizes, or stale sequences end the producer. File headers use only the Server-issued private producer directory once that lease capability is available; arbitrary paths are never accepted as Server reads.

Set/clear/info have a five-second default overall deadline. Stream has no overall deadline unless supplied, and a 30-second idle/total frame acquisition budget. Every exchange and stdout record is independently bounded. Hidden fixed Windows byte relays carry only inherited bytes; their invocation owner kills and waits for them on every exit. No helper decodes images or evaluates caller text. Ctrl+C stops input and closes the producer; EOF closes only this connection's stream and releases its layer. Static layers survive the uploading CLI's exit.

## Receipts and failures

One-shot JSON uses `orchbee.panel.graphics` v1; streamed NDJSON uses `orchbee.panel.graphics.stream` v1. Correlated Server results retain target, resource revision, stream ID, sequence and producer revision. Installation is separate from Client visibility and successful drawing. Nullable physical/capability facts remain unknown until an actual Client publishes them.

A submitted write with a missing or invalid response returns an explicit unknown receipt and exit 8; it is never reported as a successful cancellation or retried. Known refusal is exit 3, unavailable source 4, transport/protocol failure 5, deadline 7 and cancellation 130. Stream termination retains the last actual receipt and cleanup result, distinguishing acknowledged close, connection-ended cleanup, and unknown cleanup. A stalled or partially written stdout record is never followed by a fabricated terminal record.

## Ownership and acceptance

The shared contract and Site owner are documented in [Windows Panel graphics](../../orchbee-server/docs/windows-graphics.md). Layers, complete-frame staging, source fences, budgets and EOF ownership use that existing authority. CLI and TUI consumers are implemented on the same isolated branch, before final composed Windows acceptance; protocol/domain tests and command existence alone do not align R078–R081. macOS and Linux runtime additions, TLA and the manual focus-stealing Desktop test remain outside this work.

## Current implementation and focused evidence

The four native paths, help, completion, strict inputs, correlated receipts and EOF cleanup are implemented using existing read/navigation grants. Actual Windows CLI named-pipe tests cover eleven exchange/stream scenarios plus input-before-discovery; the shared owner maintains the three-language graphics contract. Native dictionary v136 contains 120 documents, adding two output schemas and a dedicated graphics failure schema. Actual TUI rendering, capability/draw facts and private file-frame leases remain pending; focused evidence does not replace final composed and full acceptance. Idle input with no partial frame does not consume a frame deadline.
