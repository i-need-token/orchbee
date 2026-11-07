# Native Windows Panel graphics rendering

## Image lanes

A Panel shows images from two lanes that share one renderer:

- **Session image layers** are written through the Panel graphics API and CLI. The sections from
  "Outcome and owners" through "Facts and acceptance" describe them.
- **Program images** are Kitty graphics that the program in the Panel prints to its terminal, such
  as `kitten icat`, yazi previews or `chafa -f kitty`. [Program-printed Kitty images](#program-printed-kitty-images)
  describes them.

The remote clipboard image bridge is neither lane: it moves clipboard images between machines,
never paints a Panel, and is outside this document. Each lane keeps its own bounds, evidence and
receipts.

## Outcome and owners

The native TUI consumes the existing Session-owned Panel/Presentation image layers. An independent instance of the existing cancellable logical read lane owns graphics queries on the shared Client carrier; it sends no PTY input and changes no focus. The main UI owns visible target selection, cache publication, clipping and successful draw receipts. The existing Client control publisher owns capability/draw observation on its already authenticated connection.

Only surfaces included in a successful shell draw enter the read scope. Selected primary/child identity and optional terminal binding are captured exactly; removed, rebound, hidden or switched targets leave the scope immediately. Session identity fences reconnect and cold restore. Stable labels and identity-preserving moves do not invalidate pixel resources. Source membership is checked before reads, after complete resources, and at UI publication. A maximum of 64 surfaces are queried in each bounded batch, rotating over the existing 4096-Panel context bound.

## Complete resources and bounds

Catalog metadata is compared with immutable cached layer metadata. Unchanged exact resource revisions reuse shared pixels without cloning image bytes. New RGBA resources are acquired in at most 256 KiB chunks, checking target, layer, revision, dimensions, total, exact offset, canonical base64 and completion. The repeated catalog must retain exactly the same layer metadata, and the canonical source fence must still agree before publication. Unrelated Session revision advances are admitted with the repeated catalog revision; they do not invalidate a complete resource for an unchanged layer. A failed, stale or interrupted read never produces a partially updated image. The last complete image may remain while the same source awaits a newer frame; its draw receipt retains its actual older revision.

The published cache holds at most 64 MiB of pixels and 64 image resources. One candidate batch is independently bounded at 64 MiB; immutable resources are shared between them. Thus old and candidate pixels together remain bounded at 128 MiB, with bounded carrier/base64 chunk scratch. Publication enforces the resident budget across the merged cache; if old historical resources exhaust it, they are retired before installing the complete new batch. Scope removal frees corresponding references; lane shutdown interrupts, joins and releases its last candidate. No new dependency, service, directory or temporary artifact is introduced.

## Cell raster surface

The baseline transport is actual Ratatui color cells: one upper/lower color sample per cell, represented by the upper-half block. Natural fit uses current measured cell dimensions to preserve physical image aspect within the real Panel rectangle. When those dimensions are unknown it preserves the logical sample aspect instead; it does not invent a font size. The same measured ratio resolves an omitted grid axis, while two explicit axes keep their declared dimensions. Positive unsigned 16-bit cell dimensions and unsigned 32-bit grid inputs are multiplied in signed 64-bit arithmetic under the existing 4096-pixel source bound; zero or missing measurements remain logical fallback. Extreme aspect ratios clamp to at least one column and at most the body height, so a subcell logical width still shows the complete image. Grid columns/rows accept the full unsigned 32-bit range (zero selects natural fit); explicit dimensions and signed placement are honored, with arithmetic wider than the coordinate inputs and clipping to the body. Z ordering follows the Session catalog. Negative layers preserve existing text glyphs and foreground, compositing their background; positive layers overlay text where image alpha is nonzero. Fully transparent samples preserve underlying content. Known RGB destinations use straight alpha blending; unresolved default/palette colors keep their identity below half opacity and select the image color above it. Wide glyph heads and spacers remain atomic and are preserved where a cell raster would split them. Chrome, other Panels, wide-cell ownership, source hit testing and terminal mouse modes remain with their existing owners.

Rendering uses the actual Panel body after terminal content and before shell overlays. Draw receipts are published only after Terminal.draw succeeds and only for uncovered, current sources. Cache acquisition alone is never a draw acknowledgement. Resizing, zoom, switching, clearing and shutdown recompose through the same surface owner; clearing restores underlying terminal cells on the next ordinary draw.

## Bounded host measurement

The interactive Windows shell probes only its already-admitted terminal, after entering its owned modes and before the first ordinary draw. The existing lifecycle writer issues the xterm `CSI 16 t` cell-pixel query and a Kitty `a=q,t=d,f=32,s=1,v=1` query with a nonzero random image ID. Queries have a 300 ms deadline and never wait before rendering or create image placements. Terminal names, environment variables, font guesses and producer `transport` hints cannot establish these facts. These sequences follow the [xterm control contract](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) and the [Kitty graphics contract](https://sw.kovidgoyal.net/kitty/graphics-protocol/).

The existing input decoder owns complete cell reports and graphics APC replies before key, prefix, popup and confirmation dispatch. The same metadata is consumed during a native text modal; bracketed paste remains literal paste, including bytes that look like replies. A graphics reply is bounded at 256 bytes; oversized or timed-out replies drain to their string terminator without allocating or exposing the remaining reply as typed keys. A bare Alt+underscore retains its ordinary meaning when it is not followed by the graphics introducer. No reply, a malformed reply or a late reply establishes capability, increments human-activity intent or reaches a retained terminal.

Only a timely cell reply to the sole pending query is admitted. Its positive unsigned 16-bit dimensions must produce exact, representable window pixel totals for the queried cell geometry. A geometry change invalidates measured pixels immediately. Since the legacy cell response has no correlation ID, another cell query cannot be issued while the previous response is outstanding, even after timeout; its late response settles that debt and is discarded before a later query is admitted. Known or settled measurements can be re-queried on the next geometry change. The Kitty reply must match the one issued ID and deadline without a placement ID; an unrelated ID cannot establish support. Query support is an internal fact until a physical renderer actually uses it. Cell-raster observations may publish real measured cell pixels, but continue to publish cell-raster transport and pixelMouse false.

The shell passes a measurement only to its exact cell geometry and the active native surface. It applies the same physical pixels to retained pane geometry and popup geometry; a newly selected Machine receives current host facts before its next draw. No filesystem resource, background thread, additional console reader or new dependency is introduced. Focused acceptance must witness timely, late, absent, wrong-ID, malformed and split replies, ordinary keys interleaved with replies, literal paste, modal capture, geometry invalidation and the actual lifecycle writer. Physical image placement and pixel mouse remain separate obligations.

## Physical Kitty surface

The Windows shell's fixed backend owns one UI-thread graphics surface and an owned image lease shared with terminal restoration. Only a timely successful Kitty query and current measured cell pixels admit physical output. A draw stages immutable canvases using the actual Panel bodies, Session, selected source/binding and cell measurement. Overlay, Machine, notification, source, layer-set or geometry changes retire the affected physical frame in the same synchronized draw. Unsupported or unknown hosts continue to render color cells. No producer hint, terminal name, cached Session ID or upload success alone establishes visible drawing.

A physical frame contains at most 64 images and 64 MiB of visible RGBA output. Each image is lazily sampled from its immutable source into its clipped physical rectangle; a single image exceeding 32 MiB uses cell fallback. Natural and omitted-axis placement share the cell renderer's measured aspect calculation. Full unsigned 32-bit virtual grids are clipped before sampling; wide intermediate arithmetic preserves offscreen placements without wrapping. Straight alpha, layer order and signed z remain intact. An unrelated Session revision advance reuses host images only when the exact source, complete layer metadata, immutable pixel identities and physical geometry are unchanged; a confirmed receipt can then advance to that equivalent canvas revision without retransmission. No full resampled image or full base64 payload is retained. Active and one frozen candidate can each retain at most 64 MiB of source pixels, in addition to the existing 128 MiB read/cache bound. Staged draw references are released after the successful frame flush, before another read publication; a bounded replan flag requests a later draw when resource deletion first needs to settle.

Only one chunked image transfer is open at a time. Each ordinary frame sends at most 32 chunks of at most 3072 raw bytes (4096 base64 bytes), followed by the existing single frame flush. A complete canvas is frozen while uploading so a continually advancing producer cannot restart every chunk. Each completed upload needs its exact image-ID reply within five seconds. Only a whole ready canvas replaces its previous complete physical canvas; the old canvas may remain only for the same exact source, layer set, body and measured cells, and retains its actual older revision. Other Panels keep their independent complete canvases. Placement replies additionally match the immutable image/placement pair and the deadline. Pending placement never publishes a drawn revision. An error, timeout or reply-queue overflow retires physical resources and falls back to cells for this host session, without blocking key dispatch or retrying uncertain output.

The backend emits upload, placement and deletion commands inside its existing synchronized update, before its one flush. Image IDs are positive, never reused during the owned alternate-screen session and exclude the probe ID; allocation is bounded and exhaustion selects fallback. The fresh alternate screen and sole admitted output writer fence the namespace. Only registered image IDs are deleted. The restoration lease records allocation before output and retains deletion obligations until a successful flush. On normal exit, output failure, panic or signal, restoration first terminates any partial graphics control string and deletes its registered images, then ends synchronized output and restores the remaining modes. Cleanup uses the admitted lifecycle writer, attempts later restoration steps after failure, and never reopens the TTY or leaves a graphics writer after the restoration fence.

Acceptance includes protocol reconstruction across chunk boundaries, current-source/geometry/overlay retirement, complete-frame publication under an advancing producer, wrong/late/failed upload and placement replies, output failure and cleanup retry, and an actual native shell draw with a simulated responding host. A simulated host does not establish visual compatibility with a real Windows emulator; that composed acceptance remains required.

## Facts and acceptance

An additive bounded graphics observation is submitted by the existing Client control connection after its normal synchronized view. The Server checks authenticated generation-derived Client ID, heartbeat, Workbench revision, canonical target/binding and installed revision bounds. Foreign connections cannot replace another Client's facts. Info uses fresh matching observations, reports cell-raster or admitted physical Kitty capability from the actual draw path and pixelMouse false, and publishes cell pixels only from the current bounded host measurement. Partial surface lists cannot claim an absent Pane is hidden. Stale, disconnected or mismatched observations do not claim visibility or successful drawing.

The fallback does not claim negotiated Kitty/Sixel or pixel mouse support. Managed private producer file leases and complete file-frame CLI inputs are implemented by the [Session graphics owner](../../orchbee-server/docs/windows-graphics.md). The physical Kitty path is integrated with bounded probing, uploads, placements and restoration; pixel mouse is not advertised. Focused evidence covers actual native renderer output, alpha/z/clipping, complete chunk/resource fences and Client ownership. Real Windows composed graphics workflows and final full regression remain required before alignment or main integration. No TLA, WSL, new macOS/Linux runtime or manual focus-stealing Desktop test is run.

## Program-printed Kitty images

Terminal Host owns Kitty parsing, media loading, animation and placement resolution. Each OBTF frame
carries one OBKG record of owned decoded pixels and resolved cell placements on the existing viewer
transport; Server and Worker relay frames unchanged, so no wire message, capability or schema is
added. The Client decodes OBKG strictly, matching the packaged TypeScript store: a malformed record,
unknown image, crop outside its image, or placement-only update inside a snapshot rejects the frame
and requests a snapshot.

`TerminalProjection` takes the graphics out of each applied frame, so retained frames and CLI frames
never hold pixels. A placement-only delta must name the retained resource revision and reference
retained images, and its animation tick never moves backwards; otherwise the frame is refused with a
base mismatch. The retained set is always the Host's complete current set, replaced atomically, so
an image the program deleted, or the Host evicted under its storage limit, is released with the
frame that omits it. Each attached Panel therefore retains at most the protocol bound of 32 MiB,
256 images and 1,024 placements; the Host's oldest-first storage eviction is the only eviction
authority, and the Client keeps no second copy or cache. An unchanged Host resource keeps its
shared pixels across snapshots, so physical output does not upload it again.

Each placement becomes one immutable single-layer canvas over the whole cells its destination
touches, computed from the Host cell size in the frame. The destination offset, destination size
and source crop map into that cell rectangle; the rest of it stays transparent rather than
stretched. Gray, gray-alpha and RGB pixels expand lazily in the sampler both renderers share; no
RGBA copy is made. Program images use the cell raster and physical Kitty surface described above
without a second renderer, and paint below Session layers. Signed z reaches physical placements
unchanged; in the raster, negative z keeps glyphs and their foreground. Every geometric input,
including the cell size, names the layer, so an image that moves with output scrolling or resize
retires its physical copy in the same frame and shows as cells until the new upload is confirmed.
When the Host has no cell pixel size, it resolves no placements and the Panel shows no image.

The live screen shows the current set. A history window shows its own render-detail graphics
snapshot, whose placement rows count from the window's first row; the history entry state shows the
live frame with its images. Loading and recovering history surfaces, reading stand-ins (which keep
text only, inside their witness budget), a revealed tail, and retained frames during reconnection
show none, so a live placement never lands on rows it does not belong to. A clipped live view uses
the raster crop; physical output is used only for unclipped views. Covering surfaces hide both lanes
in the same frame, and Panel bounds clip every image. Closing the Panel releases its projection,
and the next frame deletes its physical images.

Program images never enter the Session read scope or the Workbench draw observation. Their draw
facts only keep staged physical scenes through the frame and are dropped before receipts are
built.

An outer terminal that answers the Kitty query and reports cell pixels, such as WezTerm or Kitty,
shows physical images. Windows Terminal supports Sixel but not Kitty, so the TUI paints the
upper-half-block cell raster there; it emits no Sixel and writes no graphics bytes to a host
without Kitty support. When the TUI sizes a Panel from an outer terminal that does not report cell
pixels, the Host gets zero pixel metrics, so no client shows program images in that Panel.

The packaged GUI terminal renders the same OBKG records in both renderer backends, live and in
history windows. The TUI differs only where the host or the cell grid limits it: hosts without
Kitty get the cell raster; clipped views, which have no local camera transform for host
placements, get the raster crop; covering surfaces hide images instead of compositing over them,
because an outer terminal cannot clip a Kitty placement to a TUI overlay; and reading stand-ins
show text only.

Terminal Host generates the Kitty fixture sequence (a resource snapshot, the placement-only delta
after a scroll, and the empty snapshot after deletion) from a real direct transmission.
The Client's acceptance test feeds that sequence through the projection and the actual shell
renderer: a capable simulated host receives upload, placement and delete commands, and an incapable
host receives cell raster and no graphics bytes. Real-window confirmation in a Kitty-capable outer
terminal and in Windows Terminal remains required.
