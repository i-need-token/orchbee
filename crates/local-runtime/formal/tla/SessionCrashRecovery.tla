-------------------- MODULE SessionCrashRecovery --------------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS Clients, Nobody, MaxGeneration, MaxIncarnation
Roles == {"server", "worker", "host"}
Generations == 1..MaxGeneration
Incarnations == 1..MaxIncarnation
NoRole == "none"
Live == {"suspended", "running"}
Parent(r) == IF r = "worker" THEN "server" ELSE "worker"
VARIABLES generation, lifecycle, registration, physical, workers, incarnation, stop, pending,
          owner, marker, markerGeneration, unknown, obstructed, phase, action
vars == <<generation, lifecycle, registration, physical, workers, incarnation, stop, pending,
          owner, marker, markerGeneration, unknown, obstructed, phase, action>>
NoChildrenStarted == \A r \in Roles: registration[r] = "not-started"

\* Physical processes survive record transitions and generation changes. "dead"
\* denotes the exact PID+creation identity gone, even if its PID is reused. Only the
\* Worker has incarnations: the recorded Server replaces a lost Worker inside one
\* generation, and every earlier incarnation stays a process of its own.
State(r) == IF r = "worker" THEN workers[generation][incarnation]
            ELSE physical[generation][r]
SetState(r, s) ==
  IF r = "worker"
  THEN /\ workers' = [workers EXCEPT ![generation][incarnation] = s] /\ UNCHANGED physical
  ELSE /\ physical' = [physical EXCEPT ![generation][r] = s] /\ UNCHANGED workers
Init ==
  /\ generation = 1 /\ lifecycle = "active"
  /\ registration = [r \in Roles |-> "not-started"]
  /\ unknown \in BOOLEAN /\ obstructed \in BOOLEAN
  /\ physical = [g \in Generations |-> [r \in Roles |-> "none"]]
  /\ workers = [g \in Generations |-> [i \in Incarnations |-> "none"]]
  /\ incarnation = 1 /\ stop = "none"
  /\ marker = "absent"
  /\ markerGeneration = 1
  /\ pending = NoRole /\ owner = Nobody
  /\ phase = [c \in Clients |-> "idle"] /\ action = "Init"

Acquire(c) ==
  /\ phase[c] = "idle" /\ owner = Nobody
  /\ phase' = [phase EXCEPT ![c] = "held"] /\ owner' = c
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Acquire"
CommitMarker ==
  /\ owner # Nobody /\ lifecycle = "active"
  /\ marker = "absent" \/ (marker = "crashed" /\ NoChildrenStarted)
  /\ marker' = "active" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, unknown, obstructed, phase>>
  /\ action' = "CommitMarker"
ParentAlive(r) == IF r = "server" THEN owner # Nobody
                  ELSE State(Parent(r)) = "running"
ReserveChild(r) ==
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ pending = NoRole /\ registration[r] = "not-started" /\ ParentAlive(r)
  /\ registration' = [registration EXCEPT ![r] = "spawning"] /\ pending' = r
  /\ UNCHANGED <<generation, lifecycle, physical, workers, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "ReserveChild"
\* The recorded Server returns the Worker slot to spawning once its own handle proved the
\* current incarnation gone, or a failed replacement left no process. The Host keeps its
\* registration, and a Host still spawning blocks it. No replacement starts once a stop is
\* admitted.
ReplaceWorker ==
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ stop = "none" /\ pending = NoRole
  /\ physical[generation]["server"] = "running"
  /\ registration["worker"] # "not-started" /\ registration["host"] # "spawning"
  /\ workers[generation][incarnation] \in {"none", "dead"}
  /\ incarnation < MaxIncarnation
  /\ registration' = [registration EXCEPT !["worker"] = "spawning"]
  /\ incarnation' = incarnation + 1 /\ pending' = "worker"
  /\ UNCHANGED <<generation, lifecycle, physical, workers, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "ReplaceWorker"
