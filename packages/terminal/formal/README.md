# Terminal correctness evidence

This directory owns bounded formal evidence for concurrency, independently ordered work, and
client input repair in `@orchbee/terminal`. The models abstract only client-owned
projected-terminal state; Terminal Host and its PTY/Ghostty authority are outside this directory.

## Renderer initialization retirement

`tla/TerminalRendererInitialization.tla` models initial WebGPU setup, visibility restoration, and recovery as ordered
adapter, device, pipeline, validation, preparation, and backend-publication phases. One renderer
lease owns one detachable device-loss subscription. Device loss retires that generation before
recovery starts. Document hiding parks a ready surface or retires pending initialization, and showing
only returns parked state to initialization eligibility; it does not publish a backend. `freeze()`
and `dispose()` can occur independently with an external promise pending. Every completion fence
either advances a visible, still-live initialization or cancels it without publishing WebGPU or
Canvas fallback ownership.

| Renderer claim                                     | Model property                                                             | TypeScript refinement                                                                                                    |
| -------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| A retired initialization cannot publish WebGPU     | `PublicationRequiresLive`                                                  | `WebGPURenderer.initializeGPU` checks retirement after each await and before context acquisition/configuration.          |
| A retired failure cannot activate Canvas fallback  | `FallbackRequiresLive`                                                     | `activateFallback` rejects frozen and disposed renderers, including failures that settle after lifecycle retirement.     |
| Cancelled initialization retains no backend        | `OwnershipMatchesPhase`, `BackendMatchesPublication`                       | The retirement fence releases partially created GPU resources, its runtime lease, and its device-loss subscription.      |
| Device loss retires the previous generation        | `LossRetiresGeneration`, `OwnershipMatchesPhase`                           | The shared runtime detaches the generation's subscribers before each live renderer releases its old lease and recovers.  |
| A hidden WebGPU surface owns no GPU residency      | `HideReleasesWebGPU`, `OwnershipMatchesPhase`, `BackendMatchesPublication` | `setDocumentHidden(true)` unconfigures the canvas and releases per-surface resources, its lease, and loss subscription.  |
| Hiding retires pending initialization              | `HideCancelsPendingInitialization`, `OwnershipMatchesPhase`                | The visibility epoch fences every await; a stale completion releases its lease and cannot configure or fall back.        |
| Showing cannot publish a backend by itself         | `ShowPublishesNothing`, `BackendMatchesPublication`                        | A visible retry must reacquire and validate the current shared generation before `useWebGPU` becomes true.               |
| Disposal retains no renderer backend ownership     | `DisposedOwnsNothing`, `BackendMatchesPublication`                         | `dispose()` releases its runtime lease, detachable device-loss subscription, GPU resources, atlas backing, and contexts. |
| Freeze preserves only an already-published surface | `FreezePreservesPresentedBackend`, `BackendMatchesPublication`             | A completed renderer keeps its last painted canvas; an in-flight initialization can only cancel after freeze.            |

Required `CoveragePublishFallback` and `CoverageHardFailure` witnesses keep the
fallback-publication and strict-failure output branches independently reachable. The model treats
browser and GPU promise settlement as independently ordered environment actions.
It proves only the bounded ownership and publication safety invariants above. The deterministic
renderer and `ProjectedTerminal` tests refine retained CPU atlas state, shared-device reference
counting, full resource reconstruction, and exactly one newest-state restoration draw even when
explicit demand remains paused. The model does not claim that an adapter, device, or driver
operation eventually completes, that physical GPU memory is reclaimed synchronously, or that every
browser implements identical context-retention behavior.

## Runtime presentation configuration

`tla/TerminalPresentationConfiguration.tla` models two independently settling presentation-
configuration requests. Underlying preparation phase and caller-visible operation outcome are
separate. Supersession or disposal immediately terminalizes every eligible pending or ready caller
operation as `cancelled` without claiming already-started font work stopped. A late completion or
failure retires only that work and cannot change the cancelled outcome or publish. Only the latest
eligible ready request can publish all eleven presentation fields in one commit.

| SU-14 claim                                    | Model property                                             | TypeScript refinement                                                                                                                       |
| ---------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| A stale request never commits                  | `NoStaleCommit`                                            | `ProjectedTerminal.updatePresentationConfiguration` checks its request epoch after font preparation and before renderer preparation/commit. |
| Configuration publication is complete          | `NoPartialPublication`                                     | `WebGPURenderer.commitPresentationConfiguration` swaps one complete prepared value.                                                         |
| Revoked work cannot publish                    | `CancelledCannotPublish`, `EligibilityConsistent`          | Superseded and disposed font work may settle later, but epoch and disposal checks return typed cancellation before renderer mutation.       |
| Superseded/disposed callers cancel immediately | `SupersededCallerIsCancelled`, `DisposedCallerIsCancelled` | The production cancellation race resolves the caller before external preparation settles.                                                   |
| Late work cannot revise caller outcome         | `LatePreparationCannotChangeCallerOutcome`                 | Stale font completion or failure retires only the work.                                                                                     |
| Cancellation preserves the prior configuration | `CancellationPreservesPublication`                         | Retired completion or failure and disposal leave the committed configuration and all published fields unchanged.                            |
| Supersession preserves the prior configuration | `SupersessionPreservesPublication`                         | Starting the newer request revokes old commit eligibility without changing the active presentation.                                         |
| Preparation failure preserves the prior config | `FailurePreservesPublication`                              | Font preparation failure returns a typed failure before renderer mutation.                                                                  |
| No mutation occurs before commit               | `PublicationChangesOnlyOnCommit`                           | Validation, font preparation, cancellation, and renderer preparation leave committed and published presentation state unchanged.            |
| The newest completing request commits          | `LatestReadyEventuallyResolves`                            | With a fair commit scheduler, a ready request commits unless a newer request or disposal cancels eligibility.                               |

