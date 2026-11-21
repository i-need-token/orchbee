-------------------- MODULE SessionCrashRecovery --------------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS Clients, Nobody, MaxGeneration, MaxIncarnation, Handover
Roles == {"server", "worker", "host"}
Generations == 1..MaxGeneration
Incarnations == 1..MaxIncarnation
NoRole == "none"
Live == {"suspended", "running"}
Parent(r) == IF r = "worker" THEN "server" ELSE "worker"
VARIABLES generation, lifecycle, registration, physical, workers, incarnation, stop, pending,
          owner, marker, markerGeneration, unknown, obstructed, phase, action,
          successor, heir, oldGeneration, oldMode, exited
\* The restart's record facts (variant handover): the Session record's successor entry, the
\* successor Server it names, the generation of the Server that may hand over and whether that
\* Server released the Session to the successor, and whether the generation's `exit` is recorded.
handoverVars == <<successor, heir, oldGeneration, oldMode, exited>>
vars == <<generation, lifecycle, registration, physical, workers, incarnation, stop, pending,
          owner, marker, markerGeneration, unknown, obstructed, phase, action, handoverVars>>
\* The recorded Server holds the Session for the successor it released it to: it starts nothing.
Holding == Handover /\ oldMode = "released" /\ oldGeneration = generation
NoChildrenStarted == \A r \in Roles: registration[r] = "not-started"

\* Physical processes survive record transitions and generation changes. "dead"
\* denotes the exact PID+creation identity gone, even if its PID is reused. "orphan" is a
\* suspended child whose creation attempt ended before its identity was written: only its
\* creator held the handle that resumes it, so it never runs. Only the Worker has
\* incarnations: the recorded Server replaces a lost Worker inside one generation, and every
\* earlier incarnation stays a process of its own.
\* `pending` is the role whose creation attempt holds the record writer lock, from before its
\* Spawning write until its identity write and resume, or its failure cleanup, end the attempt;
\* the creator's loss ends it too. Cold recovery reads the record under that lock, so it only
\* ever reads `pending = NoRole`.
State(r) == IF r = "worker" THEN workers[generation][incarnation]
            ELSE physical[generation][r]
SetState(r, s) ==
  IF r = "worker"
  THEN /\ workers' = [workers EXCEPT ![generation][incarnation] = s] /\ UNCHANGED physical
  ELSE /\ physical' = [physical EXCEPT ![generation][r] = s] /\ UNCHANGED workers
Proofs == IF Handover THEN {FALSE} ELSE BOOLEAN
\* The handover variant runs one activator: the restart's record decisions race the processes, and
\* the recovery variant covers racing activators.
Activators == IF Handover THEN {CHOOSE c \in Clients: TRUE} ELSE Clients
Init ==
  /\ generation = 1 /\ lifecycle = "active"
  /\ registration = [r \in Roles |-> "not-started"]
  \* The handover variant leaves a recovery that cannot be proven to the recovery variant.
  /\ unknown \in Proofs /\ obstructed \in Proofs
  /\ physical = [g \in Generations |-> [r \in Roles |-> "none"]]
  /\ workers = [g \in Generations |-> [i \in Incarnations |-> "none"]]
  /\ incarnation = 1 /\ stop = "none"
  /\ marker = "absent"
  /\ markerGeneration = 1
  /\ pending = NoRole /\ owner = Nobody
  /\ phase = [c \in Clients |-> "idle"] /\ action = "Init"
  /\ successor = "none" /\ heir = "none" /\ oldGeneration = 1 /\ oldMode = "serving"
  /\ exited = FALSE

Acquire(c) ==
  /\ phase[c] = "idle" /\ owner = Nobody
  /\ phase' = [phase EXCEPT ![c] = "held"] /\ owner' = c
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, marker, markerGeneration, unknown, obstructed>>
  /\ UNCHANGED handoverVars
  /\ action' = "Acquire"
CommitMarker ==
  /\ owner # Nobody /\ lifecycle = "active"
  /\ marker = "absent" \/ (marker = "crashed" /\ NoChildrenStarted)
  /\ marker' = "active" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "CommitMarker"
ParentAlive(r) == IF r = "server" THEN owner # Nobody
                  ELSE State(Parent(r)) = "running"
ReserveChild(r) ==
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ pending = NoRole /\ registration[r] = "not-started" /\ ParentAlive(r) /\ ~exited
  /\ ~Holding
  /\ registration' = [registration EXCEPT ![r] = "spawning"] /\ pending' = r
  /\ UNCHANGED <<generation, lifecycle, physical, workers, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "ReserveChild"
