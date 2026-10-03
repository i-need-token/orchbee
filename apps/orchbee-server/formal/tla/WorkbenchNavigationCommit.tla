--------------------- MODULE WorkbenchNavigationCommit ---------------------
(* Tab label atoms abstract the complete (raw text, automatic naming mode) fact.
Clearing automatic mode with equal text maps to a different atom; only equal
fixed metadata maps to the unchanged transition. Concrete refinement tests
check mode encoding and the creation-only automatic-mode admission. *)
EXTENDS Naturals, FiniteSets, Sequences, TLC

CONSTANTS ExerciseTargetDomain, ExerciseMove, MoveSingleton, MoveByteLimit, MoveRevisionLimit
ASSUME /\ ExerciseTargetDomain \in BOOLEAN
       /\ ExerciseMove \in BOOLEAN /\ MoveSingleton \in BOOLEAN
       /\ MoveByteLimit \in {2, 4} /\ MoveRevisionLimit \in {1, 2}

Clients == {1, 2}
Ids == {1, 2, 3}
Labels == 0..1
(* Panel 0 denotes a Tab title; Panel 1 is a distinct primary Presentation
override in that same Tab. For that target label 0 means absent/clear and label
1 means one normalized custom label. Immutable fallbacks/bindings and concrete
normalization are checked by the protocol and authority refinement tests. *)
TargetOne == [context |-> 1, tab |-> 1, panel |-> 0]
TargetTwo == [context |-> 2, tab |-> 2, panel |-> 0]
PanelTarget == [context |-> 1, tab |-> 1, panel |-> 1]
MissingTarget == [context |-> 1, tab |-> 2, panel |-> 0]
MissingPanelTarget == [context |-> 1, tab |-> 1, panel |-> 2]
NoTarget == [context |-> 0, tab |-> 0, panel |-> 0]
ExistingTargets == IF ExerciseTargetDomain THEN {TargetOne, TargetTwo, PanelTarget} ELSE {TargetOne}
Targets == IF ExerciseTargetDomain THEN ExistingTargets \cup {MissingTarget, MissingPanelTarget} ELSE ExistingTargets
SendIds(client) == IF ExerciseTargetDomain THEN {client} ELSE Ids
SendBases == IF ExerciseTargetDomain THEN 0..1 ELSE 0..2
SendGrants == IF ExerciseTargetDomain THEN {TRUE} ELSE BOOLEAN
InitialLabels == [target \in ExistingTargets |-> 0]
Empty == [target |-> NoTarget, label |-> 0, base |-> 0,
          result |-> "empty", resultRevision |-> 0]
Results == {"none", "denied", "conflict", "full", "stale", "missing",
            "unchanged", "applied", "unavailable", "read", "unknown"}
VARIABLE m
VARIABLES phase, request, grant, remaining, ledger, revision, labels, effects,
          bases, response, responseRevision, observed, observedRevision, retired,
          history, readRevision, readLabels, hasRead
navigationVars == <<phase, request, grant, remaining, ledger, revision, labels, effects,
          bases, response, responseRevision, observed, observedRevision, retired,
          history, readRevision, readLabels, hasRead>>

vars == <<navigationVars, m>>


(* The Move variants isolate the pure order/ledger owner from label editing.
Requests retain every semantic wire field; grants stay outside the fingerprint.
A response flight may be delivered or lost after commit, including retirement.
Immutable facts abstract complete Tab values and unrelated navigation/terminal
facts. Exact byte encoding and unsupported-layout preflight remain core tests. *)
MoveDirections == {"previous", "next", "first", "last"}
MoveTabs == IF MoveSingleton THEN <<1>> ELSE <<1, 2, 3>>
MoveMembers(order) == {order[i] : i \in 1..Len(order)}
MoveFacts == [tab \in {1, 2, 3} |->
  [panel |-> tab, presentation |-> tab, binding |-> tab, label |-> tab]]
MoveRequest(site, base, context, tab, direction) ==
  [site |-> site, base |-> base, context |-> context, tab |-> tab, direction |-> direction]