The TLC configuration checks two ordered requests and all eleven published field identities,
including `terminalPadding`. Field values are abstracted to the request epoch because the
deterministic validation and renderer tests own concrete values and measurements. The model keeps
stale font work in flight until an independent completion or failure transition retires it without
changing the already-cancelled caller outcome. TLC uses weak fairness only for `Commit`; external
font completion, request arrival, and disposal remain independently ordered and are not assumed
fair.

## Image-preview freshness

`tla/TerminalImagePreview.tla` models the reversible fine-pointer image-preview workflow with two
targets and two independently completing filesystem requests. One hover epoch abstracts the exact
attachment, generation, revision, link epoch, row, column, path, and geometry membership checked by
the runtime. Admitted request identities remain as ghost state after settlement, so an explicit
repeated completion over a consumed identity can publish nothing. Replacing or retiring hover
membership clears the current request and card immediately without claiming that an already-started
operating-system read stopped. The stale read remains in the model until an independent completion
discards it.

| Image-preview claim                              | Model property                                                                                                 | TypeScript refinement                                                                                                   |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Local capability is required                     | `NoPresentationWithoutCapability`                                                                              | The projected surface enables file hover and activation only for a Desktop-local attachment capability.                 |
| One loading request belongs to the latest hover  | `CurrentRequestIsLatest`                                                                                       | The preview controller captures the complete revision-pinned target and one monotonically increasing request identity.  |
| Visible bytes belong to the current hover        | `VisibleRequiresCurrentHover`, `PublicationRequiresCurrentAuthority`                                           | Completion revalidates attachment, target, link context, request identity, and controller epoch before publication.     |
| Stale or repeated completion cannot publish      | `StaleCompletionCannotPublish`, `RepeatedCompletionCannotPublish`, `PublicationChangesOnlyOnCurrentCompletion` | Replacement, cancellation, disposal, and a consumed request identity cannot publish bytes.                              |
| Every retirement removes reversible presentation | `RetirementClearsPresentation`                                                                                 | Leave, pointer/keyboard/wheel intent, selection, geometry/revision change, capability revocation, and disposal hide it. |
| One request cannot publish more than once        | `AdmissionIdentityAccounting`, `AtMostOnePublicationPerRequest`                                                | A correlated reply consumes its pending request before the display state can change.                                    |

`LeaveHover` and `RetireHover` are semantic retirement transitions rather than coverage-only
self-loops. A synchronous preview-dispatch throw has no interleaving point and refines to the
abstract `DwellElapsed` followed immediately by `FailCurrent`; its admitted ghost identity cannot
publish. The checked bounds are two targets and two request identities. The model has no
filesystem-completion liveness property: local, removable, and network-backed filesystems may delay
or never complete at the abstraction boundary. It checks only bounded safety under replacement,
cancellation, capability revocation, disposal, and reordered or repeated completion. Raster magic,
the 8 MiB byte limit, path normalization, DOM geometry, decode support, and pointer timing remain
deterministic TypeScript, Desktop adapter, and headed-system evidence.

## Input repair

`tla/TerminalInputRepair.tla` models native composition lifetime, local text/preedit publication
eligibility, settlement, transient input presentation, event-specific single-record deduplication,
repair consequences, and disposal. A settlement guard is separate from ordinary correlations, so
repair clears correlations without forgetting native work that may still emit an end or a
composition-origin `beforeinput`. A new composition can supersede cancelled work before its old end;
independent key, focus, and idle `beforeinput` progress does not wait for that end. The runtime uses
the available browser facts (`isComposing`, `insertCompositionText`, and
`insertFromComposition`) to gate delayed composition-origin input. Browser events carry no native
composition identifier, so a new `compositionstart` establishes ownership of the next end.

