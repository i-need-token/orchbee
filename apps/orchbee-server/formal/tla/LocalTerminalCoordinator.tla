---------------------- MODULE LocalTerminalCoordinator ----------------------
EXTENDS Naturals, FiniteSets, TLC

\* The coordinator refinement covers one binding slot and two peer Connections. A Close that ends
\* the slot's terminal retires the binding, and a later create fills the slot with a fresh terminal.
\* Wire delivery and Worker/Host admission have implementation tests, not models.
\* This model checks the Server reservation/commit boundary and binding retirement;
\* geometry/capacity/codec details have deterministic runtime witnesses.
\* Each Client is one sequential OBCS request/publication slot, so its index is
\* the bounded correlation surrogate; exact wire request IDs remain runtime evidence.
\* CloseTerminals admits Closes. Retirement reads no Worker generation, so a configuration that
\* replaces Workers may leave Closes to one that refills the slot.
CONSTANTS Clients, MaxOperation, MaxGeneration, MaxTerminal, CloseTerminals
ResponseValues == {"none", "accepted", "unknown", "unavailable", "server-busy"}
TerminalIds == 1..MaxTerminal
\* The Worker's record of one terminal: none made, live (counted against its bound), Disposed
\* (kept for exact Close replay, not counted), or forgotten after the Server retired it.
WorkerStates == {"none", "live", "disposed", "forgotten"}
CloseStates == {"none", "open", "disposing", "published"}
VARIABLES phase, workerGeneration, bindingGeneration, operation, nextOperation,
  fence, issued, pendingClient, connected, response, published, committedOperation, responseOperation,
  operationGeneration,
  terminal, nextTerminal, closeState, workerState, retired, retireQueue
coordinatorVars == <<phase, workerGeneration, bindingGeneration, operation, nextOperation,
  fence, issued, pendingClient, connected, response, published, committedOperation, responseOperation,
  operationGeneration>>
retirementVars == <<terminal, nextTerminal, closeState, workerState, retired, retireQueue>>
vars == <<coordinatorVars, retirementVars>>
Pending == phase \in {"creating", "starting"}
\* A settled binding whose terminal a Close may dispose.
Settled == phase \in {"awaiting", "started"}
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
  /\ operationGeneration = 0
  /\ terminal = 0
  /\ nextTerminal = 1
  /\ closeState = "none"
  /\ workerState = [id \in TerminalIds |-> "none"]
  /\ retired = {}
  /\ retireQueue = {}

IssueCreate(client) ==
  /\ client \in connected
  /\ phase = "absent"
  /\ response[client] = "none"
  /\ nextOperation <= MaxOperation
  /\ nextTerminal <= MaxTerminal
  /\ phase' = "creating"
  /\ bindingGeneration' = workerGeneration
  /\ operation' = nextOperation
  /\ nextOperation' = nextOperation + 1
  /\ issued' = issued \cup {nextOperation}
  /\ fence' = 1
  /\ pendingClient' = client
  /\ operationGeneration' = workerGeneration
  /\ terminal' = nextTerminal
  /\ nextTerminal' = nextTerminal + 1
  /\ closeState' = "open"
  /\ UNCHANGED <<workerGeneration, connected, response, published, committedOperation, responseOperation,
       workerState, retired, retireQueue>>
\* An accepted Close fences every later start of its terminal.
IssueStart(client) ==
  /\ client \in connected
  /\ phase = "awaiting"
  /\ closeState = "open"
  /\ bindingGeneration = workerGeneration
  /\ response[client] = "none"
  /\ nextOperation <= MaxOperation
  /\ phase' = "starting"
  /\ operation' = nextOperation
  /\ nextOperation' = nextOperation + 1
  /\ issued' = issued \cup {nextOperation}
  /\ fence' = fence + 1
  /\ pendingClient' = client
  /\ operationGeneration' = workerGeneration
  /\ UNCHANGED <<bindingGeneration, workerGeneration, connected, response, published, committedOperation, responseOperation>>
  /\ UNCHANGED retirementVars
RejectConcurrent(client) ==
  /\ client \in connected
  /\ client # pendingClient
  /\ Pending
  /\ response[client] = "none"
  /\ response' = [response EXCEPT ![client] = "server-busy"]
  /\ responseOperation' = [responseOperation EXCEPT ![client] = 0]
  /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
       nextOperation, fence, issued, pendingClient, connected, published,
       committedOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