MoveRequests ==
  [site : {1}, base : 0..2, context : {1}, tab : {1, 3}, direction : MoveDirections]
  \cup {MoveRequest(2, 0, 1, 1, "first"),
        MoveRequest(1, 0, 2, 1, "first"),
        MoveRequest(1, 0, 1, 4, "first")}
MoveIndex(order, tab) == CHOOSE i \in 1..Len(order): order[i] = tab
MoveDestination(order, tab, direction) ==
  LET i == MoveIndex(order, tab) IN
  CASE direction = "first" -> 1
    [] direction = "last" -> Len(order)
    [] direction = "previous" -> IF i = 1 THEN 1 ELSE i - 1
    [] OTHER -> IF i = Len(order) THEN i ELSE i + 1
MovePermutation(order, tab, direction) ==
  LET i == MoveIndex(order, tab)
      destination == MoveDestination(order, tab, direction)
      without == SubSeq(order, 1, i - 1) \o SubSeq(order, i + 1, Len(order))
  IN SubSeq(without, 1, destination - 1) \o <<tab>>
       \o SubSeq(without, destination, Len(without))
MoveProspective(command) ==
  IF command.site # 1 THEN "site-mismatch"
  ELSE IF command.base # m.revision THEN "revision-mismatch"
  ELSE IF command.context # 1 THEN "context-unavailable"
  ELSE IF command.tab \notin MoveMembers(m.order) THEN "tab-unavailable"
  ELSE IF MovePermutation(m.order, command.tab, command.direction) = m.order
       THEN "unchanged"
  ELSE IF m.revision = MoveRevisionLimit THEN "revision-exhausted"
  ELSE "tab-moved"
MoveInitial == [
  order |-> MoveTabs, facts |-> MoveFacts, unrelated |-> "UNCHANGED",
  productCursor |-> 17, revision |-> 0, history |-> <<MoveTabs>>,
  ledger |-> <<>>, bytes |-> 0, effects |-> [id \in 1..3 |-> 0],
  phase |-> "ready", retired |-> FALSE, response |-> "none",
  responseRevision |-> 0, requestId |-> 1,
  command |-> MoveRequest(1, 0, 1, 1, "first"),
  observed |-> "none", observedRevision |-> 0, lost |-> FALSE, lostId |-> 0,
  kind |-> "none", readOrder |-> MoveTabs, readRevision |-> 0
]
MoveReady == ExerciseMove /\ m.phase = "ready"
MoveRespond(result, resultRevision, id, command, kind) ==
  [m EXCEPT !.phase = "responded", !.response = result,
    !.responseRevision = resultRevision, !.requestId = id,
    !.command = command, !.kind = kind]

MoveProcessNew(command) ==
  /\ MoveReady /\ ~m.retired
  /\ LET prospective == MoveProspective(command)
         full == Len(m.ledger) = 2 \/ m.bytes + 2 > MoveByteLimit
         result == IF full THEN "full" ELSE prospective
         changed == result = "tab-moved"
         resultRevision == IF full THEN 0 ELSE m.revision + (IF changed THEN 1 ELSE 0)
         id == Len(m.ledger) + 1
         next == MoveRespond(result, resultRevision, id, command, "new")
         order == IF changed THEN MovePermutation(m.order, command.tab, command.direction)
                  ELSE m.order
     IN m' = [next EXCEPT
          !.order = order,
          !.revision = m.revision + (IF changed THEN 1 ELSE 0),
          !.history = IF changed THEN Append(@, order) ELSE @,
          !.effects[id] = @ + (IF changed THEN 1 ELSE 0),
          !.ledger = IF full THEN @ ELSE Append(@,
            [command |-> command, result |-> result, revision |-> resultRevision]),
          !.bytes = @ + (IF full THEN 0 ELSE 2)]
  /\ UNCHANGED navigationVars

MovePrevious == \E command \in MoveRequests:
  /\ command.direction = "previous" /\ MoveProcessNew(command)
MoveNextDirection == \E command \in MoveRequests:
  /\ command.direction = "next" /\ MoveProcessNew(command)