CreateSuspended ==
  /\ pending \in Roles /\ registration[pending] = "spawning"
  /\ State(pending) = "none" /\ ParentAlive(pending)
  /\ SetState(pending, "suspended")
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, pending, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "CreateSuspended"
PublishIdentity ==
  /\ pending \in Roles /\ registration[pending] = "spawning"
  /\ State(pending) = "suspended" /\ ParentAlive(pending)
  /\ registration' = [registration EXCEPT ![pending] = "identified"]
  /\ UNCHANGED <<generation, lifecycle, physical, workers, incarnation, stop, pending, owner,
                  marker, markerGeneration, unknown, obstructed, phase>>
  /\ action' = "PublishIdentity"
ResumeChild ==
  /\ pending \in Roles /\ registration[pending] = "identified"
  /\ State(pending) = "suspended" /\ ParentAlive(pending)
  /\ SetState(pending, "running")
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "ResumeChild"
CreationFails ==
  /\ pending \in Roles /\ State(pending) = "none"
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ action' = "CreationFails"
CleanupChild ==
  /\ pending \in Roles /\ State(pending) \in {"suspended", "dead"}
  /\ SetState(pending, "dead") /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "CleanupChild"
ProcessLoss(r) ==
  /\ State(r) \in Live
  /\ SetState(r, "dead")
  /\ pending' = IF pending \in Roles /\ pending # "server" /\ Parent(pending) = r
                 THEN NoRole ELSE pending
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "ProcessLoss"
\* A stop is admitted by the running Server; it then ends the Host, through its Worker or,
\* without one, through the Host's own exit request, and completes once nothing runs.
AdmitStop ==
  /\ lifecycle = "active" /\ stop = "none" /\ physical[generation]["server"] = "running"
  /\ stop' = "admitted"
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, pending,
                  owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ action' = "AdmitStop"
StopEndsHost ==
  /\ stop = "admitted" /\ lifecycle = "active" /\ physical[generation]["host"] = "running"
  /\ physical' = [physical EXCEPT ![generation]["host"] = "dead"]
  /\ UNCHANGED <<generation, lifecycle, registration, workers, incarnation, stop, pending,
                  owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ action' = "StopEndsHost"
CompleteStop ==
  /\ stop = "admitted" /\ lifecycle = "active" /\ pending = NoRole
  /\ physical[generation]["server"] = "running"
  /\ \A i \in Incarnations: workers[generation][i] # "running"
  /\ physical[generation]["host"] # "running"
  /\ lifecycle' = "stopped"
  /\ physical' = [physical EXCEPT ![generation]["server"] = "dead"]
  /\ UNCHANGED <<generation, registration, workers, incarnation, stop, pending, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ action' = "CompleteStop"
Proof ==
  /\ ~unknown /\ ~obstructed /\ pending = NoRole
  /\ \A r \in Roles: registration[r] # "spawning"
  /\ \A r \in {"server", "host"}: physical[generation][r] \in {"none", "dead"}
  /\ \A i \in Incarnations: workers[generation][i] \in {"none", "dead"}
Recover(c) ==
  /\ owner = c /\ phase[c] = "held" /\ lifecycle = "active" /\ Proof
  /\ lifecycle' = "crashed"
  /\ UNCHANGED <<generation, registration, physical, workers, incarnation, stop, pending, owner,
                  marker, markerGeneration, unknown, obstructed, phase>>
  /\ action' = "Recover"
ClearPreviousMarker ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ Proof /\ NoChildrenStarted
  /\ marker = "crashed" /\ markerGeneration < generation
  /\ marker' = "active" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, unknown, obstructed, phase>>
  /\ action' = "ClearPreviousMarker"
PublishCrashMarker ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ marker # "crashed" /\ Proof
  /\ marker' = "crashed" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, unknown, obstructed, phase>>
  /\ action' = "PublishCrashMarker"