CompleteExact ==
  /\ Pending
  /\ workerGeneration = bindingGeneration
  /\ phase' = IF phase = "creating" THEN "awaiting" ELSE "started"
  /\ response' = [response EXCEPT ![pendingClient] = "accepted"]
  /\ committedOperation' = [committedOperation EXCEPT ![pendingClient] = operation]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ workerState' = IF phase = "creating" THEN [workerState EXCEPT ![terminal] = "live"] ELSE workerState
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, operationGeneration>>
  /\ UNCHANGED <<terminal, nextTerminal, closeState, retired, retireQueue>>
\* An unknown create may or may not have made its terminal; the binding is never retired.
CompleteUnknown ==
  /\ Pending
  /\ phase' = "unknown"
  /\ response' = [response EXCEPT ![pendingClient] = "unknown"]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ \E made \in {"none", "live"}:
       workerState' = IF phase = "creating" THEN [workerState EXCEPT ![terminal] = made] ELSE workerState
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, committedOperation, operationGeneration>>
  /\ UNCHANGED <<terminal, nextTerminal, closeState, retired, retireQueue>>
CompleteNotDelivered ==
  /\ Pending
  /\ phase' = IF phase = "creating" THEN "unavailable" ELSE "awaiting"
  /\ response' = [response EXCEPT ![pendingClient] = "unavailable"]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, committedOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
CompleteRejected ==
  /\ Pending
  /\ phase' = "unavailable"
  /\ response' = [response EXCEPT ![pendingClient] = "unavailable"]
  /\ responseOperation' = [responseOperation EXCEPT ![pendingClient] = operation]
  /\ UNCHANGED <<workerGeneration, bindingGeneration, operation, nextOperation,
       fence, issued, pendingClient, connected, published, committedOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
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
       nextOperation, fence, issued, connected, pendingClient, committedOperation, responseOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
AbandonPublication(client) ==
  /\ response[client] # "none"
  /\ client \notin connected
  /\ response' = [response EXCEPT ![client] = "none"]
  /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
       nextOperation, fence, issued, connected, pendingClient, published, committedOperation, responseOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
Disconnect(client) ==
  /\ client \in connected
  /\ connected' = connected \ {client}
  /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
       nextOperation, fence, issued, pendingClient, response, published, committedOperation, responseOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
\* The Worker's records stay as they were: an adopting replacement keeps them, and keeping them
\* also over-approximates a replacement that lost them.
ReplaceWorker ==
  /\ workerGeneration < MaxGeneration
  /\ workerGeneration' = workerGeneration + 1
  /\ UNCHANGED <<phase, bindingGeneration, operation, nextOperation, fence,
       issued, pendingClient, connected, response, published, committedOperation, responseOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
\* A replacement that adopted the Host imports each settled binding into its generation, keeping
\* its fence and issued operations. A binding with an operation in flight at the loss is not
\* imported: that operation settles as unknown against the generation it was issued to.
ImportBinding ==
  /\ phase \in {"awaiting", "started"}
  /\ bindingGeneration < workerGeneration
  /\ bindingGeneration' = workerGeneration
  /\ UNCHANGED <<phase, workerGeneration, operation, nextOperation, fence, issued, pendingClient,
       connected, response, published, committedOperation, responseOperation, operationGeneration>>
  /\ UNCHANGED retirementVars
\* A Close disposes the settled terminal. The Worker stops counting it at once, before the Server
\* can learn the outcome, so the Worker's count never exceeds the Server's.
DisposeTerminal ==
  /\ CloseTerminals
  /\ Settled
  /\ closeState = "open"
  /\ workerState[terminal] = "live"
  /\ closeState' = "disposing"
  /\ workerState' = [workerState EXCEPT ![terminal] = "disposed"]
  /\ UNCHANGED coordinatorVars
  /\ UNCHANGED <<terminal, nextTerminal, retired, retireQueue>>
\* The Server learns Disposed and publishes the Close. A create whose result said it made no
\* terminal publishes its Close without one.
PublishClose ==
  /\ CloseTerminals
  /\ \/ closeState = "disposing"
     \/ /\ closeState = "open"
        /\ phase = "unavailable"
        /\ workerState[terminal] = "none"
  /\ closeState' = "published"
  /\ UNCHANGED coordinatorVars
  /\ UNCHANGED <<terminal, nextTerminal, workerState, retired, retireQueue>>
\* Once its Close is published and no Presentation names it, the Server retires the binding: the
\* slot admits a fresh create, the terminal ID is never issued again, and the Worker that holds a
\* Disposed record is told to forget it.
RetireBinding ==
  /\ closeState = "published"
  /\ phase \in {"awaiting", "started", "unavailable"}
  /\ phase' = "absent"
  /\ bindingGeneration' = 0
  /\ fence' = 0
  /\ terminal' = 0
  /\ closeState' = "none"
  /\ retired' = retired \cup {terminal}
  /\ retireQueue' = IF workerState[terminal] = "disposed" THEN retireQueue \cup {terminal} ELSE retireQueue
  /\ UNCHANGED <<workerGeneration, operation, nextOperation, issued, pendingClient, connected,
       response, published, committedOperation, responseOperation, operationGeneration>>
  /\ UNCHANGED <<nextTerminal, workerState>>