\* The recorded Server returns the Worker slot to spawning once its own handle proved the
\* current incarnation gone, or a failed replacement left no process. The Host keeps its
\* registration, and a Host still spawning blocks it. No replacement starts once a stop is
\* admitted.
ReplaceWorker ==
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ stop = "none" /\ pending = NoRole /\ ~exited /\ ~Holding
  /\ physical[generation]["server"] = "running"
  /\ registration["worker"] # "not-started" /\ registration["host"] # "spawning"
  /\ workers[generation][incarnation] \in {"none", "dead"}
  /\ incarnation < MaxIncarnation
  /\ registration' = [registration EXCEPT !["worker"] = "spawning"]
  /\ incarnation' = incarnation + 1 /\ pending' = "worker"
  /\ UNCHANGED <<generation, lifecycle, physical, workers, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "ReplaceWorker"
CreateSuspended ==
  /\ pending \in Roles /\ registration[pending] = "spawning"
  /\ State(pending) = "none" /\ ParentAlive(pending)
  /\ SetState(pending, "suspended")
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, pending, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "CreateSuspended"
PublishIdentity ==
  /\ pending \in Roles /\ registration[pending] = "spawning"
  /\ State(pending) = "suspended" /\ ParentAlive(pending)
  /\ registration' = [registration EXCEPT ![pending] = "identified"]
  /\ UNCHANGED <<generation, lifecycle, physical, workers, incarnation, stop, pending, owner,
                  marker, markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "PublishIdentity"
ResumeChild ==
  /\ pending \in Roles /\ registration[pending] = "identified"
  /\ State(pending) = "suspended" /\ ParentAlive(pending)
  /\ SetState(pending, "running")
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "ResumeChild"
CreationFails ==
  /\ pending \in Roles /\ State(pending) = "none"
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "CreationFails"
CleanupChild ==
  /\ pending \in Roles /\ State(pending) \in {"suspended", "dead"}
  /\ SetState(pending, "dead") /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "CleanupChild"
Orphaned(c) == c \in Roles /\ pending = c /\ registration[c] = "spawning" /\ State(c) = "suspended"
\* A lost creator ends its child's attempt, and an unidentified suspended child becomes an orphan.
ProcessLoss(r) ==
  /\ State(r) \in Live
  /\ LET lost == pending \in Roles /\ pending # "server" /\ Parent(pending) = r
         orphan == lost /\ Orphaned(pending)
     IN /\ IF r = "worker"
           THEN /\ workers' = [workers EXCEPT ![generation][incarnation] = "dead"]
                /\ physical' = IF orphan THEN [physical EXCEPT ![generation][pending] = "orphan"]
                               ELSE physical
           ELSE /\ physical' = [physical EXCEPT ![generation][r] = "dead"]
                /\ workers' = IF orphan THEN [workers EXCEPT ![generation][incarnation] = "orphan"]
                              ELSE workers
        /\ pending' = IF lost THEN NoRole ELSE pending
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "ProcessLoss"
\* A creator whose cleanup is unconfirmed ends its attempt anyway: an unidentified child becomes an
\* orphan, and an identified one stays a live exact identity.
AbandonChild ==
  /\ pending \in Roles /\ State(pending) = "suspended"
  /\ IF registration[pending] = "spawning"
     THEN SetState(pending, "orphan")
     ELSE UNCHANGED <<physical, workers>>
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, incarnation, stop, owner, marker,
                  markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "AbandonChild"
\* A stop is admitted by the running Server; it then ends the Host, through its Worker or,
\* without one, through the Host's own exit request, and completes once nothing runs.
AdmitStop ==
  /\ lifecycle = "active" /\ stop = "none" /\ physical[generation]["server"] = "running"
  /\ stop' = "admitted"
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, pending,
                  owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "AdmitStop"
StopEndsHost ==
  /\ stop = "admitted" /\ lifecycle = "active" /\ physical[generation]["host"] = "running"
  /\ physical' = [physical EXCEPT ![generation]["host"] = "dead"]
  /\ UNCHANGED <<generation, lifecycle, registration, workers, incarnation, stop, pending,
                  owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
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
  /\ UNCHANGED handoverVars
  /\ action' = "CompleteStop"
\* Cold recovery's verdict, read under the record writer lock: no attempt in progress, every
\* identified role's exact process observed gone, and no observation unknown or obstructed. A
\* spawning slot names an attempt that ended without an identity, so it decides nothing.
Proof ==
  /\ ~unknown /\ ~obstructed /\ pending = NoRole
  /\ \A r \in {"server", "host"}: registration[r] = "identified" => physical[generation][r] = "dead"
  /\ registration["worker"] = "identified" => workers[generation][incarnation] = "dead"
