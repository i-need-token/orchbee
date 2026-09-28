# Agent screen detection model

The Server owns fallback screen interpretation separately from explicit Agent reports and process identity. A successful bounded terminal sample can describe working, idle or blocked UI; it cannot prove which provider owns a process, authenticate it or create a canonical Agent Session. The reference is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, its 22 bundled detection manifests and `pane/agent_detection.rs`. The initial deterministic engine is test-gated; live Host sampling, publication, overrides, reload, updates and explain commands remain integration obligations.

## Inputs and rule ownership

The private engine accepts a known provider or absence, recent live terminal text, OSC title and OSC progress as separate fields. Screen input is limited to 64 KiB and each OSC field to 4 KiB; over-limit input is rejected whole, never truncated into misleading evidence. Text is already decoded terminal content, not raw escape sequences. Unknown providers remain unknown even when text resembles another provider. Known providers with no matching rule use idle fallback, with no visible confidence flag. Omp and Mastracode have no bundled screen manifest in the reference and use this known-provider fallback.

Bundled TOML files are retained under `resources/agent-detection` with their Apache-2.0 license and source attribution. Server maintainers own review and pinned updates. Parsing rejects unknown fields, invalid states/regions, empty rule identifiers, unsupported engine versions, invalid skip-state combinations and excessive complexity before compiling any executable rule set. Each manifest allows at most 128 rules, 512 gates, depth 8 and 1,024 matchers; each gate allows 32 direct matchers of at most 512 Unicode characters. Regex compilation has explicit memory limits. The already locked `regex` and `toml` packages supply syntax that the standard library cannot implement; no new versions or runtime framework are required.

## Deterministic decisions

| Input or match | Result |
| --- | --- |
| No known provider | Unknown, no visible evidence |
| Multiple matching rules | Highest priority; first declared rule wins equal priority |
| Contains | All needles, Unicode lowercase comparison |
| Regex / line regex | All patterns; case-sensitive unless the expression changes flags; line patterns may match different lines |
| Nested all / any / not | All children / at least one child if present / no child matches |
| No matching rule for known provider | Idle without visible idle evidence |
| Transcript/history viewer rule | Skip state publication, preserve existing fallback |
| Visibility flag inconsistent with state | Flag is false in the derived result |

Regions follow the reference's live prompt, response markers, prompt boxes, top/bottom nonempty lines and separate OSC fields. Line boundaries retain correct UTF-8 offsets for LF and CRLF, including blank lines and a final unterminated line; no byte offset may split a Unicode character. All matching is pure and does not access files, processes, clocks or mutable global state. Compiled rules may be retained by the sampling owner and reused.

## Publication integration contract

The future Host adapter must validate the exact attachment, Worker generation and Host identity and associate screen evidence with the same observed PID/creation identity. It may not reinterpret a failed/missing frame as an empty screen or derive idle from unavailable data. Existing process exits retain precedence. Full-lifecycle report authority remains stronger than fallback; visible blockers only override weaker matching authority through the existing report aggregate.

Working-to-plain-idle transitions hold for three further confirmations or 700 ms; explicit visible idle/blockers, provider change or exit clear that hold. Stable visible blockers refresh at 800 ms so newer weak reports can be challenged without publishing every sampling tick. Startup grace and sampling cadence are adapter responsibilities. Screen and process sampling must retain the existing required-completion and uncertain-viewer-cleanup ownership. No controller/input/focus effect is admitted.

## Evidence and delivery

Rule, region, bounds, priority and malformed-manifest tests exercise the engine before transport integration. Pinned-reference cases and differential evaluation must cover every bundled provider, nested gates, transcript skip and OSC precedence; a passing static engine does not close the live CLI/TUI requirement. Later Host acceptance must prove real Windows screen changes and report arbitration without user query effects. Source/cohort revert is rollback; no durable state or wire migration is involved. The final full gate remains deferred until all parity implementation is complete.