| IN-02 claim                                              | Model property                                                                                                                                          | TypeScript refinement                                                                                                               |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| One composition publishes text at most once              | `AtMostOneCompositionPublication`                                                                                                                       | Both composition orders publish state before `ProjectedTerminal` calls its synchronous port.                                        |
| Every Host input attempt carries its attachment revision | `HostInputAttemptUsesCurrentCapabilityRevision`, `CapabilityRevisionStampChangesOnlyOnInputAttempt`, `RepairReopenReconcilesFocus`                      | `ProjectedTerminal` stamps key, text, paste, mouse, focus, and repair commands with the positive revision supplied at construction. |
| A reentrant downgrade cancels the old send authority     | `CapabilityDowngradeCancelsInFlightSend`, required `CoverageCapabilityDowngradeDuringSend` coverage                                                     | `applyInputCapability` can revoke input inside `port.send`; the post-send ownership commit rechecks current authority.              |
| Publication has explicit eligible or idle authority      | `CompositionPublicationRequiresEligibleAuthority`, `EveryPublicationHasAnExplicitOrigin`                                                                | History snapshots check each counter increase against its prior phase and named publication action.                                 |
| Ineligible updates cannot publish preedit                | `IneligibleUpdateCannotPublishPreedit`                                                                                                                  | `terminalCompositionUpdateIsEligible` gates every renderer preedit update.                                                          |
| Repair retains settlement while clearing correlations    | `RepairRetainsSettlementGuard`, `RepairClearsTransientInput`                                                                                            | Repair clears textarea/preedit and ordinary records but retains or creates the separate settlement guard.                           |
| A deduplication record is consumed by its paired event   | `DedupCorrelationIsSingleUse`, required `ConsumeKeyBeforeInputDuplicate` coverage                                                                       | Explicit key-beforeinput, composition-beforeinput, beforeinput-end, paste, and mouse transitions consume records.                   |
| Stateless input never becomes held ownership             | `StatelessPublicationDoesNotChangeHeldOwnership`                                                                                                        | Synthetic editing keys and wheel button reports bypass persistent key and pointer-button ledgers.                                   |
| Host ownership commits only after accepted sync send     | `HostOwnershipChangesOnlyAtPublication`, `FailedSendPreservesPublishedOwnership`                                                                        | Host-published ledgers change only after synchronous `port.send` return; a throw reports failure without committing that change.    |
| Every Host input publication waits for focus alignment   | `InputPublicationRequiresFocusConvergence`, `TextPublicationRequiresFocusConvergence`, `CurrentStagedContinuationHasAuthority`                          | Key, mouse, stateless, composition, and text publication share the focus-reconciling command path.                                  |
| Failed presses cannot create orphan releases             | `FailedPressRollsBackLocalOwnership`, `OrdinaryReleaseRequiresPublishedOwnership`                                                                       | Failed physical key and pointer presses roll back local staging and correlation; an unheld release is inert at publication.         |
| Repair releases keys, then mouse, then optional focus    | `CleanupReleaseRequiresPublishedOwnership`, `RepairConsequenceOrder`, `FocusOutRequiresPriorOwnership`, `FocusOutIsConditional`                         | Cleanup snapshots Host-published ownership and attempts each ordered consequence once per transaction.                              |
| Lifecycle repair bypasses a stalled review               | `LifecycleInvalidationDetachesQueuedGeneration`, `CancelledReviewDoesNotBlockCurrentGeneration`, `RepairEventuallyClosesAdmission`                      | Lifecycle invalidates the dispatcher generation before cleanup and never waits for paste review settlement.                         |
| Review rejection cannot stall the current generation     | `CancelledReviewResolutionIsInert`, `CancelledReviewDoesNotBlockCurrentGeneration`                                                                      | Current DOM rejection is reported; invalidated resolution or rejection cannot publish paste or queued input.                        |
| Reset reconciles retained local focus                    | `ResetNeverPublishesFocusOut`, `RepairReopenReconcilesFocus`                                                                                            | Reset retains DOM focus and begins a fresh focus-in transaction when an invalidated generation never published it.                  |
| An unfocused disposal completes without focus-out        | `UnfocusedDisposeRequiresNoFocusOut`, `DisposalEventuallyCompletes`                                                                                     | Disposal after completed blur is accepted without an intervening refocus/resume and does not duplicate focus-out.                   |
| Repair/disposal rejects synchronous input reentry        | `InputEventsRequireOpenAdmission`, `RepairAndDisposeCloseAdmissionFirst`                                                                                | `ProjectedTerminal` closes its admission gate before clearing state and reopens it only after the cleanup callback unwinds.         |
| A post-send continuation cannot outlive its authority    | `CurrentStagedContinuationHasAuthority`, `RepairInvalidatesCurrentContinuation`, `CurrentContinuationRequiresAuthority`, `StaleContinuationOnlyCancels` | The adapter checks both repair epoch and staged composition state before clearing textarea or preedit after synchronous send.       |
| A pending focus-out survives intermediate stages         | `PendingFocusOutIsNotErased`, `PendingFocusOutEventuallySettles`                                                                                        | Composition settlement cannot interrupt release ordering or erase a required focus-out attempt.                                     |
| Disposed input accepts no later event                    | `DisposedAcceptsNothing`                                                                                                                                | Disposal closes admission before one ordered cleanup attempt, then removes listeners and terminal-owned DOM.                        |

`compositionPublicationCount` deliberately has the checked domain `0..2`; the at-most-one result is
therefore a transition invariant rather than a consequence of `TypeOK`. The prior-phase,
prior-settlement, prior-publication, prior-focus, prior-admission, and prior-staged-continuation
snapshots make publication, settlement-retention, focus-ownership, admission, and stale-continuation
invariants observe the transition that just occurred. They replace error flags whose assignments
could have made a property algebraically unreachable. The model keeps one abstract pre-send
continuation separate from its current authority. Starting repair changes a current continuation to
stale; later resolution can only discard it. A newly staged continuation after admission reopens is
independent.