MoveFirst == \E command \in MoveRequests:
  /\ command.direction = "first" /\ MoveProcessNew(command)
MoveLast == \E command \in MoveRequests:
  /\ command.direction = "last" /\ MoveProcessNew(command)

MoveDenied ==
  /\ MoveReady
  /\ m' = MoveRespond("denied", 0, 1, m.command, "denied")
  /\ UNCHANGED navigationVars
MoveRetiredCommand ==
  /\ MoveReady /\ m.retired
  /\ m' = MoveRespond("unavailable", 0, 1, m.command, "retired")
  /\ UNCHANGED navigationVars

MoveReplay ==
  /\ MoveReady /\ ~m.retired
  /\ \E id \in 1..Len(m.ledger):
       LET retained == m.ledger[id] IN
       m' = MoveRespond(retained.result, retained.revision,
                        id, retained.command, "replay")
  /\ UNCHANGED navigationVars

MoveReplayAfterLossAndLaterCommit ==
  /\ MoveReady /\ ~m.retired
  /\ m.lostId \in 1..Len(m.ledger)
  /\ LET retained == m.ledger[m.lostId] IN
       /\ retained.result = "tab-moved"
       /\ retained.revision < m.revision
       /\ m' = MoveRespond(retained.result, retained.revision,
                            m.lostId, retained.command, "replay")
  /\ UNCHANGED navigationVars

MoveConflict ==
  /\ MoveReady /\ ~m.retired
  /\ \E id \in 1..Len(m.ledger), command \in MoveRequests:
       /\ command # m.ledger[id].command
       /\ m' = MoveRespond("conflict", m.revision, id, command, "conflict")
  /\ UNCHANGED navigationVars

MoveDeliver ==
  /\ ExerciseMove /\ m.phase = "responded"
  /\ m' = [m EXCEPT !.phase = "ready", !.observed = "delivered",
                    !.observedRevision = 0, !.response = "none",
                    !.responseRevision = 0, !.requestId = 1, !.kind = "none",
                    !.command = MoveRequest(1, 0, 1, 1, "first")]
  /\ UNCHANGED navigationVars
MoveLoseResponse ==
  /\ ExerciseMove /\ m.phase = "responded"
  /\ m' = [m EXCEPT !.phase = "ready", !.observed = "unknown",
                    !.observedRevision = 0, !.lost = TRUE,
                    !.lostId = IF m.response = "tab-moved" THEN m.requestId ELSE @,
                    !.response = "none", !.responseRevision = 0,
                    !.requestId = 1, !.kind = "none",
                    !.command = MoveRequest(1, 0, 1, 1, "first")]
  /\ UNCHANGED navigationVars
MoveRetire ==
  /\ ExerciseMove /\ ~m.retired
  /\ m' = [m EXCEPT !.retired = TRUE]
  /\ UNCHANGED navigationVars
MoveRead ==
  /\ MoveReady /\ ~m.retired
  /\ m' = [m EXCEPT !.readOrder = m.order, !.readRevision = m.revision]
  /\ UNCHANGED navigationVars

MoveObserveNoopAtExhaustion ==
  /\ ExerciseMove /\ m.phase = "responded"
  /\ m.revision = MoveRevisionLimit /\ m.response = "unchanged"
  /\ UNCHANGED vars
MoveObserveEntryFull ==
  /\ ExerciseMove /\ m.phase = "responded"
  /\ Len(m.ledger) = 2 /\ m.response = "full"
  /\ UNCHANGED vars
MoveObserveByteFull ==
  /\ ExerciseMove /\ m.phase = "responded"
  /\ Len(m.ledger) < 2 /\ m.bytes + 2 > MoveByteLimit /\ m.response = "full"
  /\ UNCHANGED vars

MoveTypeOK ==
  /\ m.order \in Seq({1, 2, 3})
  /\ m.revision \in 0..MoveRevisionLimit
  /\ m.bytes = 2 * Len(m.ledger) /\ m.bytes <= MoveByteLimit
  /\ Len(m.ledger) <= 2
  /\ m.effects \in [1..3 -> 0..1]
  /\ m.phase \in {"ready", "responded"}
  /\ m.retired \in BOOLEAN /\ m.lost \in BOOLEAN
  /\ m.command \in MoveRequests /\ m.requestId \in 1..3
  /\ Len(m.history) = m.revision + 1
