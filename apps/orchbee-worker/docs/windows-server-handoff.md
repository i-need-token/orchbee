# Windows Server handoff

## Scope and ownership

A managed Windows Worker can replace its admitted Server connection without replacing the
Worker, its authorization kernel, Agent Drain watermarks, background job owners or Terminal Host
connection. This is an explicit two-phase operation issued by the current authenticated Server.
Ordinary EOF still retires the Worker; it never starts recovery or discovers another Server.
The Server owns canonical-state export/import, target executable/cohort compatibility, public
endpoint publication, client disconnection and the final live-handoff result. This transport slice
alone is not a public `server.live_handoff` implementation or proof of process handoff.

Live handoff disconnects clients, preserves terminal processes and restores the original owner after precommit failure.
Windows uses the already persistent Worker/Host instead of Unix descriptor transfer. Neither
cold checkpoint restore nor automatic reconnection represents this operation.

## Inputs and state

`WorkerHandoffRequest` echoes an operation ID, exact Worker ID/generation, action and
`WorkerHandoffTarget` (nonzero 128-bit handoff ID and 32-byte verified target cohort digest).
The active Server alone delegates this target. A codec-valid digest is not compatibility evidence.
The runtime owner derives one `handoff-<handoff-id>` named pipe scoped by the existing admitted
runtime hash; the request cannot name an arbitrary path or filesystem leaf. Same-user admission
plus an exact ticket/Worker/generation/cohort echo
authenticate the standby connection. The standby Server must validate its imported ownership
grant before emitting Ready. Ordinary Worker admission and cohort checks remain unchanged.

The Worker owns at most one Prepared connection, its 15-second lease, and at most 64 spent
handoff IDs over its process lifetime. Every dial attempt spends its ID; aborted, failed, expired
and committed IDs are never reused. Prepare's dial and handshake share one 2-second deadline.
Neither partial input nor output renews it. Expiry closes only the standby connection.
The normal 2-second record budget still bounds a record that began before an idle lease cutoff.

The Server wire owner serializes these requests in the existing high lane. Prepare fences normal
admission and returns known unavailability to queued, unwritten requests. An already dispatched
request finishes before Prepare. The higher Server owner must quiesce its canonical-state writers
and wait for their settlement before export; this queue fence does not replace that obligation.
Only matching handoff requests may enter the fenced queue. A known failed Prepare with no other
Prepared owner, successful Abort or NotPrepared restores admission; Busy and TicketMismatch retain
the fence because they cannot prove absence. A committed result closes the old queue permanently.
Unknown transport outcomes retain the existing complete-write distinction and never authorize replay.

## Transitions

| State    | Event and guard                                                                                      | Output and next state                                                                                                      |
| -------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Active   | Prepare; managed, exact Worker, no viewers/controllers, fresh ID, capacity                           | Dial derived endpoint; exact Hello/Ready before deadline -> Prepared                                                       |
| Active   | Invalid identity, unmanaged, busy attachments, spent ID, exhausted capacity or failed dial/handshake | Typed refusal; preserve active Server and all physical owners                                                              |
| Prepared | Prepare or different ticket                                                                          | Busy or TicketMismatch; preserve original Prepared owner                                                                   |
| Prepared | Matching Abort                                                                                       | Close standby; Aborted -> Active                                                                                           |
| Prepared | Lease expires                                                                                        | Close standby -> Active; a later Commit is NotPrepared                                                                     |
| Prepared | Matching Commit before expiry                                                                        | Complete Committed result on old wire; swap the single owned Server stream; send Owned on new wire -> Active on new Server |
| Prepared | Non-handoff request                                                                                  | Refuse protocol continuation before dispatching any effect                                                                 |
| Active   | Ordinary EOF, partial/malformed record or Host loss                                                  | Existing retirement; never infer handoff from loss                                                                         |

Before the complete Committed write, failure drops the standby without switching authority.
That write must finish before both the prepared lease and the normal record deadline; neither
receiving a Commit nor writing fragments renews the lease.
After that write, the new wire is the sole connection; failed Owned delivery is outcome-unknown
and cannot reconnect to the old Server. The importer claims ownership only after exact Owned.
The old Server's Committed receipt is a transport commit boundary, not the final public success.
Successful swaps preserve the exact Worker generation, Host epoch, creation IDs, terminal IDs,
kernel fences, counters, titles, Agent Drain and job handles. No Host startup/shutdown or terminal
Launch/Start/Resize/Input/Dispose is issued by handoff.

## Evidence and limits

The closed fixed-size codec is checked with independent golden bytes, every action/outcome,
strict prefixes, extra bytes, zero identities/generation and contradictory action/outcome refusal.
Actual Windows named-pipe traces check authentication mismatch, Prepare/Abort/expiry rollback,
old/new wire selection, an exact Owned receipt and preserved physical ownership. Carrier tests
check fences, queued-request settlement, correlation, normal-admission recovery and committed
retirement. Deadline tests distinguish idle expiry from an incomplete record.

These checks establish this transport boundary. Canonical lossless import, exact cohort/executable
admission, retained process handles, public endpoint/activation-lock transfer, running ConPTY
survival, final CLI parameters and crash recovery require Server integration and composed evidence.
No independent review, full regression, real release upgrade or real terminal visual acceptance
is implied by these focused checks. This boundary starts no service or background thread; the
existing Worker loop owns standby cleanup and the existing carrier owns request settlement.
Admission retains the existing same-user native development trust boundary; a malicious same-user
process and process crashes are outside this transport model.
