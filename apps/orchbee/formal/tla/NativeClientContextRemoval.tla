-------------------- MODULE NativeClientContextRemoval --------------------
\* Checked finite model of the Client context-removal recovery: another Client's
\* workspace close deletes this session's current context from the canonical
\* snapshot. The session must classify the deletion, mint a one-flight
\* ContextRemovalPermit, dispose every still-attached pane viewer of the removed
\* epoch through the real membership cleanup (Detach before install, never
\* install-then-clean), and land on the surviving standalone context. No wire,
\* phase or reconnect additions are modeled; every failure retires honestly.
EXTENDS Integers, FiniteSets

CONSTANTS MaxTime
ASSUME MaxTime = 8

\* Two distinct context identities: the removed workspace context and the
\* surviving standalone context. Two pane records bound the removed epoch.
Contexts == {"removed", "landing"}
Panes == {"p1", "p2"}
LivePhases == {"ready", "swa", "rau", "empty"}
Phases == LivePhases \cup {"reconciling", "retired", "recovering",
                          "landed-ready", "landed-empty"}
\* Refresh-flight generations: 1 is the ordinary flight, 2 the retired attempt,
\* 0 stands for no armed flight and for stale/late generations.
Gens == {0, 1, 2}

VARIABLE s
vars == <<s>>

Initial(phase, panes, attached, dirty, cleanProvenance, cleanGuards,
        landingTab, retained) == [
  phase |-> phase,
  location |-> "removed",
  panes |-> panes,              \* records of the removed epoch
  attached |-> attached,        \* records still carrying a viewer
  detached |-> {},              \* panes with canonical Detached proof (monotone)
  dirty |-> dirty,              \* unattached records still flagged projected/staged/accepted
  staged |-> FALSE,             \* exact membership cleanup armed
  cleanupSettled |-> FALSE,     \* cleanup consumed (install or failure)
  permit |-> "none",            \* none | carried | spent
  permitGen |-> 0,
  flight |-> 0,
  classified |-> "none",
  removalAttached |-> {},       \* attached set snapshot at permit mint
  removalDirty |-> {},
  desired |-> IF phase = "ready" THEN "old" ELSE "none",
  provenanceClean |-> cleanProvenance,
  guardsClean |-> cleanGuards,
  landingTab |-> landingTab,
  retained |-> retained,        \* retained contexts; a prior removed-uncertain
                                \* evidence entry may persist, a settled
                                \* reinsert of the removed cache must never
  originEvidence |-> "none",    \* none | clean | dirty (UnavailablePoll join proof)
  inputPhases |-> {},           \* phases at which an input admission occurred
  taggedInFlight |-> FALSE,     \* one bounded pre-admitted tagged input
  taggedTarget |-> "none",      \* epoch the in-flight tagged input belongs to
  taggedSettledAt |-> "none",   \* none | old | landing
  settledLanded |-> FALSE,      \* whether that settle ran inside the landing epoch
  staleDropped |-> FALSE,
  lateDropped |-> FALSE,
  oldEventDropped |-> FALSE,
  landed |-> FALSE,
  landedVia |-> "none",         \* none | cleanup | direct | retired
  now |-> 0,
  cutoff |-> 0
]

Init == \E phase \in LivePhases, panes \in SUBSET Panes:
         \E attached \in SUBSET panes:
          \E dirty \in SUBSET (panes \ attached):
           \E cleanProvenance \in BOOLEAN, cleanGuards \in BOOLEAN,
              landingTab \in BOOLEAN,
              retained \in SUBSET {"other", "removed-uncertain"}:
             /\ phase = "ready" => attached # {}
             /\ phase # "ready" => attached = {}
             /\ phase = "empty" => panes = {}
             /\ s = Initial(phase, panes, attached, dirty, cleanProvenance,
                            cleanGuards, landingTab, retained)

Live == s.phase \in LivePhases

\* One recovery-deadline sample is minted with the permit and never renewed;
\* staging, cleanup and install all read this single cutoff.
AdmitFlight ==
  /\ Live /\ s.flight = 0 /\ s.permit # "carried" /\ ~s.staged
  /\ s.now + 2 <= MaxTime
  /\ s' = [s EXCEPT !.flight = 1, !.cutoff = s.now + 2]