Settled(s) == s \in {"none", "dead", "orphan"}
Recover(c) ==
  /\ owner = c /\ phase[c] = "held" /\ lifecycle = "active" /\ Proof
  /\ lifecycle' = "crashed"
  /\ UNCHANGED <<generation, registration, physical, workers, incarnation, stop, pending, owner,
                  marker, markerGeneration, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "Recover"
ClearPreviousMarker ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ Proof /\ NoChildrenStarted
  /\ marker = "crashed" /\ markerGeneration < generation
  /\ marker' = "active" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
  /\ action' = "ClearPreviousMarker"
PublishCrashMarker ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ marker # "crashed" /\ Proof
  /\ marker' = "crashed" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, unknown, obstructed, phase>>
  /\ UNCHANGED handoverVars
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
  /\ successor' = "none" /\ exited' = FALSE /\ UNCHANGED <<heir, oldGeneration, oldMode>>
  /\ action' = "BeginGeneration"
Refuse(c) ==
  /\ owner = c /\ phase[c] = "held" /\ ~Proof
  /\ pending = NoRole
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, marker, markerGeneration, unknown, obstructed>>
  /\ UNCHANGED handoverVars
  /\ action' = "Refuse"
Ready(c) ==
  /\ owner = c /\ phase[c] = "held" /\ lifecycle = "active"
  /\ physical[generation]["server"] = "running"
  /\ State("worker") = "running"
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, marker, markerGeneration, unknown, obstructed>>
  /\ UNCHANGED handoverVars
  /\ action' = "Ready"
Abort(c) ==
  /\ owner = c /\ phase[c] = "held"
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ pending' = IF pending = "server" THEN NoRole ELSE pending
  /\ physical' = IF Orphaned("server")
                 THEN [physical EXCEPT ![generation]["server"] = "orphan"] ELSE physical
  /\ UNCHANGED <<generation, lifecycle, registration, workers, incarnation, stop,
                  marker, markerGeneration, unknown, obstructed>>
  /\ UNCHANGED handoverVars
  /\ action' = "Abort"
Retry(c) ==
  /\ phase[c] = "finished"
  /\ phase' = [phase EXCEPT ![c] = "idle"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, marker, markerGeneration, unknown, obstructed>>
  /\ UNCHANGED handoverVars
  /\ action' = "Retry"
\* Variant handover: a restart that keeps terminals. The requester reserves a successor Server in
\* the record; the recorded Server releases the Session to it, ending its Worker and keeping the
\* Host; the successor commits by one record publish that names it, its Worker and the kept Host.
\* Until then the recorded Server may take the Session back. Each record action here is one step
\* under the record's writer lock.
ReserveSuccessor(c) ==
  /\ Handover /\ owner = c /\ phase[c] = "held"
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ stop = "none" /\ ~exited /\ pending = NoRole
  \* A restart is asked of a Session whose Server, Worker and Host run.
  /\ \A r \in Roles: registration[r] = "identified"
  /\ physical[generation]["server"] = "running" /\ physical[generation]["host"] = "running"
  /\ State("worker") = "running"
  /\ successor = "none" /\ heir = "none" /\ ~Holding
  /\ \A g \in Generations: g # generation => physical[g]["server"] \notin Live
  /\ generation < MaxGeneration
  /\ successor' = "reserved" /\ heir' = "running" /\ oldGeneration' = generation
  /\ oldMode' = "serving"
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, marker, markerGeneration, unknown, obstructed, phase, exited>>
  /\ action' = "ReserveSuccessor"
\* `prepare`: the recorded Server holds new intents and ends its Worker with the Keep shutdown,
\* which leaves the Host running; the successor learns `prepared`.
PrepareHandover ==
  /\ successor = "reserved" /\ heir = "running" /\ oldMode = "serving"
  /\ oldGeneration = generation /\ physical[generation]["server"] = "running"
  /\ lifecycle = "active" /\ stop = "none" /\ pending = NoRole
  /\ oldMode' = "released" /\ heir' = "prepared"
  /\ workers' = [workers EXCEPT ![generation][incarnation] = IF @ \in Live THEN "dead" ELSE @]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, incarnation, stop, pending,
                  owner, marker, markerGeneration, unknown, obstructed, phase, successor,
                  oldGeneration, exited>>
  /\ action' = "PrepareHandover"