\* The Worker forgets a retired terminal only from its Disposed record; any other record stays.
ForgetRetired(id) ==
  /\ id \in retireQueue
  /\ retireQueue' = retireQueue \ {id}
  /\ workerState' = IF workerState[id] = "disposed"
                    THEN [workerState EXCEPT ![id] = "forgotten"]
                    ELSE workerState
  /\ UNCHANGED coordinatorVars
  /\ UNCHANGED <<terminal, nextTerminal, closeState, retired>>
Next ==
  \/ \E client \in Clients: IssueCreate(client) \/ IssueStart(client)
       \/ RejectConcurrent(client) \/ Disconnect(client)
  \/ CompleteExact \/ CompleteUnknown \/ CompleteNotDelivered \/ CompleteRejected \/ CompleteStale
  \/ (\E client \in Clients: Publish(client) \/ AbandonPublication(client))
  \/ ReplaceWorker \/ ImportBinding
  \/ DisposeTerminal \/ PublishClose \/ RetireBinding
  \/ (\E id \in TerminalIds: ForgetRetired(id))
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
  /\ response \in [Clients -> ResponseValues]
  /\ published \in [Clients -> ResponseValues]
  /\ committedOperation \in [Clients -> 0..MaxOperation]
  /\ responseOperation \in [Clients -> 0..MaxOperation]
  /\ fence \in 0..MaxOperation
  /\ operationGeneration \in 0..MaxGeneration
  /\ terminal \in 0..MaxTerminal
  /\ nextTerminal \in 1..(MaxTerminal + 1)
  /\ closeState \in CloseStates
  /\ workerState \in [TerminalIds -> WorkerStates]
  /\ retired \subseteq TerminalIds
  /\ retireQueue \subseteq retired
FreshIssuance == [][(\E client \in Clients: IssueCreate(client) \/ IssueStart(client)) =>
  /\ operation' \notin issued
  /\ operation' = nextOperation
  /\ fence' > fence]_vars
ExactCommitIsCurrent == [][CompleteExact => bindingGeneration = workerGeneration]_vars
\* An operation commits exactly only against the Worker generation it was issued to.
ExactCommitWasIssuedToCurrentWorker == [][CompleteExact => operationGeneration = workerGeneration]_vars
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
ServerBusyHasNoWorkerOperation == \A client \in Clients:
  response[client] = "server-busy" => responseOperation[client] = 0
PendingExcludesResult == Pending => response[pendingClient] = "none"
\* A late message naming a retired terminal may arrive in any later state, so it reaches a live
\* terminal only if some state holds a retired ID live: retired IDs are never issued again and were
\* Disposed or never made before retirement.
RetiredIdNeverDeliveredToLive == \A id \in retired: workerState[id] # "live"
\* The binding slot never names a retired terminal.
RetiredIdNeverReissued == terminal \notin retired
\* The Worker counts only live terminals, and each one has the Server's binding, so a create the
\* Server admitted is never refused by the Worker's bound.
WorkerCountWithinServerCount ==
  Cardinality({id \in TerminalIds: workerState[id] = "live"}) <= IF phase = "absent" THEN 0 ELSE 1
\* The Worker forgets only a terminal the Server retired.
WorkerForgetsOnlyRetired == \A id \in TerminalIds: workerState[id] = "forgotten" => id \in retired
ConcurrentRequestReturnsServerBusy ==
  [][\A client \in Clients: RejectConcurrent(client) =>
    /\ response' = [response EXCEPT ![client] = "server-busy"]
    /\ responseOperation' = [responseOperation EXCEPT ![client] = 0]
    /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
         nextOperation, fence, issued, pendingClient, connected, published,
         committedOperation, operationGeneration>>]_vars
PublicationCopiesExactPredecessor ==
  [][\A client \in Clients: Publish(client) =>
    /\ published' = [published EXCEPT ![client] = response[client]]
    /\ response' = [response EXCEPT ![client] = "none"]
    /\ UNCHANGED <<phase, workerGeneration, bindingGeneration, operation,
         nextOperation, fence, issued, pendingClient, connected,
         committedOperation, responseOperation, operationGeneration>>]_vars
PendingEventuallySettles == Pending ~> ~Pending
ResultEventuallySettles == \A client \in Clients: (response[client] # "none") ~> (response[client] = "none")
=============================================================================
