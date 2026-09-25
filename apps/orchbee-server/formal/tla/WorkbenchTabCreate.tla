------------------------- MODULE WorkbenchTabCreate -------------------------
(* Tab naming atoms include automatic mode. CreatedLabel to RenamedLabel also
abstracts fixing an automatic name without changing its raw text. Presentation
labels retain their creation anchor; move and settlement never rewrite naming
metadata. Position-derived display is outside this ownership model. *)
EXTENDS Integers, Naturals, Sequences, FiniteSets, TLC

(*
Three request identities and two semantic command fingerprints race with
legacy terminal.open. A tab sequence entry names the owner of one retained
terminal plan. Arc pointer identity is abstracted by two private seal values:
the Pending Workbench entry and retained Terminal record each share their seal
with the corresponding token inside one opaque Site wrapper. ForeignSeal
models a token originating at another owner or a stale entry.
replayObservation is a single bounded invocation witness outside the ledger:
only an exact replay opens its immutable pending/final response flight, and a
later ledger settlement cannot overwrite a pending flight before its write.
*)

CONSTANTS Requests, Fingerprints, Legacy, ForeignSeal,
          MaxTabs, MaxLedgerEntries, MaxLedgerBytes,
          ReservedBytes, FinalBytes, MaxRenames, MaxMoves

ASSUME /\ Cardinality(Requests) = 3
       /\ Cardinality(Fingerprints) = 2
       /\ Legacy \notin Requests
       /\ ForeignSeal \notin Requests
       /\ ForeignSeal # Legacy
       /\ MaxTabs = 2
       /\ MaxLedgerEntries = 2
       /\ ReservedBytes > 0
       /\ FinalBytes > 0
       /\ FinalBytes <= ReservedBytes
       /\ ReservedBytes <= MaxLedgerBytes
       /\ MaxRenames \in 0..1
       /\ MaxMoves \in 0..2

Owners == Requests \cup {Legacy}
NoOwner == "NO_OWNER"
NoFingerprint == "NO_FINGERPRINT"
NoSeal == "NO_SEAL"
NoLabel == "NO_LABEL"
CreatedLabel == "CREATED_LABEL"
RenamedLabel == "RENAMED_LABEL"
Labels == {NoLabel, CreatedLabel, RenamedLabel}
CoreOnly == MaxRenames = 0 /\ MaxMoves = 0
CompositionOnly == MaxRenames + MaxMoves > 0
SubmissionBases == IF MaxRenames > 0 THEN {0} ELSE 0..MaxTabs

LedgerStates == {"absent", "pending", "rejected", "created"}
TerminalStates ==
  {"absent", "creating", "started",
   "runtime-unavailable", "server-busy", "failed", "unknown"}
FinalTerminalStates ==
  {"started", "runtime-unavailable", "server-busy", "failed", "unknown"}
CompletionOutcomes == IF CompositionOnly THEN {"started"} ELSE FinalTerminalStates
ReplayFlightStates ==
  {"none", "pending", "pending-written", "rejected", "rejected-written",
   "created", "created-written"}
OpenReplies ==
  {"idle", "waiting", "opened", "runtime-unavailable",
   "server-busy", "failed", "unknown", "unavailable"}

RequestRecordType ==
  [status : LedgerStates,
   base : -1..(MaxTabs + MaxRenames + MaxMoves),
   committedRevision : -1..(MaxTabs + MaxRenames + MaxMoves),
   resultRevision : -1..(MaxTabs + MaxRenames + MaxMoves),
   fingerprint : Fingerprints \cup {NoFingerprint},
   seal : Requests \cup {NoSeal},
   tokenLive : BOOLEAN,
   terminalSeal : Requests \cup {NoSeal},
   terminalTokenLive : BOOLEAN]

EmptyRequestRecord ==
  [status |-> "absent",
   base |-> -1,
   committedRevision |-> -1,
   resultRevision |-> -1,
   fingerprint |-> NoFingerprint,
   seal |-> NoSeal,
   tokenLive |-> FALSE,
   terminalSeal |-> NoSeal,
   terminalTokenLive |-> FALSE]

VARIABLE moveCount
VARIABLES
  revision,
  tabs,
  firstSlot,
  tabLabel,
  presentationLabel,
  terminal,
  pinnedGeneration,
  effects,
  requestState,
  chargedBytes,
  retired,
  workerAvailable,
  workerGeneration,
  openReply,
  rejectedSettlementAttempts,
  replayObservation

