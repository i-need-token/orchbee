------------------------ MODULE WorkspaceTabMove ------------------------
EXTENDS Naturals, FiniteSets, Sequences, TLC

(* Context-local TabMove refinement of WorkbenchNavigationCommit.
   Two contexts each own three complete immutable Tab records.
   The local Tab id is an index; abstract identity is (owner, id). Protocol
   TabID values are Site-wide unique. A wrong-context TabUnavailable outcome
   is outside this model's valid-target abstraction.
   At most two first-seen commands settle in one shared ledger; retries may repeat forever
   without extending it. Atomic actions abstract serialized server commits.
   The standalone bootstrap slot independently starts Empty/Creating/Settled.
   Authentication, retirement, capacity/overflow, loss/delivery and singleton
   cases remain in WorkbenchNavigationCommit; slot creation is not a writer
   in this model. No fairness or progress claim is made. No state constraint,
   symmetry reduction, runtime scheduling or implementation proof is assumed. *)

Contexts == {"standalone", "workspace"}
Directions == {"previous", "next", "first", "last"}
TabIds == 1..3
RequestLimit == 2
FullTab(context, id) ==
  [id |-> id, owner |-> context, label |-> <<context, id>>,
   terminal |-> [session |-> <<context, id>>, generation |-> 1]]
InitialOrders == [c \in Contexts |-> [i \in TabIds |-> FullTab(c, i)]]
Members(order) == {order[i] : i \in 1..Len(order)}
Position(order, tab) == CHOOSE i \in 1..Len(order): order[i].id = tab
Destination(order, tab, direction) ==
  LET i == Position(order, tab) IN
    CASE direction = "previous" -> IF i = 1 THEN i ELSE i - 1
      [] direction = "next" -> IF i = Len(order) THEN i ELSE i + 1
      [] direction = "first" -> 1
      [] OTHER -> Len(order)
Permutation(order, tab, direction) ==
  LET source == Position(order, tab)
      target == Destination(order, tab, direction)
  IN [i \in 1..Len(order) |->
       IF i = target THEN order[source]
       ELSE IF target < source /\ i > target /\ i <= source THEN order[i - 1]
       ELSE IF target > source /\ i >= source /\ i < target THEN order[i + 1]
       ELSE order[i]]
Command(context, tab, direction, base) ==
  [context |-> context, tab |-> tab, direction |-> direction, base |-> base]
Commands == [context: Contexts, tab: TabIds, direction: Directions,
             base: 0..RequestLimit]
Reply(kind, id, command, result, resultRevision) ==
  [kind |-> kind, id |-> id, command |-> command,
   result |-> result, revision |-> resultRevision]

VARIABLES orders, revision, ledger, commits, standaloneSlot, initialSlot,
          terminalEffects, productCursor, response
vars == <<orders, revision, ledger, commits, standaloneSlot, initialSlot,
          terminalEffects, productCursor, response>>
domainVars == <<orders, revision, ledger, commits>>
independentVars == <<standaloneSlot, initialSlot, terminalEffects, productCursor>>

Init ==
  /\ orders = InitialOrders /\ revision = 0 /\ ledger = <<>>
  /\ commits = [c \in Contexts |-> 0]
  /\ initialSlot \in {"Empty", "Creating", "Settled"}
  /\ standaloneSlot = initialSlot
  /\ terminalEffects = <<>> /\ productCursor = 17
  /\ response = Reply("none", 1, Command("standalone", 1, "first", 0), "none", 0)

(* One direct action owns all direction dispatch. Separate direct witnesses
   below make direction and context coverage visible without action aliases. *)
NewMove ==
  /\ Len(ledger) < RequestLimit
  /\ \E context \in Contexts, tab \in TabIds, direction \in Directions,
        base \in {0, revision}:
       LET command == Command(context, tab, direction, base)
           moved == Permutation(orders[context], tab, direction)
           changed == base = revision /\ moved # orders[context]
           result == IF base # revision THEN "stale"
                     ELSE IF changed THEN "tab-moved" ELSE "unchanged"
           resultRevision == revision + (IF changed THEN 1 ELSE 0)
       IN /\ orders' = IF changed THEN [orders EXCEPT ![context] = moved] ELSE orders
          /\ revision' = resultRevision
          /\ commits' = [commits EXCEPT ![context] = @ + (IF changed THEN 1 ELSE 0)]
          /\ ledger' = Append(ledger,
               [command |-> command, result |-> result, revision |-> resultRevision])
          /\ response' = Reply("new", Len(ledger) + 1, command, result, resultRevision)
  /\ UNCHANGED independentVars

Replay ==
  /\ \E id \in 1..Len(ledger):
       /\ ledger[id].revision = revision
       /\ response' = Reply("replay", id, ledger[id].command,
                            ledger[id].result, ledger[id].revision)
  /\ UNCHANGED <<domainVars, independentVars>>

HistoricalReplay ==
  /\ \E id \in 1..Len(ledger):
       /\ ledger[id].revision < revision
       /\ response' = Reply("replay", id, ledger[id].command,
                            ledger[id].result, ledger[id].revision)
  /\ UNCHANGED <<domainVars, independentVars>>

(* Change ONLY context in an otherwise identical retained fingerprint. *)
CrossContextConflict ==
  /\ \E id \in 1..Len(ledger), context \in Contexts:
       /\ context # ledger[id].command.context
       /\ LET command == [ledger[id].command EXCEPT !.context = context]
          IN response' = Reply("conflict", id, command, "conflict", revision)
  /\ UNCHANGED <<domainVars, independentVars>>

