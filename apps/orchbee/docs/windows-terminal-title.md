# Windows outer terminal titles

`terminal title set <title>` and `terminal title clear` submit one change to the existing Site. Both accept `--format text|json`. Set removes control characters, takes at most 200 Unicode scalars, then trims; empty results fail before submission. Internal spaces are preserved. No Site or terminal is created.

## State and delivery

The Server owns one ephemeral override and monotonic revision, separate from Panel labels. Set and clear publish even without a Client; reconnecting Clients receive the latest state. Clear currently publishes the application fallback `Orchbee`. Before the first publication the host title is left alone. Full `ui.window_title` templates and restart/live-handoff transfer remain separate parity work.

Each authenticated listening connection owns one slot (maximum 32, five-second heartbeat lifetime). New attachment or a false-to-true focus transition promotes that connection; idle polling does not steal foreground. Disconnect/expiry promotes the latest remaining connection. Only that foreground receives the publication. This describes reported Client focus, not an OS focus query. Clients reapply the current title on foreground promotion; idle repeats are suppressed and demotion discards undrawn state. The latest snapshot replaces older undrawn state; no unbounded event queue is kept. At most 256 request receipts are retained for 60 seconds; exact replay returns the original receipt without republishing, conflicting identity fails, saturation refuses before commit. `changed` records foreground delivery eligibility at commit, not a physical title acknowledgement. With no foreground, the publication is still committed and the receipt says so. Lost submitted responses remain unknown and are not automatically retried.

## Windows lifecycle

An independent cancellable two-second-bound exchange polls every 100 ms. Its thread owns transport and a single latest handoff; the UI owner alone requests the lifecycle write. Closing cancels blocked admission/polling and joins before releasing resources. Title changes never alter input, selection, drafts or layout.

The lifecycle captures the inherited console title before its first change, then uses safe Windows title APIs. `win32console` 0.1.5 is admitted only for this capture/write capability: no standard-library or existing safe binding offers capture; it adds no build script and uses the existing winapi version. Its fixed UTF-16 capture buffer is checked for padding and ambiguous truncation; titles of 259 or more UTF-16 units and invalid UTF-16 refuse capture before any title change. Last-error zero distinguishes a valid empty title. Restoring uses the exact captured string, not escaped display text. The existing bounded restoration lease runs on normal exit, errors and owner-thread panic, attempts other cleanup after failures and blocks subsequent title writes. OS/host settings may ignore requested title changes.

## Contract and evidence

Matched private cohorts add OBWP kinds 43/44 and roll out together. `orchbee.terminal.title` v1 is a closed public receipt. No persisted migration is introduced. Herdr reference is `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `cli.rs`, `config/window_title.rs` and `server/headless.rs`; queue acceptance is not display confirmation. The [Windows title API](https://learn.microsoft.com/en-us/windows/console/setconsoletitle) owns physical host behavior.

Focused checks cover Unicode/control bounds, replay/focus/expiry/capacity traces, independent Rust/TypeScript/JSON conformance, real Windows CLI pipe receipts, cancellation, lifecycle restoration/failure and grammar/help/completion. These checks do not replace final provider, multi-Client, physical terminal or full-regression acceptance.