`capabilityRevision` starts at `1` and is bounded through `MaxCapabilityRevision`; the checked
configuration permits the downgrade to revision `2` and upgrade to revision `3`. Replacement is
the only transition that advances it, authority must alternate, and every ordinary, queued,
composition, focus, stateless, or repair input attempt records the current revision. Reopening
admission has separate no-command and focus-command branches, so only the latter records a
`focus-in` attempt. A valid downgrade may re-enter an active synchronous send. It atomically
cancels that old-revision transaction, continuation, and review work and clears local and published
composition, textarea, key, mouse, and focus ownership; its later completion cannot restore them.
Read-only states admit no Host input attempt. Upgrade changes only authority and revision, so no
pre-downgrade work can replay. Malformed, stale, replayed, or non-advancing capability updates and
the committed-versus-local admission split remain deterministic `ProjectedTerminal` test evidence;
this model owns valid advancing replacements. Host-side mutation, target-attachment repair, and
stale-input rejection remain independently checked by `TerminalInputBackpressure.tla`.

Explicit repair stages model transient clearing, key release, mouse release, conditional focus-out,
and admission reopening. `localKeyPressed`, `localMousePressed`, and `localFocused` represent
admitted desired ownership; `keyPressed`, `mousePressed`, and `focused` represent ownership already
published to the Host. `reviewStalled` and `queuedOwnershipInput` belong to the current dispatcher
generation. The finite TLA+ queue abstraction covers one representative queued ownership item plus
the two-item `focus-in` then key-press order needed to expose focus overtaking. Arbitrary queue depth
and queued text payload behavior remain `OrderedInputDispatcher` and `ProjectedTerminal` test
evidence. Lifecycle invalidation clears queued work, preserves a separately modelled cancelled review
only as inert work, and starts cleanup from Host-published ownership. Thus a queued release cannot
hide a required release, a queued press cannot cause an orphan release, and cleanup progress does
not depend on review resolution or fairness. Stateless sends model synthetic editing keys and
wheel reports without changing held ownership. Successful synchronous send completion alone
updates Host-published ownership. Failed physical presses roll back local staging and their
correlation; failed releases and focus-out retain prior Host ownership. Repair still advances after
a failed cleanup send because the runtime reports and attempts each consequence at most once per
cleanup transaction; the model does not claim delivery or ownership clearance after that failure.
The cleanup ledgers use one representative key and one representative mouse button, so TLA+ proves
cross-class key-before-mouse-before-focus ordering. Plural identity completeness and within-class
press order remain deterministic `ProjectedTerminal` test evidence.

`focused` and `needsFocusOut` distinguish focused and already-unfocused repairs. A disposal may
start from the completed focus-repair stage without first reopening input, matching synchronous
disposal requested from a focus-out callback. A separate pending-disposal path closes admission
immediately during an earlier release callback, adds any required focus-out to the already-owned
cleanup, and completes disposal only after that cleanup. A focus repair that arrives while reset
owns the transaction changes the reset consequence to conditional focus-out; reset alone retains
focus. Reopening a reset repair starts focus reconciliation in the fresh generation when DOM focus
was retained but the invalidated generation did not publish it. Weak fairness applies only to
already-started synchronous send completion and repair consequence stages, establishing admission
closure, ordered cleanup-attempt progress, pending focus-out settlement, and disposal completion;
it does not assume a send succeeds, browser settlement arrives, cancelled review resolves, or a
stale continuation runs. The correlation set abstracts one replaceable record per source kind. The
key scenario creates and consumes the key-to-`beforeinput` record in one reachable trace; concrete
text identity and matching time remain test-owned.
The strict 99/100-millisecond boundary is owned by the pure TypeScript
tests. They strictly parse and replay the record/check fields of the pinned mature trace, which is
versioned in
[`../docs/input-repair-conformance-v0.0.1.json`](../docs/input-repair-conformance-v0.0.1.json).
The headed `terminal-parity/IN-02-composition` witness drives real `ProjectedTerminal` DOM and
Canvas adapters with synthetic `CompositionEvent`, `KeyboardEvent`, and `InputEvent` instances. It
checks composition repair plus Shift-punctuation native-text preference and fallback at the
application-layer browser adapter, together with completed Canvas frames. It does not exercise a
native operating-system IME candidate UI.

## Row-flight coordination

`tla/TerminalRowFlightCoordination.tla` models the independently ordered viewport, search, and
general-buffer row-read lanes. The viewport lane represents protocol-admitted `current-view`
requests; search and general-buffer lanes retain exact-revision semantics. Each lane admits at most
one request, all three may coexist, and a request-sequence-correlated response settles only its
owning lane. The viewport scheduler also models coalesced user motion, valid-anchor maintenance
refresh, and missing-anchor recovery. A response at the current observed history revision or newer
commits rows and supersedes pending refresh or recovery. An older response still commits its rows
but preserves newer recovery, which blocks later viewport admission until recovery settles. The
checked bound admits two serial viewport requests, one request in each auxiliary lane, and two
revision advances. The fixed three-lane domain encodes the total representation bound; it is not
cited as an independent cardinality theorem.