ObservePrevious == /\ response.kind = "new" /\ response.command.direction = "previous"
                   /\ response.result = "tab-moved" /\ UNCHANGED vars
ObserveNext == /\ response.kind = "new" /\ response.command.direction = "next"
               /\ response.result = "tab-moved" /\ UNCHANGED vars
ObserveFirst == /\ response.kind = "new" /\ response.command.direction = "first"
                /\ response.result = "tab-moved" /\ UNCHANGED vars
ObserveLast == /\ response.kind = "new" /\ response.command.direction = "last"
               /\ response.result = "tab-moved" /\ UNCHANGED vars
ObserveNoop == /\ response.kind = "new" /\ response.result = "unchanged"
               /\ UNCHANGED vars
ObserveStale == /\ response.kind = "new" /\ response.result = "stale"
                /\ UNCHANGED vars
ObserveBothContextsCommitted == /\ commits["standalone"] = 1
                               /\ commits["workspace"] = 1 /\ UNCHANGED vars
ObserveHistoricalMoveReplay == /\ response.kind = "replay"
                               /\ response.result = "tab-moved"
                               /\ response.revision < revision /\ UNCHANGED vars
ObserveEmptySlot == /\ standaloneSlot = "Empty" /\ UNCHANGED vars
ObserveCreatingSlot == /\ standaloneSlot = "Creating" /\ UNCHANGED vars
ObserveSettledSlot == /\ standaloneSlot = "Settled" /\ UNCHANGED vars
ObserveWorkspaceMoveWithEmptySlot ==
  /\ response.kind = "new" /\ response.command.context = "workspace"
  /\ response.result = "tab-moved" /\ standaloneSlot = "Empty"
  /\ UNCHANGED vars
ObserveWorkspaceMoveWithCreatingSlot ==
  /\ response.kind = "new" /\ response.command.context = "workspace"
  /\ response.result = "tab-moved" /\ standaloneSlot = "Creating"
  /\ UNCHANGED vars
ObserveWorkspaceMoveWithSettledSlot ==
  /\ response.kind = "new" /\ response.command.context = "workspace"
  /\ response.result = "tab-moved" /\ standaloneSlot = "Settled"
  /\ UNCHANGED vars

Next == NewMove \/ Replay \/ HistoricalReplay \/ CrossContextConflict
        \/ ObservePrevious \/ ObserveNext \/ ObserveFirst \/ ObserveLast
        \/ ObserveNoop \/ ObserveStale \/ ObserveBothContextsCommitted
        \/ ObserveHistoricalMoveReplay
        \/ ObserveEmptySlot \/ ObserveCreatingSlot \/ ObserveSettledSlot
        \/ ObserveWorkspaceMoveWithEmptySlot \/ ObserveWorkspaceMoveWithCreatingSlot
        \/ ObserveWorkspaceMoveWithSettledSlot
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ orders \in [Contexts -> Seq({FullTab(c, i): c \in Contexts, i \in TabIds})]
  /\ revision \in 0..RequestLimit /\ commits \in [Contexts -> 0..RequestLimit]
  /\ ledger \in Seq([command: Commands, result: {"stale", "unchanged", "tab-moved"},
                     revision: 0..RequestLimit])
  /\ Len(ledger) <= RequestLimit
  /\ initialSlot \in {"Empty", "Creating", "Settled"}
  /\ response \in [kind: {"none", "new", "replay", "conflict"}, id: 1..RequestLimit,
                     command: Commands, result: {"none", "stale", "unchanged", "tab-moved", "conflict"},
                     revision: 0..RequestLimit]
WholeTabIdentity == \A c \in Contexts:
  /\ Len(orders[c]) = 3 /\ Members(orders[c]) = Members(InitialOrders[c])
IndependentStatePreserved ==
  /\ standaloneSlot = initialSlot /\ terminalEffects = <<>> /\ productCursor = 17
SharedRevision == revision = commits["standalone"] + commits["workspace"]
RetainedCommitRevision == \A id \in 1..Len(ledger):
  ledger[id].result = "tab-moved" => ledger[id].revision = ledger[id].command.base + 1
AtMostOnce == revision = Cardinality({id \in 1..Len(ledger): ledger[id].result = "tab-moved"})

ContextIsolation == [][orders' # orders =>
  /\ response'.kind = "new" /\ response'.result = "tab-moved"
  /\ \A c \in Contexts \ {response'.command.context}: orders'[c] = orders[c]
]_vars
AtomicRevision == [][
  /\ revision' = revision + (IF orders' # orders THEN 1 ELSE 0)
  /\ orders' # orders => Len(ledger') = Len(ledger) + 1
]_vars
ReplayFixed == [][response'.kind = "replay" =>
  /\ response'.command = ledger[response'.id].command
  /\ response'.result = ledger[response'.id].result
  /\ response'.revision = ledger[response'.id].revision
  /\ UNCHANGED domainVars
]_vars
ConflictPreservesState == [][response'.kind = "conflict" =>
  /\ response'.result = "conflict"
  /\ response'.command.context # ledger[response'.id].command.context
  /\ response'.command = [ledger[response'.id].command EXCEPT
                           !.context = response'.command.context]
  /\ UNCHANGED domainVars
]_vars
NewNonCommitPreservesOrder == [][
  response'.kind = "new" /\ response'.result \in {"unchanged", "stale"} =>
    UNCHANGED <<orders, revision, commits>>
]_vars
LedgerImmutable == [][\A id \in 1..Len(ledger): ledger'[id] = ledger[id]]_vars
=============================================================================
