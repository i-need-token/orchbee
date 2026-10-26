-------------------- MODULE SessionCrashRecovery --------------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS Clients, Nobody, MaxGeneration
Roles == {"server", "worker", "host"}
Generations == 1..MaxGeneration
NoRole == "none"
Parent(r) == IF r = "worker" THEN "server" ELSE "worker"
VARIABLES generation, lifecycle, registration, physical, pending, owner,
          marker, markerGeneration, unknown, obstructed, phase, action
vars == <<generation, lifecycle, registration, physical, pending, owner,
          marker, markerGeneration, unknown, obstructed, phase, action>>
NoChildrenStarted == \A r \in Roles: registration[r] = "not-started"

\* Physical processes survive record transitions and generation changes. "dead"
\* denotes the exact PID+creation identity gone, even if its PID is reused.
Init ==
  /\ generation = 1 /\ lifecycle = "active"
  /\ registration = [r \in Roles |-> "not-started"]
  /\ unknown \in BOOLEAN /\ obstructed \in BOOLEAN
  /\ physical = [g \in Generations |-> [r \in Roles |->
       "none"]]
  /\ marker = "absent"
  /\ markerGeneration = 1
  /\ pending = NoRole /\ owner = Nobody
  /\ phase = [c \in Clients |-> "idle"] /\ action = "Init"

Acquire(c) ==
  /\ phase[c] = "idle" /\ owner = Nobody
  /\ phase' = [phase EXCEPT ![c] = "held"] /\ owner' = c
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending,
                  marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Acquire"
CommitMarker ==
  /\ owner # Nobody /\ lifecycle = "active"
  /\ marker = "absent" \/ (marker = "crashed" /\ NoChildrenStarted)
  /\ marker' = "active" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending,
                  owner, unknown, obstructed, phase>>
  /\ action' = "CommitMarker"
ParentAlive(r) == IF r = "server" THEN owner # Nobody
                  ELSE physical[generation][Parent(r)] = "running"
ReserveChild(r) ==
  /\ lifecycle = "active" /\ marker = "active" /\ markerGeneration = generation
  /\ pending = NoRole /\ registration[r] = "not-started" /\ ParentAlive(r)
  /\ registration' = [registration EXCEPT ![r] = "spawning"] /\ pending' = r
  /\ UNCHANGED <<generation, lifecycle, physical, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "ReserveChild"
CreateSuspended ==
  /\ pending \in Roles /\ registration[pending] = "spawning"
  /\ physical[generation][pending] = "none" /\ ParentAlive(pending)
  /\ physical' = [physical EXCEPT ![generation][pending] = "suspended"]
  /\ UNCHANGED <<generation, lifecycle, registration, pending, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "CreateSuspended"
PublishIdentity ==
  /\ pending \in Roles /\ registration[pending] = "spawning"
  /\ physical[generation][pending] = "suspended" /\ ParentAlive(pending)
  /\ registration' = [registration EXCEPT ![pending] = "identified"]
  /\ UNCHANGED <<generation, lifecycle, physical, pending, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "PublishIdentity"
ResumeChild ==
  /\ pending \in Roles /\ registration[pending] = "identified"
  /\ physical[generation][pending] = "suspended" /\ ParentAlive(pending)
  /\ physical' = [physical EXCEPT ![generation][pending] = "running"]
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "ResumeChild"
CreationFails ==
  /\ pending \in Roles /\ physical[generation][pending] = "none"
  /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, physical, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "CreationFails"
CleanupChild ==
  /\ pending \in Roles /\ physical[generation][pending] \in {"suspended", "dead"}
  /\ physical' = [physical EXCEPT ![generation][pending] = "dead"] /\ pending' = NoRole
  /\ UNCHANGED <<generation, lifecycle, registration, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "CleanupChild"
ProcessLoss(r) ==
  /\ physical[generation][r] \in {"suspended", "running"}
  /\ physical' = [physical EXCEPT ![generation][r] = "dead"]
  /\ pending' = IF pending \in Roles /\ pending # "server" /\ Parent(pending) = r
                 THEN NoRole ELSE pending
  /\ UNCHANGED <<generation, lifecycle, registration, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "ProcessLoss"
Proof ==
  /\ ~unknown /\ ~obstructed /\ pending = NoRole
  /\ \A r \in Roles: registration[r] # "spawning"
  /\ \A r \in Roles: physical[generation][r] \in {"none", "dead"}