\* classify = Candidate: the current context still projects — the ordinary
\* membership path consumes the flight outside this model's removal scope.
DeliverCandidate ==
  /\ s.flight = 1 /\ Live
  /\ s' = [s EXCEPT !.flight = 0, !.classified = "candidate"]

\* classify = Removed: envelope-validated same-site nonregressing snapshot,
\* the current context's id absent from every navigation entry, the surviving
\* standalone projects, the removed context's provenance fully settled and all
\* structural guards quiet. Only this conjunction mints the permit; it rides
\* inside the prepared candidate, never on ambient state.
DeliverRemoved ==
  /\ s.flight = 1 /\ Live
  /\ s.provenanceClean /\ s.guardsClean
  /\ s' = [s EXCEPT !.permit = "carried", !.permitGen = 1, !.flight = 0,
              !.classified = "removed", !.removalAttached = s.attached,
              !.removalDirty = s.dirty]

\* The same classification result with any guard dirty refuses the permit and
\* retires truthfully — unknown creation evidence is never erased to land.
RemovedRejected ==
  /\ s.flight = 1 /\ Live /\ (~s.provenanceClean \/ ~s.guardsClean)
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0,
              !.classified = "divergent", !.originEvidence = "none",
              !.panes = {}, !.attached = {}, !.dirty = {},
              !.taggedInFlight = FALSE]

\* classify = Divergent: wrong site, regressed revision, malformed navigation
\* or the old context id still present elsewhere — never a removal.
DeliverDivergent ==
  /\ s.flight = 1 /\ Live
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0,
              !.classified = "divergent", !.originEvidence = "none",
              !.panes = {}, !.attached = {}, !.dirty = {},
              !.taggedInFlight = FALSE]

\* With viewers alive the two-segment order is mandatory: stage the exact
\* cleanup first (phase ReconcilingMembership freezes Ready-based admission),
\* keep old cache/desired so the executor detaches the correct epoch.
StageCleanup ==
  /\ s.permit = "carried" /\ s.permitGen = 1 /\ s.phase = "ready"
  /\ s.attached # {} /\ s.desired = "old" /\ s.now < s.cutoff
  /\ s' = [s EXCEPT !.phase = "reconciling", !.staged = TRUE]

\* No viewers and no stray flags: the direct install leg applies unchanged.
DirectInstall ==
  /\ s.permit = "carried" /\ s.permitGen = 1 /\ s.attached = {}
  /\ s.dirty = {} /\ ~s.staged /\ s.now < s.cutoff
  /\ s' = [s EXCEPT !.permit = "spent", !.flight = 0,
              !.location = "landing",
              !.phase = IF s.landingTab THEN "landed-ready" ELSE "landed-empty",
              !.desired = IF s.landingTab THEN "landing" ELSE "none",
              !.panes = {}, !.dirty = {}, !.retained = s.retained,
              !.landed = TRUE, !.landedVia = "direct"]

\* A stray projected/staged/accepted record with no viewer cannot be cleaned
\* (nothing to Detach) and must not install — the four-flag gate stays.
DirtyBlocksDirectInstall ==
  /\ s.permit = "carried" /\ s.permitGen = 1 /\ s.attached = {}
  /\ s.dirty # {} /\ ~s.staged
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none",
              !.cleanupSettled = TRUE, !.originEvidence = "none",
              !.panes = {}, !.dirty = {}, !.taggedInFlight = FALSE]

\* The workspace close published every member Disposed before the deletion was
\* visible, so the ordinary Detach receives Detached per pane; the anchor epoch
\* drains pane by pane before install may run.
DetachDetached ==
  /\ s.staged /\ s.now < s.cutoff /\ \E p \in s.attached:
      s' = [s EXCEPT !.attached = @ \ {p}, !.detached = @ \cup {p}]

