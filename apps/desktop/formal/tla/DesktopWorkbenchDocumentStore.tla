-------------------- MODULE DesktopWorkbenchDocumentStore --------------------
EXTENDS FiniteSets, Integers, Sequences, TLC

Clients == {"generation-1", "generation-2"}
NoClient == "no-client"
NoRevision == -1
MaxRevision == 2
Revisions == 0..MaxRevision
ExpectedRevisions == Revisions \cup {NoRevision}
RequestStates == {"idle", "queued", "active", "completed"}
Outcomes == {"none", "available", "missing", "committed", "conflict",
  "rejected", "unavailable", "suppressed"}
VARIABLES
  storedRevision,
  writable,
  connected,
  requestState,
  expectedRevision,
  proposedRevision,
  outcome,
  queue,
  activeClient

vars == <<storedRevision, writable, connected, requestState, expectedRevision,
  proposedRevision, outcome, queue, activeClient>>

\* Step properties observe pre/post state directly; diagnostic history is not model state.
Init ==
  /\ storedRevision \in ExpectedRevisions
  /\ writable \in BOOLEAN
  /\ connected = [client \in Clients |-> TRUE]
  /\ requestState = [client \in Clients |-> "idle"]
  /\ expectedRevision = [client \in Clients |-> NoRevision]
  /\ proposedRevision = [client \in Clients |-> 0]
  /\ outcome = [client \in Clients |-> "none"]
  /\ queue = <<>>
  /\ activeClient = NoClient

Load(client) ==
  /\ connected[client]
  /\ requestState[client] = "idle"
  /\ requestState' = [requestState EXCEPT ![client] = "completed"]
  /\ outcome' = [outcome EXCEPT ![client] =
       IF ~writable
       THEN "unavailable"
       ELSE IF storedRevision = NoRevision THEN "missing" ELSE "available"]
  /\ UNCHANGED <<storedRevision, writable, connected, expectedRevision,
       proposedRevision, queue, activeClient>>


Submit(client, expected, proposed) ==
  /\ connected[client]
  /\ requestState[client] = "idle"
  /\ expected \in ExpectedRevisions
  /\ proposed \in Revisions
  /\ requestState' = [requestState EXCEPT ![client] = "queued"]
  /\ expectedRevision' = [expectedRevision EXCEPT ![client] = expected]
  /\ proposedRevision' = [proposedRevision EXCEPT ![client] = proposed]
  /\ outcome' = [outcome EXCEPT ![client] = "none"]
  /\ queue' = Append(queue, client)
  /\ UNCHANGED <<storedRevision, writable, connected, activeClient>>


StartNext ==
  /\ activeClient = NoClient
  /\ Len(queue) > 0
  /\ LET client == Head(queue) IN
       /\ connected[client]
       /\ activeClient' = client
       /\ queue' = Tail(queue)
       /\ requestState' = [requestState EXCEPT ![client] = "active"]
       /\ UNCHANGED <<storedRevision, writable, connected, expectedRevision,
            proposedRevision, outcome>>


DropDisconnectedHead ==
  /\ activeClient = NoClient
  /\ Len(queue) > 0
  /\ LET client == Head(queue) IN
       /\ ~connected[client]
       /\ activeClient' = NoClient
       /\ queue' = Tail(queue)
       /\ requestState' = [requestState EXCEPT ![client] = "completed"]
       /\ outcome' = [outcome EXCEPT ![client] = "suppressed"]
       /\ UNCHANGED <<storedRevision, writable, connected, expectedRevision,
            proposedRevision>>


AdvanceQueue == StartNext \/ DropDisconnectedHead

CompletionResult(client) ==
  IF ~writable
  THEN "unavailable"
  ELSE IF proposedRevision[client] # expectedRevision[client] + 1
  THEN "rejected"
  ELSE IF storedRevision # expectedRevision[client]
  THEN "conflict"
  ELSE "committed"

Complete(client) ==
  /\ activeClient = client
  /\ requestState[client] = "active"
  /\ LET result == CompletionResult(client) IN
       /\ storedRevision' =
            IF result = "committed" THEN proposedRevision[client] ELSE storedRevision
       /\ requestState' = [requestState EXCEPT ![client] = "completed"]
       /\ outcome' = [outcome EXCEPT ![client] =
            IF connected[client] THEN result ELSE "suppressed"]
       /\ activeClient' = NoClient
       /\ UNCHANGED <<writable, connected, expectedRevision, proposedRevision, queue>>


Dispose(client) ==
  /\ connected[client]
  /\ connected' = [connected EXCEPT ![client] = FALSE]
  /\ UNCHANGED <<storedRevision, writable, requestState, expectedRevision,
       proposedRevision, outcome, queue, activeClient>>


Next ==
  \/ \E client \in Clients: Load(client)
  \/ \E client \in Clients, expected \in ExpectedRevisions, proposed \in Revisions:
       Submit(client, expected, proposed)
  \/ StartNext
  \/ DropDisconnectedHead
  \/ \E client \in Clients: Complete(client)
  \/ \E client \in Clients: Dispose(client)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AdvanceQueue)
  /\ \A client \in Clients: WF_vars(Complete(client))

QueueClients == {queue[index] : index \in 1..Len(queue)}

TypeOK ==
  /\ storedRevision \in ExpectedRevisions
  /\ writable \in BOOLEAN
  /\ connected \in [Clients -> BOOLEAN]
  /\ requestState \in [Clients -> RequestStates]
  /\ expectedRevision \in [Clients -> ExpectedRevisions]
  /\ proposedRevision \in [Clients -> Revisions]
  /\ outcome \in [Clients -> Outcomes]
  /\ queue \in Seq(Clients)
  /\ activeClient \in Clients \cup {NoClient}

QueueIntegrity ==
  /\ Len(queue) = Cardinality(QueueClients)
  /\ \A client \in Clients:
       (requestState[client] = "queued") <=> (client \in QueueClients)
  /\ (activeClient = NoClient) <=>
       (\A client \in Clients: requestState[client] # "active")
  /\ activeClient # NoClient => requestState[activeClient] = "active"

RevisionNeverDecreases == [][storedRevision' >= storedRevision]_vars

CommitAdvancesExactlyOne ==
  [][\A client \in Clients: Complete(client) /\ storedRevision' # storedRevision =>
    /\ storedRevision' = storedRevision + 1
    /\ expectedRevision[client] = storedRevision
    /\ proposedRevision[client] = storedRevision']_vars

NonCommitPreservesStore ==
  [][storedRevision' # storedRevision =>
    \E client \in Clients: Complete(client) /\ CompletionResult(client) = "committed"]_vars

UnavailableStoreNeverCommits == [][~writable => UNCHANGED storedRevision]_vars

DisposedCompletionIsSuppressed ==
  [][\A client \in Clients: Complete(client) /\ ~connected[client] => outcome'[client] = "suppressed"]_vars

StartedRequestWasConnected == [][StartNext => connected[activeClient']]_vars

DroppedDisconnectedRequestIsSuppressed ==
  [][DropDisconnectedHead =>
    LET client == Head(queue) IN
      /\ ~connected[client]
      /\ requestState'[client] = "completed"
      /\ outcome'[client] = "suppressed"
      /\ activeClient' = NoClient
      /\ UNCHANGED storedRevision]_vars

QueuedRequestEventuallySettles ==
  \A client \in Clients:
    requestState[client] = "queued" ~> requestState[client] = "completed"

=============================================================================