baseVars ==
  <<revision, tabs, firstSlot, tabLabel, presentationLabel,
    terminal, pinnedGeneration, effects,
    requestState, chargedBytes, retired, workerAvailable, workerGeneration,
    openReply, rejectedSettlementAttempts, replayObservation>>

vars == <<baseVars, moveCount>>

TabOwners == {tabs[index] : index \in 1..Len(tabs)}
RenamedOwners == {owner \in TabOwners : tabLabel[owner] = RenamedLabel}
LedgerEntries ==
  {request \in Requests : requestState[request].status # "absent"}
PendingEntries ==
  {request \in Requests : requestState[request].status = "pending"}
SettledEntries ==
  {request \in Requests :
     requestState[request].status \in {"rejected", "created"}}

(* Owners abstract distinct retained references. BindingFor is the exact
authorization-to-PresentationBinding conversion, not a stored Site anchor. *)
BindingFor(owner) == owner
DerivedComposition ==
  /\ (Len(tabs) = 0) = (firstSlot = NoOwner)
  /\ (Len(tabs) > 0 =>
       /\ terminal[firstSlot] # "absent"
       /\ Cardinality({i \in 1..Len(tabs):
            BindingFor(tabs[i]) = BindingFor(firstSlot)}) = 1)


CountAvailable == Cardinality(LedgerEntries) < MaxLedgerEntries
CanCharge(amount) == chargedBytes + amount <= MaxLedgerBytes

BaseInit ==
  /\ revision = 0
  /\ tabs = <<>>
  /\ firstSlot = NoOwner
  /\ tabLabel = [owner \in Owners |-> NoLabel]
  /\ presentationLabel = [owner \in Owners |-> NoLabel]
  /\ terminal = [owner \in Owners |-> "absent"]
  /\ pinnedGeneration = [owner \in Owners |-> 0]
  /\ effects = [owner \in Owners |-> 0]
  /\ requestState = [request \in Requests |-> EmptyRequestRecord]
  /\ chargedBytes = 0
  /\ retired = FALSE
  /\ workerAvailable \in IF CompositionOnly THEN {TRUE} ELSE BOOLEAN
  /\ workerGeneration = 1
  /\ openReply = "idle"
  /\ rejectedSettlementAttempts = 0
  /\ replayObservation =
       [request |-> NoOwner, flight |-> "none", resultRevision |-> -1]

CallRetired(request) ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ retired
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

ReplayPending(request, base, fingerprint) ==
  /\ ~retired
  /\ requestState[request].status = "pending"
  /\ requestState[request].base = base
  /\ requestState[request].fingerprint = fingerprint
  /\ replayObservation.flight = "none"
  /\ replayObservation' =
       [request |-> request, flight |-> "pending",
        resultRevision |-> requestState[request].resultRevision]
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration,
         openReply, rejectedSettlementAttempts>>

ReplaySettled(request, base, fingerprint) ==
  /\ ~retired
  /\ requestState[request].status \in {"rejected", "created"}
  /\ requestState[request].base = base
  /\ requestState[request].fingerprint = fingerprint
  /\ (replayObservation.flight = "none"
       \/ /\ replayObservation.request = request
          /\ replayObservation.flight = "pending-written")
  /\ replayObservation' =
       [request |-> request,
        flight |-> IF requestState[request].status = "rejected"
                   THEN "rejected" ELSE "created",
        resultRevision |-> requestState[request].resultRevision]
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration,
         openReply, rejectedSettlementAttempts>>

Conflict(request, base, fingerprint) ==
  /\ ~retired
  /\ requestState[request].status # "absent"
  /\ (requestState[request].base # base
       \/ requestState[request].fingerprint # fingerprint)
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

CountUnavailable(request, base, fingerprint) ==
  /\ CoreOnly
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ ~CountAvailable
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

RejectRevision(request, base, fingerprint) ==
  /\ CoreOnly
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ CountAvailable
  /\ base # revision
  /\ CanCharge(FinalBytes)
  /\ requestState' =
       [requestState EXCEPT
         ![request] =
           [@ EXCEPT
             !.status = "rejected",
             !.base = base,
             !.resultRevision = revision,
             !.fingerprint = fingerprint]]
  /\ chargedBytes' = chargedBytes + FinalBytes
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

RejectResource(request, base, fingerprint) ==
  /\ CoreOnly
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ CountAvailable
  /\ base = revision
  /\ Len(tabs) = MaxTabs
  /\ CanCharge(FinalBytes)
  /\ requestState' =
       [requestState EXCEPT
         ![request] =
           [@ EXCEPT
             !.status = "rejected",
             !.base = base,
             !.resultRevision = revision,
             !.fingerprint = fingerprint]]
  /\ chargedBytes' = chargedBytes + FinalBytes
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

RejectionBytesUnavailable(request, base, fingerprint) ==
  /\ CoreOnly
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ CountAvailable
  /\ (base # revision \/ (base = revision /\ Len(tabs) = MaxTabs))
  /\ ~CanCharge(FinalBytes)
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

WorkerUnavailable(request, base, fingerprint) ==
  /\ CoreOnly
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ CountAvailable
  /\ base = revision
  /\ Len(tabs) < MaxTabs
  /\ ~workerAvailable
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

ReservationUnavailable(request, base, fingerprint) ==
  /\ CoreOnly
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ CountAvailable
  /\ base = revision
  /\ Len(tabs) < MaxTabs
  /\ workerAvailable
  /\ ~CanCharge(ReservedBytes)
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

CommitCreate(request, base, fingerprint) ==
  /\ ~retired
  /\ requestState[request].status = "absent"
  /\ CountAvailable
  /\ base = revision
  /\ Len(tabs) < MaxTabs
  /\ workerAvailable
  /\ CanCharge(ReservedBytes)
  /\ tabs' = Append(tabs, request)
  /\ revision' = revision + 1
  /\ firstSlot' = IF Len(tabs) = 0 THEN request ELSE firstSlot
  /\ tabLabel' = [tabLabel EXCEPT ![request] = CreatedLabel]
  /\ presentationLabel' =
       [presentationLabel EXCEPT ![request] = CreatedLabel]
  /\ terminal' = [terminal EXCEPT ![request] = "creating"]
  /\ pinnedGeneration' =
       [pinnedGeneration EXCEPT ![request] = workerGeneration]
  /\ requestState' =
       [requestState EXCEPT
         ![request] =
            [status |-> "pending",
             base |-> base,
             committedRevision |-> revision + 1,
             resultRevision |-> revision + 1,
            fingerprint |-> fingerprint,
            seal |-> request,
            tokenLive |-> TRUE,
            terminalSeal |-> request,
            terminalTokenLive |-> TRUE]]
  /\ chargedBytes' = chargedBytes + ReservedBytes
  /\ UNCHANGED
       <<moveCount, effects, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

OpenCreatesFirst ==
  /\ CoreOnly
  /\ firstSlot = NoOwner
  /\ Len(tabs) = 0
  /\ ~retired
  /\ workerAvailable
  /\ tabs' = Append(tabs, Legacy)
  /\ revision' = revision + 1
  /\ firstSlot' = Legacy
  /\ tabLabel' = [tabLabel EXCEPT ![Legacy] = CreatedLabel]
  /\ presentationLabel' =
       [presentationLabel EXCEPT ![Legacy] = CreatedLabel]
  /\ terminal' = [terminal EXCEPT ![Legacy] = "creating"]
  /\ pinnedGeneration' =
       [pinnedGeneration EXCEPT ![Legacy] = workerGeneration]
  /\ openReply' = "waiting"
  /\ UNCHANGED
       <<moveCount, effects, requestState, chargedBytes, retired, workerAvailable,
         workerGeneration, rejectedSettlementAttempts, replayObservation>>

OpenUnavailableEmpty ==
  /\ CoreOnly
  /\ firstSlot = NoOwner
  /\ Len(tabs) = 0
  /\ (retired \/ ~workerAvailable)
  /\ openReply' = "unavailable"
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable,
         workerGeneration, rejectedSettlementAttempts, replayObservation>>

OpenOutcome(phase) ==
  CASE phase = "creating" -> "waiting"
    [] phase = "started" -> "opened"
    [] phase = "runtime-unavailable" -> "runtime-unavailable"
    [] phase = "server-busy" -> "server-busy"
    [] phase = "failed" -> "failed"
    [] OTHER -> "unknown"
OpenObservesFirst ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ firstSlot # NoOwner
  /\ openReply' = OpenOutcome(terminal[firstSlot])
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable,
         workerGeneration, rejectedSettlementAttempts, replayObservation>>

(* A pure tab.rename has already passed its own authorization, replay and
capacity boundary. This action models only its atomic projection commit while
tab.create completion remains independently ordered. *)
ExternalRename(owner) ==
  /\ MaxRenames > 0
  /\ owner \in TabOwners
  /\ tabLabel[owner] = CreatedLabel
  /\ Cardinality(RenamedOwners) < MaxRenames
  /\ revision' = revision + 1
  /\ tabLabel' = [tabLabel EXCEPT ![owner] = RenamedLabel]
  /\ UNCHANGED
       <<moveCount, tabs, firstSlot, presentationLabel, terminal, pinnedGeneration,
         effects, requestState, chargedBytes, retired, workerAvailable,
         workerGeneration, openReply, rejectedSettlementAttempts,
         replayObservation>>


Dispatch(owner) ==
  /\ terminal[owner] = "creating"
  /\ effects[owner] = 0
  /\ effects' = [effects EXCEPT ![owner] = 1]
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, requestState,
         chargedBytes, retired, workerAvailable, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

Complete(owner, outcome) ==
  /\ outcome \in FinalTerminalStates
  /\ terminal[owner] = "creating"
  /\ DerivedComposition
  /\ effects[owner] = 1
  /\ (owner \in Requests =>
       /\ requestState[owner].status = "pending"
       /\ requestState[owner].tokenLive
       /\ requestState[owner].seal = owner
       /\ requestState[owner].terminalTokenLive
       /\ requestState[owner].terminalSeal = owner)
  /\ (outcome = "started" =>
       (workerAvailable /\ workerGeneration = pinnedGeneration[owner]))
  /\ terminal' = [terminal EXCEPT ![owner] = outcome]
  /\ requestState' =
       IF owner \in Requests
       THEN
         [requestState EXCEPT
           ![owner] =
             [@ EXCEPT
               !.status = "created",
               !.seal = NoSeal,
                !.tokenLive = FALSE,
                !.terminalSeal = NoSeal,
                !.terminalTokenLive = FALSE]]
       ELSE requestState
  /\ chargedBytes' =
       IF owner \in Requests
       THEN chargedBytes - ReservedBytes + FinalBytes
       ELSE chargedBytes
  /\ openReply' =
       IF firstSlot = owner /\ openReply = "waiting"
       THEN OpenOutcome(outcome)
       ELSE openReply
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         pinnedGeneration, effects, retired,
         workerAvailable, workerGeneration, rejectedSettlementAttempts, replayObservation>>

RejectForeignOrStaleSettlement(owner) ==
  /\ CoreOnly
  /\ owner \in Requests
  /\ requestState[owner].status \in {"pending", "created"}
  /\ rejectedSettlementAttempts = 0
  /\ ForeignSeal # requestState[owner].seal
  /\ rejectedSettlementAttempts' = 1
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable,
         workerGeneration, openReply, replayObservation>>

RejectForeignOrStaleTerminalCompletion(owner) ==
  /\ CoreOnly
  /\ owner \in Requests
  /\ requestState[owner].status \in {"pending", "created"}
  /\ rejectedSettlementAttempts = 0
  /\ ForeignSeal # requestState[owner].terminalSeal
  /\ rejectedSettlementAttempts' = 2
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable,
         workerGeneration, openReply, replayObservation>>

WriteReplayResponse ==
  /\ replayObservation.flight \in {"pending", "rejected", "created"}
  /\ replayObservation' =
       [replayObservation EXCEPT !.flight =
          CASE replayObservation.flight = "pending" -> "pending-written"
            [] replayObservation.flight = "rejected" -> "rejected-written"
            [] OTHER -> "created-written"]
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerAvailable, workerGeneration,
         openReply, rejectedSettlementAttempts>>

Retire ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ ~retired
  /\ retired' = TRUE
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, workerAvailable, workerGeneration,
         openReply, rejectedSettlementAttempts, replayObservation>>

LoseWorker ==
  /\ CoreOnly
  /\ workerAvailable
  /\ workerAvailable' = FALSE
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

RestoreWorker ==
  /\ CoreOnly
  /\ ~workerAvailable
  /\ workerAvailable' = TRUE
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, workerGeneration, openReply,
         rejectedSettlementAttempts, replayObservation>>

ReplaceWorker ==
  /\ CoreOnly
  /\ workerGeneration = 1
  /\ workerGeneration' = 2
  /\ workerAvailable' = TRUE
  /\ UNCHANGED
       <<moveCount, revision, tabs, firstSlot, tabLabel, presentationLabel,
         terminal, pinnedGeneration, effects,
         requestState, chargedBytes, retired, openReply,
         rejectedSettlementAttempts, replayObservation>>

(* Coverage-only stuttering witnesses for the concurrency configuration. *)
ObserveTwoPending ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ Cardinality(PendingEntries) = 2
  /\ UNCHANGED vars

ObserveOlderSettledWhileNewerPending ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ Len(tabs) = 2
  /\ tabs[1] \in Requests
  /\ tabs[2] \in Requests
  /\ requestState[firstSlot].status = "created"
  /\ \E owner \in TabOwners \ {firstSlot}: requestState[owner].status = "pending"
  /\ UNCHANGED vars

ObserveNewerSettledWhileOlderPending ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ Len(tabs) = 2
  /\ tabs[1] \in Requests
  /\ tabs[2] \in Requests
  /\ requestState[firstSlot].status = "pending"
  /\ \E owner \in TabOwners \ {firstSlot}: requestState[owner].status = "created"
  /\ UNCHANGED vars

ObserveBothSettled ==
  /\ (CoreOnly \/ MaxMoves > 0)
  /\ Len(tabs) = 2
  /\ tabs[1] \in Requests
  /\ tabs[2] \in Requests
  /\ requestState[tabs[1]].status = "created"
  /\ requestState[tabs[2]].status = "created"
  /\ UNCHANGED vars

ObservePendingRenameReplay ==
  /\ \E request \in Requests:
       /\ requestState[request].status = "pending"
       /\ request \in RenamedOwners
       /\ replayObservation.request = request
       /\ replayObservation.flight \in {"pending", "pending-written"}
       /\ replayObservation.resultRevision =
            requestState[request].resultRevision
  /\ UNCHANGED vars

ObserveCompletedRenameReplay ==
  /\ \E request \in Requests:
       /\ requestState[request].status = "created"
       /\ request \in RenamedOwners
       /\ replayObservation.request = request
       /\ replayObservation.flight \in {"created", "created-written"}
       /\ replayObservation.resultRevision =
            requestState[request].resultRevision
  /\ UNCHANGED vars

SubmitActions(request, base, fingerprint) ==
  \/ ReplayPending(request, base, fingerprint)
  \/ ReplaySettled(request, base, fingerprint)
  \/ Conflict(request, base, fingerprint)
  \/ CountUnavailable(request, base, fingerprint)
  \/ RejectRevision(request, base, fingerprint)
  \/ RejectResource(request, base, fingerprint)
  \/ RejectionBytesUnavailable(request, base, fingerprint)
  \/ WorkerUnavailable(request, base, fingerprint)
  \/ ReservationUnavailable(request, base, fingerprint)
  \/ CommitCreate(request, base, fingerprint)


MovePendingMember ==
  /\ ~retired
  /\ Len(tabs) = 2
  /\ DerivedComposition
  /\ moveCount < MaxMoves
  /\ tabs' = <<tabs[2], tabs[1]>>
  /\ revision' = revision + 1
  /\ moveCount' = moveCount + 1
  /\ UNCHANGED <<firstSlot, tabLabel, presentationLabel, terminal,
       pinnedGeneration, effects, requestState, chargedBytes, retired,
       workerAvailable, workerGeneration, openReply,
       rejectedSettlementAttempts, replayObservation>>

RetirePendingAfterMove ==
  /\ moveCount > 0
  /\ firstSlot \in Requests
  /\ requestState[firstSlot].status = "pending"
  /\ tabs[1] # firstSlot
  /\ Retire
  /\ UNCHANGED moveCount

CompleteMovedFirstAfterRetire ==
  /\ retired
  /\ moveCount > 0
  /\ firstSlot \in Requests
  /\ tabs[1] # firstSlot
  /\ \E outcome \in CompletionOutcomes: Complete(firstSlot, outcome)
  /\ UNCHANGED moveCount

ObserveMovedFirstPending ==
  /\ moveCount > 0
  /\ tabs[1] # firstSlot
  /\ firstSlot \in PendingEntries
  /\ UNCHANGED vars

ObserveRetiredReadMoveReplay ==
  /\ retired
  /\ moveCount > 0
  /\ firstSlot \in Requests
  /\ requestState[firstSlot].status = "created"
  /\ UNCHANGED vars

Init == BaseInit /\ moveCount = 0
Next ==
  \/ \E request \in Requests: CallRetired(request)
  \/ \E request \in Requests,
         base \in SubmissionBases,
         fingerprint \in Fingerprints:
       SubmitActions(request, base, fingerprint)
  \/ OpenCreatesFirst
  \/ OpenUnavailableEmpty
  \/ OpenObservesFirst
  \/ \E owner \in Requests: ExternalRename(owner)
  \/ \E owner \in Owners: Dispatch(owner)
  \/ \E owner \in Owners, outcome \in CompletionOutcomes:
       Complete(owner, outcome)
  \/ \E owner \in Requests: RejectForeignOrStaleSettlement(owner)
  \/ \E owner \in Requests: RejectForeignOrStaleTerminalCompletion(owner)
  \/ WriteReplayResponse
  \/ Retire
  \/ LoseWorker
  \/ RestoreWorker
  \/ ReplaceWorker
  \/ ObserveTwoPending
  \/ ObserveOlderSettledWhileNewerPending
  \/ ObserveNewerSettledWhileOlderPending
  \/ ObserveBothSettled
  \/ ObservePendingRenameReplay
  \/ ObserveCompletedRenameReplay
  \/ MovePendingMember
  \/ RetirePendingAfterMove
  \/ CompleteMovedFirstAfterRetire
  \/ ObserveMovedFirstPending
  \/ ObserveRetiredReadMoveReplay
Spec == Init /\ [][Next]_vars

MovePreservesNonOrder ==
  [][moveCount' > moveCount =>
     /\ revision' = revision + 1
     /\ TabOwners' = TabOwners
     /\ UNCHANGED <<firstSlot, tabLabel, presentationLabel, terminal,
          pinnedGeneration, effects, requestState, chargedBytes, retired,
          workerAvailable, workerGeneration, openReply,
          rejectedSettlementAttempts, replayObservation>>]_vars

TypeOK ==
  /\ moveCount \in 0..MaxMoves
  /\ revision \in 0..(MaxTabs + MaxRenames + MaxMoves)
  /\ tabs \in Seq(Owners)
  /\ Len(tabs) <= MaxTabs
  /\ firstSlot \in Owners \cup {NoOwner}
  /\ tabLabel \in [Owners -> Labels]
  /\ presentationLabel \in [Owners -> Labels]
  /\ terminal \in [Owners -> TerminalStates]
  /\ pinnedGeneration \in [Owners -> 0..2]
  /\ effects \in [Owners -> 0..1]
  /\ requestState \in [Requests -> RequestRecordType]
  /\ chargedBytes \in 0..MaxLedgerBytes
  /\ retired \in BOOLEAN
  /\ workerAvailable \in BOOLEAN
  /\ workerGeneration \in 1..2
  /\ openReply \in OpenReplies
  /\ rejectedSettlementAttempts \in 0..2
  /\ replayObservation \in
       [request : Requests \cup {NoOwner}, flight : ReplayFlightStates,
        resultRevision : -1..(MaxTabs + MaxRenames + MaxMoves)]

NoDuplicateTabs ==
  Len(tabs) = Cardinality(TabOwners)

RevisionTracksMembership ==
  revision = Len(tabs) + Cardinality(RenamedOwners) + moveCount

LabelsMatchMembership ==
  /\ Cardinality(RenamedOwners) <= MaxRenames
  /\ \A owner \in Owners:
       IF owner \in TabOwners
       THEN /\ presentationLabel[owner] = CreatedLabel
            /\ tabLabel[owner] \in {CreatedLabel, RenamedLabel}
       ELSE /\ presentationLabel[owner] = NoLabel
            /\ tabLabel[owner] = NoLabel

FirstSlotIsCanonical == DerivedComposition

LedgerOwnsMembership ==
  \A request \in Requests:
    /\ (requestState[request].status \in {"pending", "created"} =>
          request \in TabOwners)
    /\ (requestState[request].status = "rejected" =>
          request \notin TabOwners)

PendingPrecedesFinal ==
  \A request \in Requests:
    /\ (requestState[request].status = "pending" =>
          terminal[request] = "creating")
    /\ (requestState[request].status = "created" =>
          terminal[request] \in FinalTerminalStates)

ExactPendingSeal ==
  \A request \in Requests:
    /\ (requestState[request].status = "pending" =>
          /\ requestState[request].seal = request
          /\ requestState[request].tokenLive)
    /\ (requestState[request].status \in {"absent", "rejected", "created"} =>
          /\ requestState[request].seal = NoSeal
          /\ ~requestState[request].tokenLive)

ExactTerminalCompletionSeal ==
  \A request \in Requests:
    /\ (requestState[request].status = "pending" =>
          /\ requestState[request].terminalSeal = request
          /\ requestState[request].terminalTokenLive)
    /\ (requestState[request].status \in {"absent", "rejected", "created"} =>
          /\ requestState[request].terminalSeal = NoSeal
          /\ ~requestState[request].terminalTokenLive)

DistinctPendingCapabilities ==
  \A left, right \in PendingEntries:
    left # right =>
      /\ requestState[left].seal # requestState[right].seal
      /\ requestState[left].terminalSeal # requestState[right].terminalSeal

ReplayFlightIsIndependentAndCorrelated ==
  /\ (replayObservation.flight = "none") =
       (replayObservation.request = NoOwner)
  /\ (replayObservation.flight = "none") =
       (replayObservation.resultRevision = -1)
  /\ (replayObservation.flight # "none" =>
        /\ replayObservation.request \in Requests
        /\ requestState[replayObservation.request].status # "absent"
        /\ replayObservation.resultRevision =
             requestState[replayObservation.request].resultRevision)
  /\ (replayObservation.flight \in {"pending", "pending-written"} =>
        requestState[replayObservation.request].status \in {"pending", "created"})
  /\ (replayObservation.flight \in {"rejected", "rejected-written"} =>
        requestState[replayObservation.request].status = "rejected")
  /\ (replayObservation.flight \in {"created", "created-written"} =>
        requestState[replayObservation.request].status = "created")

RetainedCommandFingerprint ==
  \A request \in Requests:
    /\ (requestState[request].status = "absent" =>
          requestState[request].fingerprint = NoFingerprint)
    /\ (requestState[request].status # "absent" =>
          requestState[request].fingerprint \in Fingerprints)

CommittedRevisionIsStable ==
  \A request \in Requests:
    /\ (requestState[request].status \in {"absent", "rejected"} =>
          requestState[request].committedRevision = -1)
    /\ (requestState[request].status \in {"pending", "created"} =>
          requestState[request].committedRevision \in 1..revision)

ResultRevisionIsStable ==
  \A request \in Requests:
    /\ (requestState[request].status = "absent" =>
          requestState[request].resultRevision = -1)
    /\ (requestState[request].status = "rejected" =>
          requestState[request].resultRevision \in 0..revision)
    /\ (requestState[request].status \in {"pending", "created"} =>
          requestState[request].resultRevision =
            requestState[request].committedRevision)

CompletionPreservesProjection == [][
  (\E request \in Requests:
     requestState[request].status = "pending"
     /\ requestState'[request].status = "created") =>
  (revision' = revision /\ tabs' = tabs /\ firstSlot' = firstSlot
   /\ tabLabel' = tabLabel /\ presentationLabel' = presentationLabel)]_vars

DispatchFollowsMembership ==
  \A owner \in Owners:
    effects[owner] = 1 =>
      owner \in TabOwners /\ pinnedGeneration[owner] > 0

NoRedispatch ==
  \A owner \in Owners: effects[owner] <= 1

PinnedGenerationIsExact ==
  \A owner \in Owners:
    terminal[owner] # "absent" => pinnedGeneration[owner] \in 1..2

LedgerAccounting ==
  chargedBytes =
    ReservedBytes * Cardinality(PendingEntries)
    + FinalBytes * Cardinality(SettledEntries)

LegacyCanOnlyBeFirst ==
  Legacy \in TabOwners => firstSlot = Legacy

WaitingOpenOwnsFirst ==
  openReply = "waiting" =>
    firstSlot # NoOwner /\ terminal[firstSlot] = "creating"

RequestSymmetry == Permutations(Requests)

=============================================================================