MovePreservesWholeMembers ==
  /\ MoveMembers(m.order) = MoveMembers(MoveTabs)
  /\ Len(m.order) = Len(MoveTabs)
  /\ m.facts = MoveFacts /\ m.unrelated = "UNCHANGED" /\ m.productCursor = 17
MoveRevisionCountsCommits ==
  m.revision = m.effects[1] + m.effects[2] + m.effects[3]
MoveRetainedCommitRevision ==
  \A id \in 1..Len(m.ledger):
    m.ledger[id].result = "tab-moved" =>
      /\ m.ledger[id].revision = m.ledger[id].command.base + 1
      /\ m.effects[id] = 1
MoveReadIsCommitted ==
  m.readOrder = m.history[m.readRevision + 1]
MoveReplayIsFixed == [][
  m'.kind = "replay" /\ m.phase = "ready" /\ m'.phase = "responded" =>
    /\ m'.response = m.ledger[m'.requestId].result
    /\ m'.responseRevision = m.ledger[m'.requestId].revision
    /\ UNCHANGED <<m.order, m.revision, m.ledger, m.bytes, m.effects>>
]_vars
MoveRetiredFences == [][m.retired =>
  UNCHANGED <<m.order, m.revision, m.ledger, m.bytes, m.effects>>]_vars
MoveLedgerIsImmutable == [][
  \A id \in 1..Len(m.ledger): m'.ledger[id] = m.ledger[id]
]_vars



Init == /\ m = MoveInitial
        /\ phase = [c \in Clients |-> "idle"]
        /\ request = [c \in Clients |->
             [id |-> 1, target |-> TargetOne, label |-> 1, base |-> 0]]
        /\ grant = [c \in Clients |-> TRUE]
        /\ remaining = [c \in Clients |-> IF ExerciseMove THEN 0 ELSE IF c = 1 THEN 2 ELSE 1]
        /\ ledger = [id \in Ids |-> Empty]
        /\ revision = 0 /\ labels = InitialLabels
        /\ effects = [id \in Ids |-> 0]
        /\ bases = [id \in Ids |-> 0]
        /\ response = [c \in Clients |-> "none"]
        /\ responseRevision = [c \in Clients |-> 0]
        /\ observed = [c \in Clients |-> "none"]
        /\ observedRevision = [c \in Clients |-> 0]
        /\ retired = FALSE /\ history = <<InitialLabels>>
        /\ readRevision = [c \in Clients |-> 0]
        /\ readLabels = [c \in Clients |-> InitialLabels]
        /\ hasRead = [c \in Clients |-> FALSE]

SendTo(targets, values) == \E c \in Clients:
          \E id \in SendIds(c), target \in targets,
             label \in values, base \in SendBases, allowed \in SendGrants:
        /\ phase[c] = "idle" /\ remaining[c] > 0
        /\ phase' = [phase EXCEPT ![c] = "pending"]
        /\ request' = [request EXCEPT ![c] =
             [id |-> id, target |-> target, label |-> label, base |-> base]]
        /\ grant' = [grant EXCEPT ![c] = allowed]
        /\ remaining' = [remaining EXCEPT ![c] = @ - 1]
        /\ UNCHANGED <<m, ledger, revision, labels, effects, bases, response,
                        responseRevision, observed, observedRevision>>
        /\ UNCHANGED <<m, retired, history, readRevision, readLabels, hasRead>>

\* Keep this action distinct in TLC coverage: a bare alias is reported only
\* as SendTo, which is also shared by the two Panel actions below.
Send == /\ TRUE /\ SendTo(Targets \ {PanelTarget}, Labels)
SendPanelSet == ExerciseTargetDomain /\ SendTo({PanelTarget}, {1})
SendPanelClear == ExerciseTargetDomain /\ SendTo({PanelTarget}, {0})

