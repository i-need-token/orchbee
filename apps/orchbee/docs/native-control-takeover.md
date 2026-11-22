# Native controller takeover

## Ownership and activation

The Server publishes the current controlling Viewer and its live recent-focus order through
OBCS `Observe`. `Activate` records one foreground activation and that Viewer's content geometry;
it does not acquire or resize a terminal. The returned private grant, consumed sequence and
acknowledged geometry are included only for the observing attachment that actually owns them.
Every attached Client can read the public ownership and focus order.

Host focus gain and foreground pane changes arm one activation, including while the new pane's
controller mode is still unknown. The activation waits for attachment readiness and capture to
clear. It reads Server ownership before requesting Takeover. Verified self ownership at the same
geometry keeps the current frame, grant and input sequence without another Host acquisition,
projection reactivation or control-acquired notice. The first activation also preserves a fresh
startup grant, so initial pointer selection and drag do not acquire a redundant barrier.

Routed keyboard input and button presses may precede the emulator's FocusIn report. They supply
foreground evidence early enough to start the same activation and hold its first keys; the later
identical focus report cannot rearm it. Background pointer motion, wheel and host-query replies
supply no such evidence.

A host's FocusOut can go missing, for example between two emulator windows, so the next FocusIn
repeats a state the Client already holds. While the Client only views its focused terminal and
has no activation armed or settling, a FocusIn, a routed key or a button press is therefore a new
activation, and the Client the person is using takes the terminal's size. While it controls the
terminal, or an activation is armed or settling, repeated focus reports and input change nothing.
A program-tracked press in a terminal being claimed this way reaches no program and shows no
failure: the claim may resize the terminal and move what the press pointed at.

An activation submits at most one transfer. Refusal and uncertainty never rearm it; a later focus
activation is a new intent that may reconcile Server ownership. A takeover whose focus lost its
Viewer before the takeover's Worker lease ran sends nothing and settles as refused beside the
parked focus ([panel lifecycle](native-panel-lifecycle.md)). The Panel menu's take-control row
remains an explicit alternative; it has no default key. A known grant never regresses to unknown
within its operation. Exact operation, pane incarnation and original deadlines qualify receipts.

## Input and display

Input typed during an automatic activation has not reached the program. The Client holds at most
4,096 bytes for that exact pane until the operation settles and, for a new grant, the compatible
frame has been drawn and acknowledged. Focus loss, a different pane, capture, refusal, uncertainty,
retirement or the fixed deadline cancels those unsent bytes. No submitted input effect is replayed.
Explicit takeover retains its ordinary blocked-input refusal. Uncertain authority blocks input;
a new activation may read and reconcile it without resubmitting the previous operation.

A transferred or newly learned grant resizes to the requesting pane's content geometry, then
pauses and reactivates projection demand on the same Viewer. The first compatible draw and
feedback complete the handoff gate. Established full snapshots subsequently follow the Host's
actual grid, including a peer's different dimensions; deltas still require their exact base.
A former controller keeps viewing output. Ownership observations update local controller mode
without invalidating terminal content. A Client that received a startup grant or has activated
keeps reading these published facts through ordinary polling, so revocation and Server promotion
do not require failed typing. A startup grant taken before its holder's first activation
therefore leaves that holder a known Viewer, which its next focus, key or press can reclaim.

A confirmed pause retires the Host frame flight in Worker and Server relays as well. Its queued
projection bytes are released, exact old feedback becomes stale, and reactivation requires a full
snapshot with a newer sequence. History, lifecycle and other Viewers keep their independent state.

## Departure and recent focus

Server owns the recent-focus order for each exact Worker generation and Host terminal. When the
controller departs and its release fence settles, the preferred surviving Viewer may acquire on
its next poll. Admission rechecks both vacancy and that Viewer being the current preference under
the reservation gate. A competitor cannot be stolen from, and stale cleanup cannot release the
successor. Promotion restores the surviving Viewer's recorded content geometry. A pending or
uncertain release remains fenced rather than looking vacant.

The outer emulator owns operating-system window and tab focus. Server promotion does not move
that focus. An actual restored focus activation reads the already published owner, or requests a
transfer when needed. Terminal processes and viewing attachments survive these handoffs.

## Evidence and staged delivery

The [Server transfer model](../../orchbee-server/docs/terminal-controller-takeover.md) owns the
shared authority, including Viewer and grant cleanup; activation and MRU behavior stay with the
Client.

The GUI focus/control projection must consume the same published ownership and focus order and match
self-owner and recent-Viewer behavior. All four native binaries remain one cohort.
