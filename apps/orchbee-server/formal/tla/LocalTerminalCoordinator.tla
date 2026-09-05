---------------------- MODULE LocalTerminalCoordinator ----------------------
EXTENDS Naturals, FiniteSets, TLC

\* The coordinator refinement covers one deferred binding and two peer Sessions.
\* Wire delivery and Worker/Host admission remain in LocalWorkerObcnCarrier and
\* TerminalLaunchAuthorization. This model checks the Server reservation/commit
\* boundary; geometry/capacity/codec details have deterministic runtime witnesses.
CONSTANTS Clients, MaxOperation, MaxGeneration
VARIABLES phase, workerGeneration, bindingGeneration, operation, nextOperation,
  fence, issued, pendingClient, connected, response, published, committedOperation, responseOperation
vars == <<phase, workerGeneration, bindingGeneration, operation, nextOperation,
  fence, issued, pendingClient, connected, response, published, committedOperation, responseOperation>>
Pending == phase \in {"creating", "starting"}
Init ==
  /\ phase = "absent"
  /\ workerGeneration = 1
  /\ bindingGeneration = 0
  /\ operation = 0
  /\ nextOperation = 1
  /\ fence = 0
  /\ issued = {}
  /\ pendingClient = "none"
  /\ connected = Clients
  /\ response = [client \in Clients |-> "none"]
  /\ published = [client \in Clients |-> "none"]
  /\ committedOperation = [client \in Clients |-> 0]
  /\ responseOperation = [client \in Clients |-> 0]

IssueCreate(client) ==
  /\ client \in connected
  /\ phase = "absent"
  /\ response[client] = "none"
  /\ nextOperation <= MaxOperation
  /\ phase' = "creating"
  /\ bindingGeneration' = workerGeneration
  /\ operation' = nextOperation
  /\ nextOperation' = nextOperation + 1
  /\ issued' = issued \cup {nextOperation}
  /\ fence' = 1
  /\ pendingClient' = client
  /\ UNCHANGED <<workerGeneration, connected, response, published, committedOperation, responseOperation>>
IssueStart(client) ==
  /\ client \in connected
  /\ phase = "awaiting"
  /\ bindingGeneration = workerGeneration
  /\ response[client] = "none"
  /\ nextOperation <= MaxOperation
  /\ phase' = "starting"
  /\ operation' = nextOperation
  /\ nextOperation' = nextOperation + 1
  /\ issued' = issued \cup {nextOperation}
  /\ fence' = fence + 1
  /\ pendingClient' = client
  /\ UNCHANGED <<bindingGeneration, workerGeneration, connected, response, published, committedOperation, responseOperation>>
RejectConcurrent(client) ==
  /\ client \in connected
  /\ Pending
  /\ UNCHANGED vars
CompleteExact ==
  /\ Pending
  /\ workerGeneration = bindingGeneration
  /\ phase' = IF phase = "creating" THEN "awaiting" ELSE "started"
  /\ response' = [response EXCEPT ![pendingClient] = "accepted"]
  /\ committedOperation' = [committedOperation EXCEPT ![pendingClient] = operation]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published>>
CompleteUnknown ==
  /\ Pending
  /\ phase' = "unknown"
  /\ response' = [response EXCEPT ![pendingClient] = "unknown"]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, committedOperation>>
CompleteNotDelivered ==
  /\ Pending
  /\ phase' = IF phase = "creating" THEN "unavailable" ELSE "awaiting"
  /\ response' = [response EXCEPT ![pendingClient] = "unavailable"]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, committedOperation>>
CompleteRejected ==
  /\ Pending
  /\ phase' = "unavailable"
  /\ response' = [response EXCEPT ![pendingClient] = "unavailable"]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, committedOperation>>
CompleteStale ==
  /\ Pending
  /\ workerGeneration # bindingGeneration
  /\ CompleteUnknown