\* A stale worker releases its slot once and reports Unavailable; the cleanup
\* fails honestly — no fabricated proof, no unconditional retry of the wire
\* action. Retirement is the truthful outcome, origin stays empty. Failure
\* records nothing in `detached`: that field is canonical Detached evidence
\* and grows only through a real DetachDetached result.
DetachUnavailable ==
  /\ s.staged /\ s.attached # {}
  /\ s' = [s EXCEPT !.phase = "retired", !.staged = FALSE, !.permit = "none",
              !.flight = 0, !.cleanupSettled = TRUE, !.originEvidence = "none",
              !.panes = {}, !.attached = {}, !.dirty = {},
              !.taggedInFlight = FALSE]

DetachTransportError ==
  /\ s.staged /\ s.attached # {}
  /\ s' = [s EXCEPT !.phase = "retired", !.staged = FALSE, !.permit = "none",
              !.flight = 0, !.cleanupSettled = TRUE, !.originEvidence = "none",
              !.panes = {}, !.attached = {}, !.dirty = {},
              !.taggedInFlight = FALSE]

\* MembershipDetached consumed: the pending candidate copy (carrying the same
\* permit) installs inside the exact-cleanup transaction — old anchor records
\* drop, cache/desired/phase commit to the landing context, the removed
\* location is skipped from retained reinsertion (prior uncertain evidence
\* entries stay), and the permit is spent once.
InstallAfterCleanup ==
  /\ s.staged /\ s.attached = {} /\ s.permit = "carried"
  /\ s.permitGen = 1 /\ s.now < s.cutoff
  /\ s' = [s EXCEPT !.permit = "spent", !.staged = FALSE, !.flight = 0,
              !.location = "landing",
              !.phase = IF s.landingTab THEN "landed-ready" ELSE "landed-empty",
              !.desired = IF s.landingTab THEN "landing" ELSE "none",
              !.panes = {}, !.dirty = {}, !.retained = s.retained,
              !.cleanupSettled = TRUE, !.landed = TRUE, !.landedVia = "cleanup"]

\* The carried permit expires with its single sampled window: a prepared or
\* staged candidate past the forward cutoff retires instead of installing.
ExpirePrepared ==
  /\ s.permit = "carried" /\ s.permitGen = 1 /\ ~s.staged /\ s.now >= s.cutoff
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none",
              !.originEvidence = "none", !.panes = {}, !.attached = {},
              !.dirty = {}, !.taggedInFlight = FALSE]

ExpireCleanup ==
  /\ s.staged /\ s.now >= s.cutoff
  /\ s' = [s EXCEPT !.phase = "retired", !.staged = FALSE, !.permit = "none",
              !.flight = 0, !.cleanupSettled = TRUE, !.originEvidence = "none",
              !.panes = {}, !.attached = {}, !.dirty = {},
              !.taggedInFlight = FALSE]

\* Input admission requires the exact Ready phase; ReconcilingMembership and
\* every non-Ready phase reject forwarding without a new gate. A landed-ready
\* session forwards to the landing pane — input survives recovery.
AdmitInput ==
  /\ \/ s.phase = "ready" /\ s.desired = "old"
     \/ s.phase = "landed-ready" /\ s.desired = "landing"
  /\ ~s.taggedInFlight
  /\ s' = [s EXCEPT !.inputPhases = @ \cup {s.phase},
              !.taggedInFlight = TRUE, !.taggedTarget = s.desired]

\* A tagged input settles only inside its own epoch: an old-tagged input while
\* its record still exists before landing, a landing-tagged input after.
SettleTaggedInput ==
  /\ s.taggedInFlight
  /\ \/ s.taggedTarget = "landing" /\ s.landed
     \/ s.taggedTarget = "old" /\ ~s.landed /\ s.phase \in LivePhases
        /\ s.panes # {}
  /\ s' = [s EXCEPT !.taggedInFlight = FALSE,
              !.taggedSettledAt = s.taggedTarget, !.settledLanded = s.landed]

\* An old-tagged input that outlives its record is dropped — incarnation and
\* desired fences, never a re-tag across the landing.
DropTaggedInput ==
  /\ s.taggedInFlight /\ s.taggedTarget = "old"
  /\ (s.landed \/ s.phase = "retired" \/ s.panes = {})
  /\ s' = [s EXCEPT !.taggedInFlight = FALSE]