Pending(c) == phase[c] = "pending"
Respond(c, result, resultRevision) ==
  /\ phase' = [phase EXCEPT ![c] = "responded"]
  /\ response' = [response EXCEPT ![c] = result]
  /\ responseRevision' = [responseRevision EXCEPT ![c] = resultRevision]
StableInput == UNCHANGED <<m, request, grant, remaining, observed, observedRevision,
                           retired, readRevision, readLabels, hasRead>>
New(c) == /\ Pending(c) /\ grant[c] /\ ~retired
          /\ ledger[request[c].id] = Empty
Capacity == Cardinality({id \in Ids: ledger[id] # Empty}) < 2
Settle(c, result, resultRevision) == ledger' = [ledger EXCEPT ![request[c].id] =
  [target |-> request[c].target, label |-> request[c].label,
   base |-> request[c].base, result |-> result,
   resultRevision |-> resultRevision]]
Exact(c) == /\ ledger[request[c].id] # Empty
            /\ ledger[request[c].id].target = request[c].target
            /\ ledger[request[c].id].label = request[c].label
            /\ ledger[request[c].id].base = request[c].base

Deny == \E c \in Clients:
        /\ Pending(c) /\ ~grant[c] /\ Respond(c, "denied", 0) /\ StableInput
        /\ UNCHANGED <<m, ledger, revision, labels, effects, bases, history>>
Replay == \E c \in Clients:
          /\ Pending(c) /\ grant[c] /\ ~retired /\ Exact(c)
          /\ Respond(c, ledger[request[c].id].result,
                        ledger[request[c].id].resultRevision)
          /\ StableInput
          /\ UNCHANGED <<m, ledger, revision, labels, effects, bases, history>>
Conflict == \E c \in Clients:
            /\ Pending(c) /\ grant[c] /\ ~retired
            /\ ledger[request[c].id] # Empty /\ ~Exact(c)
            /\ Respond(c, "conflict", revision) /\ StableInput
            /\ UNCHANGED <<m, ledger, revision, labels, effects, bases, history>>
Full == \E c \in Clients:
        /\ New(c) /\ ~Capacity /\ Respond(c, "full", 0) /\ StableInput
        /\ UNCHANGED <<m, ledger, revision, labels, effects, bases, history>>
RejectStale == \E c \in Clients:
               /\ New(c) /\ Capacity /\ request[c].base # revision
               /\ Respond(c, "stale", revision)
               /\ Settle(c, "stale", revision) /\ StableInput
               /\ UNCHANGED <<m, revision, labels, effects, bases, history>>
RejectMissing == \E c \in Clients:
                 /\ New(c) /\ Capacity /\ request[c].base = revision
                 /\ request[c].target \notin ExistingTargets
                 /\ Respond(c, "missing", revision)
                 /\ Settle(c, "missing", revision) /\ StableInput
                 /\ UNCHANGED <<m, revision, labels, effects, bases, history>>
Unchanged == \E c \in Clients:
             /\ New(c) /\ Capacity /\ request[c].base = revision
             /\ request[c].target \in ExistingTargets
             /\ request[c].label = labels[request[c].target]
             /\ Respond(c, "unchanged", revision)
             /\ Settle(c, "unchanged", revision) /\ StableInput
             /\ UNCHANGED <<m, revision, labels, effects, bases, history>>
Commit == \E c \in Clients:
          /\ New(c) /\ Capacity /\ request[c].base = revision
          /\ request[c].target \in ExistingTargets
          /\ request[c].label # labels[request[c].target]
          /\ Respond(c, "applied", revision + 1)
          /\ Settle(c, "applied", revision + 1) /\ StableInput
          /\ revision' = revision + 1
          /\ labels' = [labels EXCEPT ![request[c].target] = request[c].label]
          /\ effects' = [effects EXCEPT ![request[c].id] = @ + 1]
          /\ bases' = [bases EXCEPT ![request[c].id] = revision]
          /\ history' = Append(history, labels')