\* The commit point: one publish names the successor, its Worker and the kept Host under a fresh
\* generation, only while the record still names this successor and the recorded Server.
CommitSuccessor ==
  /\ heir = "prepared"
  /\ successor = "reserved"
  /\ lifecycle = "active" /\ stop = "none" /\ generation = oldGeneration
  /\ generation < MaxGeneration
  /\ generation' = generation + 1
  /\ registration' = [registration EXCEPT !["server"] = "identified", !["worker"] = "identified"]
  /\ physical' = [physical EXCEPT ![generation + 1]["server"] = "running",
                                  ![generation + 1]["host"] = physical[generation]["host"],
                                  ![generation]["host"] = "none"]
  /\ workers' = [workers EXCEPT ![generation + 1][1] = "running"]
  /\ incarnation' = 1 /\ markerGeneration' = generation + 1
  /\ successor' = "none" /\ heir' = "none" /\ exited' = FALSE
  /\ UNCHANGED <<lifecycle, stop, pending, owner, marker, unknown, obstructed, phase,
                  oldGeneration, oldMode>>
  /\ action' = "CommitSuccessor"
\* The take-back after `abort`, the lease or the successor's end: the released Server removes the
\* successor entry and serves again, only while the record still names it.
ClearSuccessor ==
  /\ oldMode = "released" /\ physical[oldGeneration]["server"] = "running"
  /\ oldGeneration = generation
  /\ lifecycle = "active"
  /\ successor' = "none" /\ oldMode' = "serving"
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, marker, markerGeneration, unknown, obstructed, phase, heir,
                  oldGeneration, exited>>
  /\ action' = "ClearSuccessor"
\* A released Server the record no longer names serves nothing and exits.
Retired(g) == Handover /\ g = oldGeneration /\ g < generation /\ oldMode = "released"
RetiredExit ==
  /\ Retired(oldGeneration) /\ physical[oldGeneration]["server"] \in Live
  /\ physical' = [physical EXCEPT ![oldGeneration]["server"] = "dead"]
  /\ UNCHANGED <<generation, lifecycle, registration, workers, incarnation, stop, pending, owner,
                  marker, markerGeneration, unknown, obstructed, phase, handoverVars>>
  /\ action' = "RetiredExit"
\* Cold adoption: an activation whose verdict, taken under the record writer lock as recovery's
\* is, proves the recorded Server and Worker gone while the recorded Host runs starts a fresh
\* generation that keeps the Host registration. A spawning slot decides nothing, as in Proof.
ServerWorkerGone ==
  /\ ~unknown /\ ~obstructed /\ pending = NoRole
  /\ registration["server"] = "identified" => physical[generation]["server"] = "dead"
  /\ registration["worker"] = "identified" => workers[generation][incarnation] = "dead"
AdoptCold(c) ==
  /\ Handover /\ owner = c /\ phase[c] = "held"
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ ServerWorkerGone
  /\ physical[generation]["host"] = "running" /\ registration["host"] = "identified"
  /\ generation < MaxGeneration
  /\ generation' = generation + 1
  /\ registration' = [registration EXCEPT !["server"] = "not-started", !["worker"] = "not-started"]
  /\ physical' = [physical EXCEPT ![generation + 1]["host"] = "running",
                                  ![generation]["host"] = "none"]
  /\ incarnation' = 1 /\ stop' = "none" /\ markerGeneration' = generation + 1
  /\ successor' = "none" /\ exited' = FALSE
  /\ UNCHANGED <<lifecycle, workers, pending, owner, marker, unknown, obstructed, phase, heir,
                  oldGeneration, oldMode>>
  /\ action' = "AdoptCold"
\* `exit` records its end once its Worker is gone; the generation then starts nothing more, and the
\* next activation counts no crash for it.
ExitRecorded ==
  /\ Handover /\ lifecycle = "active" /\ stop = "none" /\ ~exited
  /\ physical[generation]["server"] = "running" /\ ~Holding /\ successor = "none"
  /\ pending = NoRole
  /\ \A i \in Incarnations: workers[generation][i] \notin Live
  /\ exited' = TRUE
  /\ UNCHANGED <<generation, lifecycle, registration, physical, workers, incarnation, stop,
                  pending, owner, marker, markerGeneration, unknown, obstructed, phase, successor,
                  heir, oldGeneration, oldMode>>
  /\ action' = "ExitRecorded"
Next ==
  \/ \E c \in Activators: Acquire(c) \/ Recover(c) \/ Refuse(c) \/ Ready(c) \/ Abort(c) \/ Retry(c)
  \/ \E r \in Roles: ReserveChild(r) \/ ProcessLoss(r)
  \/ CommitMarker \/ CreateSuspended \/ PublishIdentity \/ ResumeChild \/ CreationFails
  \/ CleanupChild \/ ClearPreviousMarker \/ PublishCrashMarker \/ BeginGeneration
  \/ ReplaceWorker \/ AdmitStop \/ StopEndsHost \/ CompleteStop \/ AbandonChild
  \/ \E c \in Activators: ReserveSuccessor(c) \/ AdoptCold(c)
  \/ PrepareHandover \/ CommitSuccessor \/ ClearSuccessor \/ RetiredExit
  \/ ExitRecorded