\* A refresh result carrying a stale generation never settles the armed or
\* already-consumed flight; it is dropped without touching cache or flight.
StaleRefreshResult ==
  /\ s' = [s EXCEPT !.staleDropped = TRUE]

\* A cleanup result after the exact pending entry consumed or failed settles
\* nothing: it is dropped, never installing a superseded candidate.
LateCleanupResult ==
  /\ s.cleanupSettled
  /\ s' = [s EXCEPT !.lateDropped = TRUE]

\* Events addressed to the removed context after landing have no record and
\* are dropped; they cannot resurrect the old location.
OldContextEvent ==
  /\ s.landed
  /\ s' = [s EXCEPT !.oldEventDropped = TRUE]

\* Retired entry: only a Ready-phase ordinary Poll returning Unavailable and a
\* clean old-task join can mint origin evidence. Evidence captured elsewhere —
\* including any cleanup failure — is dirty and never authorizes recovery.
ObserveUnavailablePollClean ==
  /\ s.phase = "ready" /\ s.attached # {}
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none",
              !.originEvidence = "clean", !.panes = {}, !.attached = {},
              !.dirty = {}, !.taggedInFlight = FALSE]

ObserveUnavailablePollDirty ==
  /\ s.phase = "ready" /\ s.attached # {}
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none",
              !.originEvidence = "dirty", !.panes = {}, !.attached = {},
              !.dirty = {}, !.taggedInFlight = FALSE]

\* The bounded single-shot attempt: one fresh connection and one refresh
\* flight under its own sampled window, only while the clean origin holds.
BeginRetiredRecovery ==
  /\ s.phase = "retired" /\ s.originEvidence = "clean" /\ s.flight = 0
  /\ s.now + 2 <= MaxTime
  /\ s' = [s EXCEPT !.phase = "recovering", !.flight = 2, !.cutoff = s.now + 2]

\* Same classify inside the retired attempt: a Removed snapshot mints a
\* generation-2 permit; the origin supplies attempt authority only, never
\* landing authority.
RetiredMintPermit ==
  /\ s.phase = "recovering" /\ s.flight = 2 /\ s.now < s.cutoff
  /\ s.originEvidence = "clean" /\ s.provenanceClean /\ s.guardsClean
  /\ s' = [s EXCEPT !.permit = "carried", !.permitGen = 2,
              !.classified = "removed"]

\* Retired panes are already empty: external release came from the old
\* session/worker actually settling, not from clearing records. The install is
\* the direct leg under the same permit discipline.
RetiredInstall ==
  /\ s.phase = "recovering" /\ s.permit = "carried" /\ s.permitGen = 2
  /\ s.originEvidence = "clean" /\ s.now < s.cutoff
  /\ s' = [s EXCEPT !.phase = IF s.landingTab THEN "landed-ready" ELSE "landed-empty",
              !.location = "landing", !.permit = "spent", !.flight = 0,
              !.desired = IF s.landingTab THEN "landing" ELSE "none",
              !.panes = {}, !.dirty = {}, !.retained = s.retained,
              !.originEvidence = "none", !.landed = TRUE, !.landedVia = "retired"]

RetiredDivergent ==
  /\ s.phase = "recovering"
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none"]

CancelRecovery ==
  /\ s.phase = "recovering"
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none"]

ExpireRecovery ==
  /\ s.phase = "recovering" /\ s.now >= s.cutoff
  /\ s' = [s EXCEPT !.phase = "retired", !.flight = 0, !.permit = "none"]

Tick ==
  /\ s.now < MaxTime
  /\ s' = [s EXCEPT !.now = @ + 1]

Next == AdmitFlight \/ DeliverCandidate \/ DeliverRemoved \/ RemovedRejected
  \/ DeliverDivergent \/ StageCleanup \/ DirectInstall \/ DirtyBlocksDirectInstall
  \/ DetachDetached \/ DetachUnavailable \/ DetachTransportError
  \/ InstallAfterCleanup \/ ExpirePrepared \/ ExpireCleanup
  \/ AdmitInput \/ SettleTaggedInput \/ DropTaggedInput
  \/ StaleRefreshResult \/ LateCleanupResult \/ OldContextEvent
  \/ ObserveUnavailablePollClean \/ ObserveUnavailablePollDirty
  \/ BeginRetiredRecovery \/ RetiredMintPermit \/ RetiredInstall
  \/ RetiredDivergent \/ CancelRecovery \/ ExpireRecovery \/ Tick