Deliver == \E c \in Clients:
           /\ phase[c] = "responded"
           /\ phase' = [phase EXCEPT ![c] = "idle"]
           /\ observed' = [observed EXCEPT ![c] = response[c]]
           /\ observedRevision' =
                [observedRevision EXCEPT ![c] = responseRevision[c]]
           /\ UNCHANGED <<m, request, grant, remaining, ledger, revision, labels,
                           effects, bases, response, responseRevision, retired,
                           history, readRevision, readLabels, hasRead>>
LoseResponse == \E c \in Clients:
                /\ phase[c] = "responded"
                /\ phase' = [phase EXCEPT ![c] = "idle"]
                /\ observed' = [observed EXCEPT ![c] = "unknown"]
                /\ observedRevision' = [observedRevision EXCEPT ![c] = 0]
                /\ UNCHANGED <<m, request, grant, remaining, ledger, revision, labels,
                               effects, bases, response, responseRevision, retired,
                               history, readRevision, readLabels, hasRead>>

Read == \E c \in Clients:
        /\ phase[c] = "idle" /\ remaining[c] > 0 /\ grant[c] /\ ~retired
        /\ remaining' = [remaining EXCEPT ![c] = @ - 1]
        /\ observed' = [observed EXCEPT ![c] = "read"]
        /\ observedRevision' = [observedRevision EXCEPT ![c] = revision]
        /\ hasRead' = [hasRead EXCEPT ![c] = TRUE]
        /\ readRevision' = [readRevision EXCEPT ![c] = revision]
        /\ readLabels' = [readLabels EXCEPT ![c] = labels]
        /\ UNCHANGED <<m, phase, request, grant, ledger, revision, labels, effects,
                       bases, response, responseRevision, retired, history>>
DeniedRead == \E c \in Clients:
              /\ phase[c] = "idle" /\ remaining[c] > 0 /\ ~grant[c]
              /\ remaining' = [remaining EXCEPT ![c] = @ - 1]
              /\ observed' = [observed EXCEPT ![c] = "denied"]
              /\ observedRevision' = [observedRevision EXCEPT ![c] = 0]
              /\ UNCHANGED <<m, phase, request, grant, ledger, revision, labels,
                             effects, bases, response, responseRevision, retired,
                             history, readRevision, readLabels, hasRead>>
RetiredRead == \E c \in Clients:
               /\ phase[c] = "idle" /\ remaining[c] > 0 /\ grant[c] /\ retired
               /\ remaining' = [remaining EXCEPT ![c] = @ - 1]
               /\ observed' = [observed EXCEPT ![c] = "unavailable"]
               /\ observedRevision' = [observedRevision EXCEPT ![c] = 0]
               /\ UNCHANGED <<m, phase, request, grant, ledger, revision, labels,
                              effects, bases, response, responseRevision, retired,
                              history, readRevision, readLabels, hasRead>>
Retire == /\ ~ExerciseMove /\ ~retired /\ retired' = TRUE
          /\ UNCHANGED <<m, phase, request, grant, remaining, ledger, revision,
                         labels, effects, bases, response, responseRevision,
                         observed, observedRevision, history, readRevision,
                         readLabels, hasRead>>
Unavailable == \E c \in Clients:
               /\ Pending(c) /\ grant[c] /\ retired
               /\ Respond(c, "unavailable", 0) /\ StableInput
               /\ UNCHANGED <<m, ledger, revision, labels, effects, bases, history>>

(* Coverage witness: an exact retained request is still replayable after a
different request has advanced the shared revision. ExactReplayKeepsResultRevision
then checks that the response uses the retained result revision. *)
ObserveHistoricalReplay == \E c \in Clients:
  /\ Pending(c) /\ grant[c] /\ ~retired /\ Exact(c)
  /\ ledger[request[c].id].resultRevision < revision
  /\ UNCHANGED vars