Recover(c) ==
  /\ owner = c /\ phase[c] = "held" /\ lifecycle = "active" /\ Proof
  /\ lifecycle' = "crashed"
  /\ UNCHANGED <<generation, registration, physical, pending, owner, marker, markerGeneration,
                  unknown, obstructed, phase>>
  /\ action' = "Recover"
ClearPreviousMarker ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ Proof /\ NoChildrenStarted
  /\ marker = "crashed" /\ markerGeneration < generation
  /\ marker' = "active" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending, owner,
                  unknown, obstructed, phase>>
  /\ action' = "ClearPreviousMarker"
PublishCrashMarker ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ marker # "crashed" /\ Proof
  /\ marker' = "crashed" /\ markerGeneration' = generation
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending, owner,
                  unknown, obstructed, phase>>
  /\ action' = "PublishCrashMarker"
BeginGeneration ==
  /\ owner # Nobody /\ lifecycle = "crashed" /\ marker = "crashed" /\ Proof
  /\ markerGeneration = generation
  /\ generation < MaxGeneration
  \* Catalog publication and marker consumption are independently interrupted.
  /\ generation' = generation + 1 /\ lifecycle' = "active"
  /\ registration' = [r \in Roles |-> "not-started"]
  /\ UNCHANGED <<physical, pending, owner, marker, markerGeneration, unknown, obstructed, phase>>
  /\ action' = "BeginGeneration"
Refuse(c) ==
  /\ owner = c /\ phase[c] = "held" /\ ~Proof
  /\ pending = NoRole
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending, marker, markerGeneration,
                  unknown, obstructed>>
  /\ action' = "Refuse"
Ready(c) ==
  /\ owner = c /\ phase[c] = "held" /\ lifecycle = "active"
  /\ physical[generation]["server"] = "running"
  /\ physical[generation]["worker"] = "running"
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending, marker, markerGeneration,
                  unknown, obstructed>>
  /\ action' = "Ready"
Abort(c) ==
  /\ owner = c /\ phase[c] = "held"
  /\ owner' = Nobody /\ phase' = [phase EXCEPT ![c] = "finished"]
  /\ pending' = IF pending = "server" THEN NoRole ELSE pending
  /\ UNCHANGED <<generation, lifecycle, registration, physical, marker, markerGeneration,
                  unknown, obstructed>>
  /\ action' = "Abort"
Retry(c) ==
  /\ phase[c] = "finished"
  /\ phase' = [phase EXCEPT ![c] = "idle"]
  /\ UNCHANGED <<generation, lifecycle, registration, physical, pending, owner,
                  marker, markerGeneration, unknown, obstructed>>
  /\ action' = "Retry"
Next ==
  \/ \E c \in Clients: Acquire(c) \/ Recover(c) \/ Refuse(c) \/ Ready(c) \/ Abort(c) \/ Retry(c)
  \/ \E r \in Roles: ReserveChild(r) \/ ProcessLoss(r)
  \/ CommitMarker \/ CreateSuspended \/ PublishIdentity \/ ResumeChild \/ CreationFails
  \/ CleanupChild \/ ClearPreviousMarker \/ PublishCrashMarker \/ BeginGeneration
Spec == Init /\ [][Next]_vars /\ \A c \in Clients: WF_vars(Abort(c))
TypeOK ==
  /\ generation \in Generations /\ lifecycle \in {"active", "crashed"}
  /\ registration \in [Roles -> {"not-started", "spawning", "identified"}]
  /\ physical \in [Generations -> [Roles -> {"none", "suspended", "running", "dead"}]]
  /\ pending \in Roles \cup {NoRole} /\ owner \in Clients \cup {Nobody}
  /\ marker \in {"absent", "active", "crashed"}
  /\ markerGeneration \in 1..generation
  /\ unknown \in BOOLEAN /\ obstructed \in BOOLEAN
  /\ phase \in [Clients -> {"idle", "held", "finished"}]
SingleOwner == \A r \in Roles:
  Cardinality({g \in Generations: physical[g][r] \in {"running", "suspended"}}) <= 1
IdentityBeforeExecution == \A g \in Generations, r \in Roles:
  physical[g][r] = "running" => g = generation /\ registration[r] = "identified"
MarkerBeforeCreation == \A r \in Roles:
  physical[generation][r] \in {"suspended", "running"}
    => marker = "active" /\ markerGeneration = generation
CrashSafety == lifecycle = "crashed" => Proof
ActivationOwnership == owner # Nobody => phase[owner] = "held"
AttemptTerminates == \A c \in Clients: phase[c] = "held" ~> phase[c] = "finished"
=============================================================================