Publish(client) ==
  /\ response[client] # "none"
  /\ client \in connected
  /\ published' = [published EXCEPT ![client] = response[client]]
  /\ response' = [response EXCEPT ![client] = "none"]
  /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
       nextOperation, fence, issued, connected, pendingClient, committedOperation, responseOperation>>
AbandonPublication(client) ==
  /\ response[client] # "none"
  /\ client \notin connected
  /\ response' = [response EXCEPT ![client] = "none"]
  /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
       nextOperation, fence, issued, connected, pendingClient, published, committedOperation, responseOperation>>
Disconnect(client) ==
  /\ client \in connected
  /\ connected' = connected \ {client}
  /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
       nextOperation, fence, issued, pendingClient, response, published, committedOperation, responseOperation>>
ReplaceWorker ==
  /\ workerGeneration < MaxGeneration
  /\ workerGeneration' = workerGeneration + 1
  /\ UNCHANGED <<phase, bindingGeneration, operation, nextOperation, fence,
       issued, pendingClient, connected, response, published, committedOperation, responseOperation>>
Next ==
  \/ \E client \in Clients: IssueCreate(client) \/ IssueStart(client)
       \/ RejectConcurrent(client) \/ Disconnect(client)
  \/ CompleteExact \/ CompleteUnknown \/ CompleteNotDelivered \/ CompleteRejected \/ CompleteStale
  \/ (\E client \in Clients: Publish(client) \/ AbandonPublication(client))
  \/ ReplaceWorker
Settle == CompleteExact \/ CompleteUnknown \/ CompleteNotDelivered \/ CompleteRejected \/ CompleteStale
FinishPublication(client) == Publish(client) \/ AbandonPublication(client)
Spec == Init /\ [][Next]_vars /\ WF_vars(Settle) /\ (\A client \in Clients: WF_vars(FinishPublication(client)))
TypeOK ==
  /\ phase \in {"absent", "creating", "awaiting", "starting", "started", "unknown", "unavailable"}
  /\ workerGeneration \in 1..MaxGeneration
  /\ bindingGeneration \in 0..MaxGeneration
  /\ operation \in 0..MaxOperation
  /\ nextOperation \in 1..(MaxOperation + 1)
  /\ issued \subseteq 1..MaxOperation
  /\ pendingClient \in Clients \cup {"none"}
  /\ connected \subseteq Clients
  /\ response \in [Clients -> {"none", "accepted", "unknown", "unavailable"}]
  /\ published \in [Clients -> {"none", "accepted", "unknown", "unavailable"}]
  /\ committedOperation \in [Clients -> 0..MaxOperation]
  /\ responseOperation \in [Clients -> 0..MaxOperation]
  /\ fence \in 0..MaxOperation
FreshIssuance == [][(\E client \in Clients: IssueCreate(client) \/ IssueStart(client)) =>
  /\ operation' \notin issued
  /\ operation' = nextOperation
  /\ fence' > fence]_vars
ExactCommitIsCurrent == [][CompleteExact => bindingGeneration = workerGeneration]_vars
UnknownCannotDispatch == [][phase = "unknown" =>
  /\ phase' = "unknown"
  /\ issued' = issued]_vars
DisconnectPreservesTerminal == [][(\E client \in Clients: Disconnect(client)) =>
  /\ phase' = phase
  /\ issued' = issued
  /\ committedOperation' = committedOperation]_vars
ReplacementCannotRetarget == [][ReplaceWorker =>
  /\ bindingGeneration' = bindingGeneration
  /\ issued' = issued]_vars
PublicationFollowsCommit == \A client \in Clients:
  response[client] = "accepted" => committedOperation[client] = responseOperation[client]
PendingExcludesResult == Pending => response[pendingClient] = "none"
PendingEventuallySettles == Pending ~> ~Pending
ResultEventuallySettles == \A client \in Clients: (response[client] # "none") ~> (response[client] = "none")
=============================================================================