Spec == Init /\ [][Next]_vars /\ \A c \in Clients: WF_vars(Abort(c))
ProcessStates == {"none", "suspended", "running", "dead", "orphan"}
TypeOK ==
  /\ generation \in Generations /\ lifecycle \in {"active", "crashed", "stopped"}
  /\ registration \in [Roles -> {"not-started", "spawning", "identified"}]
  /\ physical \in [Generations -> [Roles -> ProcessStates]]
  /\ workers \in [Generations -> [Incarnations -> ProcessStates]]
  /\ incarnation \in Incarnations /\ stop \in {"none", "admitted"}
  /\ pending \in Roles \cup {NoRole} /\ owner \in Clients \cup {Nobody}
  /\ marker \in {"absent", "active", "crashed"}
  /\ markerGeneration \in 1..generation
  /\ unknown \in BOOLEAN /\ obstructed \in BOOLEAN
  /\ phase \in [Clients -> {"idle", "held", "finished"}]
  /\ successor \in {"none", "reserved"} /\ heir \in {"none", "running", "prepared"}
  /\ oldGeneration \in Generations /\ oldMode \in {"serving", "released"}
  /\ exited \in BOOLEAN
\* Each role has at most one live process across generations and, for the Worker, incarnations;
\* a released Server the record no longer names lingers until it exits.
SingleOwner ==
  /\ Cardinality({g \in Generations: physical[g]["server"] \in Live /\ ~Retired(g)}) <= 1
  /\ Cardinality({g \in Generations: physical[g]["host"] \in Live}) <= 1
  /\ Cardinality({w \in Generations \X Incarnations: workers[w[1]][w[2]] \in Live}) <= 1
OnlyCurrentWorkerRuns == \A g \in Generations, i \in Incarnations:
  workers[g][i] \in Live => g = generation /\ i = incarnation
IdentityBeforeExecution ==
  /\ \A g \in Generations, r \in {"server", "host"}:
       physical[g][r] = "running" /\ ~(r = "server" /\ Retired(g)) =>
         g = generation /\ registration[r] = "identified"
  /\ \A g \in Generations, i \in Incarnations:
       workers[g][i] = "running" => g = generation /\ registration["worker"] = "identified"
MarkerBeforeCreation == \A r \in Roles:
  State(r) \in Live => marker = "active" /\ markerGeneration = generation
CrashSafety == lifecycle = "crashed" => Proof
StoppedRunsNothing == lifecycle = "stopped" =>
  /\ \A i \in Incarnations: workers[generation][i] # "running"
  /\ physical[generation]["host"] # "running"
ActivationOwnership == owner # Nobody => phase[owner] = "held"
\* A Server that serves the Session is the one the record names: never a Server that released it
\* to a successor that then committed, and never one beside a committed successor.
ServingIsRecorded ==
  oldMode = "serving" /\ physical[oldGeneration]["server"] \in Live => oldGeneration = generation
\* A recorded exit never hides a running Worker from the next activation.
ExitedRunsNoWorker == exited => \A i \in Incarnations: workers[generation][i] \notin Live
ReplacementKeepsHost ==
  [][action' = "ReplaceWorker" => registration'["host"] = registration["host"]]_vars
AttemptTerminates == \A c \in Clients: phase[c] = "held" ~> phase[c] = "finished"
\* The verdict never admits a generation that may still run a process: each process it does not
\* read is never started, gone, or an orphan.
ProofIsSound == Proof =>
  /\ \A r \in {"server", "host"}: Settled(physical[generation][r])
  /\ \A i \in Incarnations: Settled(workers[generation][i])
\* No registration refuses recovery for good: once no process of the generation may run, no attempt
\* is in progress and every observation is certain, the verdict admits recovery.
RecoveryNeverStuck ==
  (/\ lifecycle = "active" /\ pending = NoRole /\ ~unknown /\ ~obstructed
   /\ \A r \in {"server", "host"}: Settled(physical[generation][r])
   /\ \A i \in Incarnations: Settled(workers[generation][i])) => Proof
OrphansNeverRun == [][\A g \in Generations, r \in {"server", "host"}, i \in Incarnations:
  /\ physical[g][r] = "orphan" => physical'[g][r] = "orphan"
  /\ workers[g][i] = "orphan" => workers'[g][i] = "orphan"]_vars
=============================================================================