| Row-flight claim                                         | Model property                                                                                                                        | TypeScript refinement                                                                                                               |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Each purpose remains single-flight                       | `AtMostOneFlightPerLane`                                                                                                              | `ProjectedTerminal` stores one viewport, search, and general-buffer flight.                                                         |
| Independently ordered replies cannot cross               | `InFlightSequencesAreDistinct`, `CrossLaneSettlementIsolation`                                                                        | `applyRows` selects the exact lane by decoded request sequence before settling or applying its result.                              |
| Revision churn cannot invalidate viewport work           | `RevisionAdvancePreservesFlights`, `EveryAdmittedFlightEventuallySettles`                                                             | A same-generation current-view response may commit after newer Host output; a correlated refresh then coalesces.                    |
| Maintenance refresh cannot preempt queued user motion    | `RefreshAdmissionNeverPreemptsQueuedUserIntent`, `UserAdmissionAbsorbsMaintenanceRefresh`                                             | Once the current viewport flight settles, eligible explicit motion is admitted before a valid-anchor `delta: 0` refresh.            |
| A queued user intent is eventually consumed or cancelled | `EveryQueuedUserIntentEventuallyConsumesOrCancels`                                                                                    | Admission consumes it; invalid-anchor recovery discards pending coordinates; disposal cancels the remaining authority.              |
| Viewport response age controls pending history work      | `ViewportResolutionSupersedesPriorAnchorWork`, `OlderViewportResolutionPreservesNewerRecovery`, `ViewportAdmissionRequiresNoRecovery` | Same-or-newer responses clear pending history work; older responses preserve authoritative later recovery before another admission. |
| Exact row results never cross a revision                 | `ExactRowsResolveOnlyAtCapturedRevision`                                                                                              | Search and general-buffer responses still require the captured generation and revision.                                             |
| Abort, disposal, and ordinary completion are local       | `CrossLaneSettlementIsolation`, `DisposalClosesEveryLane`                                                                             | Auxiliary aborts capture their lane and sequence; disposal closes all lanes without promoting viewport work.                        |

The model assumes no Host response deadline or guaranteed successful transport. Its liveness proof
does assume that continuously enabled admitted work eventually receives the model's consistency-
appropriate settlement unless failure, abort, or disposal retires it. Weak fairness also applies to
enabled user or maintenance viewport admission and enabled invalid-anchor recovery; it does not
impose an order among lanes. The schema restriction to viewport plus `tail` or `committed`,
generation invalidation, byte bounds, rendering, and correlated refresh are covered by protocol and
TypeScript tests.

## Request-publication recovery

`tla/TerminalRequestPublicationRecovery.tla` models the local publication boundary after
`ProjectedTerminal` has installed correlation state for a viewport row read, an exact buffer row
read, or an asynchronous link effect. Its trusted inputs are terminal-owned lane and request
sequence identities. The command port's synchronous return, throw, or refusal, its independently
ordered Promise rejection, Host completion, a later explicit request, queued viewport intent, and
lifecycle disposal are external events. A void or fulfilled send is accepted publication with no
additional semantic output; an absent Host completion remains in flight.

The terminal owns each lane's current request and resolver. Publication refusal settles the staged
request immediately. An accepted publication may later complete through the Host or reject through
the command-port Promise. Both failure paths compare the captured request sequence with the current
lane before retiring anything. Disposal cancels current work, while a late rejection after Host
completion, disposal, or readmission is correlation-state inert. Buffer-row cancellation remains
owned by the row-flight model: a later send rejection may release its retained lane but cannot
replace the already returned `cancelled` result.

| State/event                      | Guard and next state                                                                                      | Observable output                                                                 |
| -------------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `idle` or `settled` / admit      | The lane is below its bound; install a fresh sequence in `publishing`.                                    | No result yet.                                                                    |
| `publishing` / accept            | The command port returned; retain a rejectable send and enter `in-flight`.                                | No result yet.                                                                    |
| `publishing` / refuse            | The port returned false or threw; retire only that staged sequence and enter `settled`.                   | Buffer `unavailable`, retryable `link-send-failed`, or history recovery.          |
| `in-flight` / Host completion    | The completion matches the current sequence; enter `settled`.                                             | Rows or completed link effect.                                                    |
| `in-flight` / Promise rejection  | The rejection matches the current sequence; enter `settled` without creating another admission.           | The lane-specific failure above.                                                  |
| any / stale Promise rejection    | Completion, disposal, or readmission made the captured sequence noncurrent.                               | No correlation-state change; command diagnostics remain outside this abstraction. |
| settled viewport / queued intent | Admit the separately queued user intent with a fresh sequence; without one, remain settled or recovering. | New work, never a retry of the failed request.                                    |

| Publication-recovery claim                          | Model property                                                                                                                                                | TypeScript refinement                                                                                                      |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| A reported current send failure settles one request | `CurrentPublicationFailureSettlesExactRequest`, `SettledRequestHasTypedOutcome`                                                                               | Buffer rows resolve `unavailable`; async links resolve `link-send-failed`; viewport rows enter the existing recovery path. |
| Failure never retries the failed command            | `PublicationFailureDoesNotRetry`, `PendingViewportAdmissionConsumesOnlyQueuedIntent`                                                                          | Cleanup removes the exact flight/completion; only a distinct caller or queued viewport intent may publish again.           |
| A stale rejection cannot clear newer work           | `StalePublicationFailureIsInert`, `CrossLaneSettlementIsolation`, `CurrentSequencesAreDistinct`, plus required completion/disposal/readmission stale coverage | Every failure callback rechecks its captured request sequence; generic command diagnostics remain observable.              |
| Later work receives a fresh identity                | `ReadmissionUsesFreshSequence` plus required retry/readmission action coverage                                                                                | A retired lane admits a later read or link request through the existing monotonic sequence allocators.                     |
| A settled queued viewport intent makes progress     | `EverySettledViewportIntentEventuallyDispatches` under weak fairness of enabled intent admission                                                              | Existing history intent priority runs only after the failed viewport flight is retired.                                    |
| Disposal remains authoritative                      | `DisposalClosesEveryLane`                                                                                                                                     | Freeze/disposal clears row flights and link completions; later rejection callbacks are no-ops.                             |