Next == Send \/ SendPanelSet \/ SendPanelClear \/ Deny \/ Replay \/ Conflict \/ Full \/ RejectStale
        \/ RejectMissing \/ Unchanged \/ Commit \/ Deliver \/ LoseResponse
        \/ Read \/ DeniedRead \/ RetiredRead \/ Retire \/ Unavailable
        \/ ObserveHistoricalReplay
        \/ MovePrevious \/ MoveNextDirection \/ MoveFirst \/ MoveLast
        \/ MoveDenied \/ MoveRetiredCommand \/ MoveReplay
        \/ MoveReplayAfterLossAndLaterCommit \/ MoveConflict
        \/ MoveDeliver \/ MoveLoseResponse \/ MoveRetire \/ MoveRead
        \/ MoveObserveNoopAtExhaustion \/ MoveObserveEntryFull \/ MoveObserveByteFull
Spec == Init /\ [][Next]_vars
TypeOK == /\ phase \in [Clients -> {"idle", "pending", "responded"}]
          /\ remaining \in [Clients -> 0..2] /\ grant \in [Clients -> BOOLEAN]
          /\ revision \in 0..2 /\ labels \in [ExistingTargets -> Labels]
          /\ effects \in [Ids -> 0..1]
          /\ request \in [Clients ->
               [id: Ids, target: Targets, label: Labels, base: 0..2]]
          /\ ledger \in [Ids ->
               [target: ExistingTargets \cup {MissingTarget, MissingPanelTarget, NoTarget},
                label: Labels, base: 0..2,
                result: {"empty", "stale", "missing", "unchanged", "applied"},
                resultRevision: 0..2]]
          /\ bases \in [Ids -> 0..2]
          /\ response \in [Clients -> Results]
          /\ responseRevision \in [Clients -> 0..2]
          /\ observed \in [Clients -> Results]
          /\ observedRevision \in [Clients -> 0..2]
          /\ retired \in BOOLEAN /\ hasRead \in [Clients -> BOOLEAN]
          /\ readRevision \in [Clients -> 0..2]
          /\ readLabels \in [Clients -> [ExistingTargets -> Labels]]
          /\ history \in Seq([ExistingTargets -> Labels])
          /\ Len(history) = revision + 1
ReadIsCommitted == \A c \in Clients: hasRead[c] =>
  (readRevision[c] <= revision /\ readLabels[c] = history[readRevision[c] + 1])
RetiredCannotCommit == [][retired =>
  (retired' /\ ledger' = ledger /\ revision' = revision
   /\ labels' = labels /\ effects' = effects)]_vars
ReadAuthorizationPrecedence == [][\A c \in Clients:
  (remaining'[c] < remaining[c] /\ phase[c] = "idle" /\ phase'[c] = "idle") =>
  observed'[c] = (IF ~grant[c] THEN "denied"
                  ELSE IF retired THEN "unavailable" ELSE "read")]_vars
ExactReplayKeepsResultRevision == [][\A c \in Clients:
  (Pending(c) /\ grant[c] /\ ~retired /\ Exact(c)
   /\ phase'[c] = "responded") =>
  (response'[c] = ledger[request[c].id].result
   /\ responseRevision'[c] = ledger[request[c].id].resultRevision)]_vars
CommitChangesOnlyTarget == [][\A c \in Clients:
  (Pending(c) /\ phase'[c] = "responded" /\ response'[c] = "applied") =>
    \A target \in ExistingTargets \ {request[c].target}:
      labels'[target] = labels[target]]_vars
AtMostOnce == \A id \in Ids: effects[id] <= 1
RevisionAccountsForEffects == revision = effects[1] + effects[2] + effects[3]
OneCommitPerBase == \A i, j \in Ids:
  (i # j /\ effects[i] = 1 /\ effects[j] = 1) => bases[i] # bases[j]
LedgerBound == Cardinality({id \in Ids: ledger[id] # Empty}) <= 2
CommittedRecorded == \A id \in Ids: effects[id] = 1 =>
  (ledger[id] # Empty /\ ledger[id].result = "applied"
   /\ ledger[id].resultRevision = ledger[id].base + 1)
UnauthorizedCannotSettle == [][(\E c \in Clients:
  Pending(c) /\ ~grant[c] /\ phase'[c] = "responded") =>
  (ledger' = ledger /\ revision' = revision /\ labels' = labels
   /\ effects' = effects)]_vars
=============================================================================