BeginGeneration ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ marker = "crashed" /\ Proof
  /\ markerGeneration = generation
  /\ generation < MaxGeneration
  \* Catalog publication and marker consumption are independently interrupted.
  /\ generation' = generation + 1 /\ lifecycle' = "active"
  /\ registration' = [r \in Roles |-> "not-started"]
  /\ incarnation' = 1 /\ stop' = "none"
  /\ UNCHANGED <<physical, workers, pending, owner, marker, markerGeneration, unknown,
                  obstructed, phase>>
  /\ action' = "BeginGeneration"
Refuse(c) ==
  /\ owner = c /\ phase[c] = "held" /\ ~Proof
  /\ pending = NoRole
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Refuse"
Ready(c) ==
  /\ owner = c /\ phase[c] = "held" /\ lifecycle = "active"
  /\ physical[generation]["server"] = "running"
  /\ State("worker") = "running"
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Ready"
Abort(c) ==
  /\ owner = c /\ phase[c] = "held"
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ pending' = IF pending = "server" THEN NoRole ELSE pending
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Abort"
Retry(c) ==
  /\ phase[c] = "finished"
  /\ phase' = [phase EXCEPT ![c] = "idle"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Retry"
Next ==
  \/ \E c \in Clients: Acquire(c) \/ Recover(c) \/ Refuse(c) \/ Ready(c) \/ Abort(c) \/ Retry(c)
  \/ \E r \in Roles: ReserveChild(r) \/ ProcessLoss(r)
  \/ CommitMarker \/ CreateSuspended \/ PublishIdentity \/ ResumeChild \/ CreationFails
  \/ CleanupChild \/ ClearPreviousMarker \/ PublishCrashMarker \/ BeginGeneration
  \/ ReplaceWorker \/ AdmitStop \/ StopEndsHost \/ CompleteStop
Spec == Init /\ [][Next]_vars /\ \A c \in Clients: WF_vars(Abort(c))
TypeOK ==
  /\ generation \in Generations /\ lifecycle \in {"active", "crashed", "stopped"}
  /\ registration \in [Roles -> {"not-started", "spawning", "identified"}]
  /\ physical \in [Generations -> [Roles -> {"none", "suspended", "running", "dead"}]]
  /\ workers \in [Generations -> [Incarnations -> {"none", "suspended", "running", "dead"}]]
  /\ incarnation \in Incarnations /\ stop \in {"none", "admitted"}
  /\ pending \in Roles \cup {NoRole} /\ owner \in Clients \cup {Nobody}
  /\ marker \in {"absent", "active", "crashed"}
  /\ markerGeneration \in 1..generation
  /\ unknown \in BOOLEAN /\ obstructed \in BOOLEAN
  /\ phase \in [Clients -> {"idle", "held", "finished"}]
\* Each role has at most one live process across generations and, for the Worker, incarnations.
SingleOwner ==
  /\ \A r \in {"server", "host"}: Cardinality({g \in Generations: physical[g][r] \in Live}) <= 1
  /\ Cardinality({w \in Generations \X Incarnations: workers[w[1]][w[2]] \in Live}) <= 1
OnlyCurrentWorkerRuns == \A g \in Generations, i \in Incarnations:
  workers[g][i] \in Live => g = generation /\ i = incarnation
IdentityBeforeExecution ==
  /\ \A g \in Generations, r \in {"server", "host"}:
       physical[g][r] = "running" => g = generation /\ registration[r] = "identified"
  /\ \A g \in Generations, i \in Incarnations:
       workers[g][i] = "running" => g = generation /\ registration["worker"] = "identified"
MarkerBeforeCreation == \A r \in Roles:
  State(r) \in Live => marker = "active" /\ markerGeneration = generation
CrashSafety == lifecycle = "crashed" => Proof
StoppedRunsNothing == lifecycle = "stopped" =>
  /\ \A i \in Incarnations: workers[generation][i] # "running"
  /\ physical[generation]["host"] # "running"
ActivationOwnership == owner # Nobody => phase[owner] = "held"
ReplacementKeepsHost ==
  [][action' = "ReplaceWorker" => registration'["host"] = registration["host"]]_vars
AttemptTerminates == \A c \in Clients: phase[c] = "held" ~> phase[c] = "finished"
=============================================================================