`SettledRequestHasTypedOutcome` is a literal lane-to-result-set oracle independent of the helpers
used by settlement transitions. Required stale witnesses separately preserve the completion,
disposal, and changed-sequence readmission causes.

The checked bound admits two requests per lane, permits all three lanes to overlap, and retains an
old accepted send while a newer request becomes current. It does not assume transport or Host
completion liveness, prove delivery, retry commands, model row bytes or link policy, or change the
Host/protocol contract. Deterministic TypeScript tests own concrete synchronous throws, native
Promise rejections, frozen-port refusal, typed results, history compensation, and re-entry.
Cancellation settlement and lane retention are checked by the separate row-flight model and
existing abort tests.

## Search-refresh scheduling

`tla/TerminalSearchRefreshScheduling.tla` models exact-revision search admission during output
churn. A scheduled scan keeps one admission deadline while revisions advance. Once admitted, a scan
keeps its captured revision until it settles: a current result publishes, while an obsolete result
publishes nothing and schedules one replacement. Revision invalidation after a completed result
also schedules one replacement. Query replacement, invalid-query rejection, Worker failure,
watchdog timeout, page streaming, and directional navigation remain outside this scheduling model.

| Search-refresh claim                             | Model property                                                                    | TypeScript refinement                                                                                                                                         |
| ------------------------------------------------ | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Revision churn cannot postpone a scheduled scan  | `ScheduledInvalidationPreservesAdmission`, `EveryScheduledSearchEventuallyStarts` | `ProjectedSearchController.invalidate()` leaves an existing debounce deadline unchanged.                                                                      |
| Revision churn cannot cancel an admitted scan    | `ScanningInvalidationDoesNotCancel`, `EveryAdmittedScanEventuallySettles`         | An in-flight scan settles against its captured generation and revision; invalidation records only one pending replacement.                                    |
| Only a current exact-revision result can publish | `CurrentResultsOnly`, `ObsoleteResultsNeverPublish`                               | Worker completion rechecks the row-reader snapshot; stale or evicted pages and superseded results retire silently before one fixed-delay refresh is admitted. |

The checked configuration covers three revisions and three serial scan attempts. Weak fairness
assumes an enabled timer callback and consistency-appropriate settlement of an admitted scan; it
does not assume a Host response deadline or a quiet revision interval. Deterministic controller
tests own the 50-millisecond query debounce, 200-millisecond refresh delay, navigation replay,
cancellation, and watchdog boundaries. The generalized module names exhausted finite exploration
capacity `bound-exhausted`; this is not a product search error. It is intentionally unreachable in
the registered equal three-revision/three-scan configuration and becomes reachable only when a
discovery configuration gives revisions more room than scans. Worker failure and watchdog timeout
remain excluded.

## Prompt-navigation commit authority

`tla/TerminalPromptNavigation.tla` models two replaceable previous/next prompt-navigation requests,
an independently advancing Host revision, a separately advancing local viewport-intent epoch,
target discovery, cancellation, and disposal. A discovered target belongs to one request epoch and
can publish a viewport change only while both captured authorities remain current. Replacement
starts a fresh request without retaining the older target. Weak fairness applies only after a
target becomes ready, so a current target commits and a stale target retires; no Host row-read
deadline is assumed. The concrete adapter selects either an exact physical visible anchor or a
same-intent transient repair anchor before admission; the model treats that selected anchor as part
of the captured viewport-intent authority. A repair admission captures the latest Host revision and
does not preserve an older scan result.

| Prompt-navigation claim                            | Model property                                                  | TypeScript refinement                                                                                                                                                                       |
| -------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Only current source and viewport intent can commit | `CommitRequiresCurrentAuthority`, `ViewportChangesOnlyOnCommit` | `ProjectedTerminal` rechecks generation, revision, total rows, primary screen, history-intent epoch, and its selected exact-visible or same-intent repair anchor before revealing a target. |
| Replacement cannot publish an older target         | `ReplacementStartsFresh`, `TargetBelongsToCurrentRequest`       | Replace-latest Actions abort the older general-buffer read; each invocation owns one captured scan and commit guard.                                                                        |
| Cancellation and disposal cannot commit            | `TerminalPhaseCannotCommit`, `ViewportChangesOnlyOnCommit`      | An aborted row read returns cancellation and disposal settles auxiliary flights before any reveal.                                                                                          |
| One invocation publishes at most once              | `AtMostOneCommitPerAdmission`, `TargetReadyEventuallyRetires`   | The prompt locator returns one target and the owning method issues at most one retained-history start-row intent.                                                                           |