Spec == Init /\ [][Next]_vars /\ WF_vars(Tick)
        /\ WF_vars(ExpirePrepared) /\ WF_vars(ExpireCleanup) /\ WF_vars(ExpireRecovery)

TypeOK ==
  /\ s.phase \in Phases /\ s.location \in Contexts
  /\ s.panes \in SUBSET Panes /\ s.attached \in SUBSET s.panes
  /\ s.detached \in SUBSET Panes /\ s.dirty \in SUBSET Panes
  /\ s.permit \in {"none", "carried", "spent"} /\ s.permitGen \in Gens
  /\ s.flight \in Gens /\ s.classified \in {"none", "candidate", "removed", "divergent"}
  /\ s.desired \in {"none", "old", "landing"}
  /\ s.originEvidence \in {"none", "clean", "dirty"}
  /\ s.inputPhases \in SUBSET Phases
  /\ s.taggedTarget \in {"none", "old", "landing"}
  /\ s.taggedSettledAt \in {"none", "old", "landing"}
  /\ s.settledLanded \in BOOLEAN
  /\ s.retained \in SUBSET {"other", "removed-uncertain", "removed-settled"}
  /\ s.landedVia \in {"none", "cleanup", "direct", "retired"}
  /\ s.now \in 0..MaxTime /\ s.cutoff \in 0..MaxTime

\* Cleanup runs to canonical completion before any install: every pane that
\* was attached when the permit minted received a Detached proof first.
InstallOnlyAfterCleanup ==
  s.landedVia = "cleanup" => s.removalAttached # {} /\ s.removalAttached \subseteq s.detached
DirectInstallOnlyWhenClean ==
  s.landedVia = "direct" => s.removalAttached = {} /\ s.removalDirty = {}
\* Landing never leaves an old viewer behind and never parks the removed
\* cache back into retained; prior uncertain evidence entries may persist.
NoOldAttachmentSurvives == s.landed => s.attached = {} /\ s.dirty = {}
RemovedCacheNeverReinserted == s.landed => "removed-settled" \notin s.retained
\* One permit, one flight, one consumption: a carried permit stays inside the
\* lineage of the flight generation that minted it — an ordinary candidate may
\* travel through the live phases and ReconcilingMembership, a retired-attempt
\* candidate only through the recovery window — and a spent permit never
\* re-arms, so replayed candidates or cleanup results cannot reinstall.
PermitBoundToFlight ==
  s.permit = "carried" =>
    \/ s.permitGen = 1 /\ s.phase \in LivePhases \cup {"reconciling"}
    \/ s.permitGen = 2 /\ s.phase = "recovering"
PermitSpentOnlyByInstall == s.permit = "spent" => s.landed
\* ReconcilingMembership suppresses Ready-based admission from staging until
\* commit; tagged inputs settle only inside their own epoch.
NoInputDuringCleanup == "reconciling" \notin s.inputPhases
NoCrossTargetInput ==
  /\ s.taggedSettledAt = "old" => ~s.settledLanded
  /\ s.taggedSettledAt = "landing" => s.settledLanded
\* Without a clean UnavailablePoll origin there is no retired recovery; every
\* other failure stays isolated with its evidence intact.
RetiredEntryRequiresOrigin == s.phase = "recovering" => s.originEvidence = "clean"
HonestIsolation == s.phase = "retired" => ~s.landed
\* Once the landing commits, no stale generation, late cleanup result or
\* removed-context event can move the session back.
LandingIsStable == s.landed => s.location = "landing" /\ s.desired # "old"

StaleNeverSettles == [][s.landed => s'.landed /\ s'.location = "landing"
                        /\ s'.desired = s.desired]_vars
SpentPermitNeverRearms == [][s.permit = "spent" => s'.permit # "carried"]_vars
EventuallySettles == s.phase \in {"reconciling", "recovering"}
                     ~> s.phase \in {"retired", "landed-ready", "landed-empty"}
=============================================================================
