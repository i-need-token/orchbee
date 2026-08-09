--------------------- MODULE TerminalInventoryPagination ---------------------
EXTENDS FiniteSets, Integers, Sequences, TLC

\* Refines runtime-model claims TH-002 and TH-012. Create, Dispose, BeginScan,
\* ReadPage, and RejectStaleCursor model one generation-scoped registry snapshot
\* without duplicate, omitted, or cross-generation members.

CONSTANTS TerminalCount, MaxGeneration

None == -1
ScanStates == {"idle", "active", "stale", "complete"}
TerminalIds == 1..TerminalCount
TerminalOrder == [index \in 1..TerminalCount |-> index]

VARIABLES
  present,
  generation,
  scanState,
  scanGeneration,
  scanMembership,
  nextOffset,
  seen,
  pageCount

vars == <<present, generation, scanState, scanGeneration, scanMembership,
          nextOffset, seen, pageCount>>

OrderIsCanonical ==
  /\ Len(TerminalOrder) = Cardinality(TerminalIds)
  /\ {TerminalOrder[index]: index \in 1..Len(TerminalOrder)} = TerminalIds

HasNext ==
  \E index \in (nextOffset + 1)..Len(TerminalOrder):
    TerminalOrder[index] \in scanMembership

NextIndex ==
  CHOOSE index \in (nextOffset + 1)..Len(TerminalOrder):
    /\ TerminalOrder[index] \in scanMembership
    /\ \A earlier \in (nextOffset + 1)..(index - 1):
         TerminalOrder[earlier] \notin scanMembership

Init ==
  /\ OrderIsCanonical
  /\ present = {}
  /\ generation = 0
  /\ scanState = "idle"
  /\ scanGeneration = None
  /\ scanMembership = {}
  /\ nextOffset = 0
  /\ seen = {}
  /\ pageCount = 0

Create(terminal) ==
  /\ terminal \in TerminalIds \ present
  /\ generation < MaxGeneration
  /\ present' = present \cup {terminal}
  /\ generation' = generation + 1
  /\ UNCHANGED <<scanState, scanGeneration, scanMembership,
                  nextOffset, seen, pageCount>>

Dispose(terminal) ==
  /\ terminal \in present
  /\ generation < MaxGeneration
  /\ present' = present \ {terminal}
  /\ generation' = generation + 1
  /\ UNCHANGED <<scanState, scanGeneration, scanMembership,
                  nextOffset, seen, pageCount>>

BeginScan ==
  /\ scanState \in {"idle", "stale", "complete"}
  /\ scanGeneration' = generation
  /\ scanMembership' = present
  /\ nextOffset' = 0
  /\ seen' = {}
  /\ pageCount' = 0
  /\ scanState' = IF present = {} THEN "complete" ELSE "active"
  /\ UNCHANGED <<present, generation>>

ReadPage ==
  /\ scanState = "active"
  /\ scanGeneration = generation
  /\ HasNext
  /\ LET index == NextIndex
         terminal == TerminalOrder[index]
     IN
       /\ nextOffset' = index
       /\ seen' = seen \cup {terminal}
       /\ pageCount' = pageCount + 1
       /\ scanState' =
            IF \E later \in (index + 1)..Len(TerminalOrder):
                 TerminalOrder[later] \in scanMembership
            THEN "active"
            ELSE "complete"
  /\ UNCHANGED <<present, generation, scanGeneration, scanMembership>>

RejectStaleCursor ==
  /\ scanState = "active"
  /\ scanGeneration # generation
  /\ scanState' = "stale"
  /\ UNCHANGED <<present, generation, scanGeneration, scanMembership,
                  nextOffset, seen, pageCount>>

Next ==
  \/ \E terminal \in TerminalIds: Create(terminal)
  \/ \E terminal \in TerminalIds: Dispose(terminal)
  \/ BeginScan
  \/ ReadPage
  \/ RejectStaleCursor

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ present \subseteq TerminalIds
  /\ generation \in 0..MaxGeneration
  /\ scanState \in ScanStates
  /\ scanGeneration \in (0..MaxGeneration) \cup {None}
  /\ scanMembership \subseteq TerminalIds
  /\ nextOffset \in 0..Len(TerminalOrder)
  /\ seen \subseteq TerminalIds
  /\ pageCount \in 0..Len(TerminalOrder)

SeenBelongsToSnapshot == seen \subseteq scanMembership

NoDuplicatePages == Cardinality(seen) = pageCount

CompleteScanIsExact ==
  scanState = "complete" => seen = scanMembership

StaleCursorNeverCompletes ==
  scanState = "stale" => seen \subseteq scanMembership

MembershipMutationChangesGeneration ==
  [][\A terminal \in TerminalIds:
       (Create(terminal) \/ Dispose(terminal)) => generation' # generation]_vars

StaleCursorCannotAdvance ==
  [][RejectStaleCursor =>
       UNCHANGED <<nextOffset, seen, pageCount>>]_vars

=============================================================================