The state vector omits prompt-row contents, direction, detailed outcomes, pagination, and the 16 MiB
source budget; separate previous/next action coverage preserves branch reachability, while
deterministic prompt-navigation tests own the omitted decisions. It does not claim transport
delivery, automatic shell integration, alternate-screen navigation, or success when retained
history changes during the scan.

## Selection-gesture current-view coordination

`tla/TerminalSelectionGestureCoordination.tla` models one serialized pointer gesture that captures a
projection generation and carries viewport-relative endpoints. The client owns one in-flight
command and a bounded FIFO sequence of `{action, generation}` endpoint records. A later update or
cancel replaces a trailing queued update; end appends behind it, preserving the latest update before
release. Terminal Host resolves each dispatched endpoint against its current viewport or tracked
attachment anchor. Content revision is deliberately absent from gesture identity, while generation
remains a safety fence. Queued fresh starts are outside this gesture-scoped abstraction.

| Event                        | Guard                                        | Client transition and observable result                                                                                                |
| ---------------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Pointer start                | No prior gesture work                        | Capture the current generation, capture the pointer, and publish `start`.                                                              |
| Pointer move, end, or cancel | The local gesture owns a captured generation | Publish immediately only when neither a flight nor a queued endpoint exists; otherwise coalesce a trailing update or append behind it. |
| Revision advancement         | Output changes while a command is in flight  | Preserve the gesture flight and FIFO entries; settlement remains enabled.                                                              |
| Rapid release                | Start is in flight and update is queued      | Append end after update; FIFO dispatch publishes update while end remains retained.                                                    |
| Current Host acceptance      | The captured generation still matches        | Resolve the endpoint against current Host state; only an accepted `end` may request automatic copy.                                    |
| Generation rejection         | The captured viewport generation changed     | Retire all local gesture work, preserve Host selection, copy nothing, and emit no feature failure.                                     |
| Other rejection              | The serialized command is current            | Retain visible typed-failure behavior without automatic replay.                                                                        |
| Fresh pointer start          | Generation cancellation left no old work     | Start an independent gesture using the new current generation.                                                                         |

| Selection-gesture claim                          | Model property                                                                                             | TypeScript refinement                                                                                                          |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Every queued endpoint keeps the start generation | `GestureWorkKeepsCapturedGeneration`, `FlightIdentityIsComplete`, `DispatchIsFIFO`                         | Each retained record carries pointer-start generation and dispatch copies that record rather than restamping it.               |
| Rapid release preserves update before end        | `ReleaseRetainsQueuedUpdate`, required `CoverageRapidReleaseQueue`, `CoverageDispatchUpdateAheadOfRelease` | The deterministic rapid-release regression observes `start`, then `update`, then `end`.                                        |
| Revision churn cannot retire an admitted command | `EveryAdmittedFlightEventuallySettles`                                                                     | Revision-only history events preserve the published flight; queued callback progress remains deterministic scheduler evidence. |
| Generation cancellation leaves no old work       | `GenerationBarrierClearsGestureWork`, `GenerationBarrierRecordsCancellation`                               | A stale-generation reply clears the flight and queue, cancels scheduled dispatch and autoscroll, and releases pointer capture. |
| Generation cancellation preserves selection      | `GenerationBarrierPreservesSelection`, `GenerationBarrierDoesNotCopy`                                      | Repair sends no replacement endpoint or clear command, and a retired `end` cannot trigger copy.                                |
| Generation cancellation remains silent           | `GenerationBarrierDoesNotSurfaceFailure`                                                                   | Only an in-flight `stale-generation` gesture failure is absorbed; stale revision is not a valid pointer-gesture result.        |
| Other selection failures remain visible          | Required action coverage for `RejectSelection`                                                             | Existing `onError` delivery remains unchanged for non-generation failures.                                                     |

TLC checks one current pointer gesture, a FIFO endpoint queue, two generations, two revisions, and
bounded publication, selection, failure, and generation-cancellation counts. The existing four-
publication exploration budget also reserves one future publication for every retained entry; it is
not the production queue capacity. Weak fairness is limited to settlement of an admitted gesture
command. No queued-dispatch or browser-callback fairness is assumed. Required rapid-release
witnesses prove retained update-before-end order and head dispatch. The concrete 64-entry queue
bound, queued fresh starts, callback scheduling, viewport coordinate resolution, pointer/capture
effects, and generation failure mapping remain deterministic client and Host evidence. Concurrent
programmatic exact-revision selection, transport deadlines, Host text semantics, clipboard
delivery, and automatic retry remain excluded.

## Native input settlement

`tla/TerminalNativeInputSettlement.tla` owns the short native-text settlement window that is
separate from composition settlement in `TerminalInputRepair.tla`. It explores two consecutively
admitted requests, distinguishes unmodified and Shift-produced printable fallback requests, and
independently orders textarea-value changes, repeated queue attempts, fallback coalescing,
input-event deferral, timer firing, composition start/end, repair/reopen, and disposal. It also
represents a later Host-directed input as a publication transition that remains disabled until the
current native request has a terminal outcome.

| Native settlement claim                                  | Model property                                                                                                                                                                                                            | TypeScript refinement                                                                                                     |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| One admitted request has one terminal outcome            | `EveryAdmittedRequestSettlesOnce`, `PublicationIsExactlyOnce`                                                                                                                                                             | `#queueNativeInputChange` owns one pending request and clears it before timer publication.                                |
| Shift printable candidates retain a text fallback        | `ShiftedRequestsAreAdmittedFallbacks`, required `QueueShiftedFallback` coverage                                                                                                                                           | `shouldUseNativeTextInput` admits Shift punctuation and digits into the same settlement state machine.                    |
| Changed native text wins over the layout-text fallback   | `ActualValueChangesOnlyOnNativeValueChange`, `NativePublicationRequiresChangedValue`, `FallbackPublicationRequiresUnchangedValue`, `NoopRequiresUnchangedValue`, required `CoverageNativeValueReturnsToBaseline` coverage | The timer compares the settled textarea value with its captured baseline before choosing native text, fallback, or no-op. |
| A repeated keydown only enriches the current request     | `PendingIdentityIsCurrent`, `FallbackPublicationRequiresUnchangedValue`                                                                                                                                                   | A pending request retains its baseline and timer while the first non-empty fallback is coalesced.                         |
| An independent input event waits for the timer           | `DeferredInputDoesNotPublishEarly`, required `CoverageDeferredInputBeforeValueChange` coverage                                                                                                                            | `#consumeInputValue` returns before or after the textarea changes while native settlement is pending.                     |
| A later Host-directed input cannot overtake settlement   | `FollowingInputCannotOvertakeSettlement`                                                                                                                                                                                  | `#sendInput` synchronously settles a pending native request before publishing the later command.                          |
| Composition, repair, and disposal revoke timer authority | `CompositionOwnsNoSettlementTimer`, `ClosedAdmissionHasNoTimer`, `InvalidationCancelsPendingSettlement`, `StaleTimerIsInert`, required `CoverageStaleTimerAfterReadmission` coverage                                      | Cancellation clears current authority; an old timer identity/epoch is inert after repair and re-admission.                |
| An uncancelled timer eventually settles                  | `PendingTimerEventuallySettles`                                                                                                                                                                                           | Weak fairness applies only to the already-armed current timer; environment events may cancel it first.                    |

The model represents time by event ordering and never assumes that a slow machine fires at exactly
30 milliseconds. Its exact finite limits are two admitted requests, two repair-epoch advances, and
two textarea-value transitions. Two abstract textarea values provide an actual current-versus-
baseline comparison and include the changed-then-restored ABA trace; the monotonic generation is
only a transition bound. The timer actions snapshot that pre-action value relation. Independent
`beforeinput` may arrive on either side of the value change, remains suppressed, and receives no
separate eventual publication: the timer still owns the request's sole native, fallback, or no-op
outcome. Allocated timer identities may fire stale after cancellation and re-admission, but only the
exact current identity and epoch can settle the request.

The abstraction admits only a publishable nonempty changed value. Empty sealed text and values over
the concrete byte limit, plus the punctuation/digit, letter, Space, and Control/Meta/Alt admission
partitions, remain deterministic TypeScript evidence. The operating-system input method owns the
resulting Unicode mapping. Send failure and focus convergence remain owned by
`TerminalInputRepair.tla` and TypeScript tests. The separate zero-delay
`#nativeInputTextForNextKey` marker is also outside this model; unit tests cover input-before-keydown
within the marker's browser-task window, so no formal claim is made for arbitrary delay between
those two browser events.

Run the bounded model from the repository root:

```sh
pnpm --filter @orchbee/terminal test:tla
```

The shared repository runner executes the renderer-initialization, presentation, image-preview,
row-flight, request-publication-recovery, prompt-navigation, selection-gesture, and native-settlement models once and the input-repair model through nine fixed
semantic scenarios: composition, key, mouse, focus, review-key, review-mouse, review-focus,
review-composition, and lifecycle. `review-composition`
combines a stalled review, queued focus-in, reset invalidation, a deliberately failed first
fresh-generation focus reconciliation, a blocked composition publication attempt, a successful
reconciliation retry, and a later composition publication. `InputPublicationReady` guards only
Host-directed publication; composition start, update, cancelled-end settlement, and
stale-continuation discard remain local dispatcher actions and do not require Host focus
convergence.

[`../../../scripts/tla/models.json`](../../../scripts/tla/models.json) is the execution inventory.
The runner pins `tla2tools 1.7.4` by SHA-256, uses one worker, fingerprint polynomial zero, explicit
timeouts, action coverage, and a unique metadata directory beneath the ignored root `.tools` per run.
Variant configurations and model copies are transient and removed in `finally`. Each completed check emits a `TLA_RESULT`
record containing the model/config digest, duration, generated and distinct states, remaining
queue, and depth. Stable named coverage entries must exist and have a nonzero count across the
registered variants; source line numbers are not evidence. Set `ORCHBEE_TLA2TOOLS_JAR` to an
already provisioned jar with the pinned digest or let the runner provision the ignored root copy.

The inventory deliberately records bounds and executable coverage rather than dated state counts.
Current counts belong to the `TLA_RESULT` output for the exact model/config digest that produced
them. Passing these bounded checks is evidence only for the declared abstractions and fairness
assumptions; it is not proof of browser scheduling, native IME behavior, transport delivery, or an
unbounded production system.
