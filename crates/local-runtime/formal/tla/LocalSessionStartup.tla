-------------------------- MODULE LocalSessionStartup --------------------------
EXTENDS Naturals, FiniteSets, TLC
\* MaxHostStarts stands for the lazy loop's HOST_START_ATTEMPTS, the Host starts one Worker makes;
\* the configuration checks two, a start that leaves a retry and a last one.
CONSTANTS Clients, Nobody, MaxHostStarts
ASSUME MaxHostStarts \in Nat \ {0}
\* endpoint is "stale" when an endpoint no live Host of this Worker serves is at the Host's address,
\* such as the socket a Host left when it ended.
\* workerStarts counts the Host starts of the serving Worker connection, hostAttempts the
\* generation's.
VARIABLES phase, guard, marker, launches, server, worker, authorized, hostAttempts,
          host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner,
          endpoint, workerStarts, action
vars == <<phase, guard, marker, launches, server, worker, authorized, hostAttempts,
          host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner,
          endpoint, workerStarts, action>>
Active == {"connecting", "locked", "committed", "waiting", "opening", "frame"}
Init ==
  /\ phase = [c \in Clients |-> "connecting"] /\ guard = Nobody
  /\ marker = FALSE /\ launches = 0 /\ server = "absent" /\ worker = FALSE
  /\ authorized = FALSE /\ hostAttempts = 0 /\ host = "absent" /\ action = "Init"
  /\ lifetime \in BOOLEAN /\ workerAttempts = 0 /\ dispatch = "none"
  /\ incarnation = 0 /\ dispatchWorker = 0 /\ hostOwner = 0
  /\ endpoint \in {"absent", "stale"} /\ workerStarts = 0
Acquire(c) ==
  /\ phase[c] = "connecting" /\ guard = Nobody
  /\ phase' = [phase EXCEPT ![c] = "locked"] /\ guard' = c
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Acquire"
Commit(c) ==
  /\ phase[c] = "locked" /\ ~marker /\ ~lifetime /\ server = "absent"
  /\ marker' = TRUE
  /\ phase' = [phase EXCEPT ![c] = "committed"]
  /\ UNCHANGED <<guard, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Commit"
Spawn(c) ==
  /\ phase[c] = "committed" /\ marker /\ launches = 0
  /\ launches' = 1 /\ server' = "starting"
  /\ phase' = [phase EXCEPT ![c] = "waiting"]
  /\ UNCHANGED <<guard, marker, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Spawn"
AcquireLifetime ==
  /\ server = "starting" /\ marker /\ ~lifetime
  /\ lifetime' = TRUE /\ server' = "live"
  /\ UNCHANGED <<phase, guard, marker, launches, worker, authorized, hostAttempts, host, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "AcquireLifetime"
Wait(c) ==
  /\ phase[c] = "locked" /\ marker /\ lifetime
  /\ phase' = [phase EXCEPT ![c] = "waiting"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Wait"
\* An attempt that launches nothing in this generation: with the marker and a free lifetime lock the
\* generation ended without its stop, and the catalog closes it once the Host lock is also free;
\* with no marker and a held lifetime lock, a Server is completing its stop. A later attempt then
\* starts the next generation, a fresh instance of this model.
Recovery(c) ==
  /\ phase[c] = "locked" /\ ((marker /\ ~lifetime) \/ (~marker /\ lifetime))
  /\ phase' = [phase EXCEPT ![c] = "failed"] /\ guard' = Nobody
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Recovery"
WorkerReady ==
  /\ server = "live" /\ lifetime /\ workerAttempts = 0 /\ worker' = TRUE /\ workerAttempts' = 1
  /\ incarnation' = 1
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, dispatch, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "WorkerReady"
Connect(c) ==
  /\ phase[c] \in {"connecting", "locked", "waiting"} /\ server = "live" /\ worker
  /\ phase' = [phase EXCEPT ![c] = "opening"]
  /\ guard' = IF guard = c THEN Nobody ELSE guard
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Connect"
\* A Launch that failed before handoff leaves the Worker serving, so a later Launch is authorized
\* again; a handed-off Launch settles the startup and later ones belong to the admitted loop.
Authorize ==
  /\ \E c \in Clients: phase[c] = "opening"
  /\ server = "live" /\ worker /\ dispatch \in {"none", "failed"}
  /\ authorized' = TRUE /\ dispatch' = "authorized" /\ dispatchWorker' = incarnation
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, hostAttempts, host, lifetime, workerAttempts, incarnation, hostOwner, endpoint, workerStarts>>
  /\ action' = "Authorize"
\* Each start answers one authorized Launch. A failed start leaves this Worker no Host, and only the
\* next authorized Launch starts one again, up to MaxHostStarts starts for each Worker; the model
\* bounds a generation at two Host starts. An authorization belongs to the Worker connection that
\* received it: a replacement Worker never starts a Host for, or hands off, a Launch authorized to
\* the Worker it replaced.
StartHost ==
  /\ dispatch = "authorized" /\ worker /\ host \in {"absent", "failed"}
  /\ workerStarts < MaxHostStarts /\ endpoint = "absent" /\ hostAttempts < 2
  /\ dispatchWorker = incarnation
  /\ hostAttempts' = hostAttempts + 1 /\ workerStarts' = workerStarts + 1
  /\ host' = "starting" /\ hostOwner' = incarnation
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, endpoint>>
  /\ action' = "StartHost"
HostReady ==
  /\ host = "starting" /\ worker
  /\ host' = "live"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, hostAttempts, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "HostReady"
Open(c) ==
  /\ phase[c] = "opening" /\ worker /\ host = "live" /\ dispatch = "settled"
  /\ phase' = [phase EXCEPT ![c] = "frame"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Open"
FirstFrame(c) ==
  /\ phase[c] = "frame" /\ worker /\ host = "live"
  /\ phase' = [phase EXCEPT ![c] = "ready"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "FirstFrame"
Timeout(c) ==
  /\ phase[c] \in Active
  /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ guard' = IF guard = c THEN Nobody ELSE guard
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Timeout"
Disconnect(c) ==
  /\ phase[c] \in {"ready", "failed"}
  /\ phase' = [phase EXCEPT ![c] = "connecting"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Disconnect"
ServerLoss ==
  /\ server \in {"starting", "live"} /\ server' = "dead" /\ worker' = FALSE /\ lifetime' = FALSE
  /\ UNCHANGED <<phase, guard, marker, launches, authorized, hostAttempts, host, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "ServerLoss"
WorkerLoss ==
  /\ worker /\ worker' = FALSE
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "WorkerLoss"
\* A Host that bound its endpoint and then ended may leave that endpoint behind.
\* After a handoff no later start or replacement meets that endpoint, so the model leaves it alone.
HostLoss ==
  /\ host \in {"starting", "live"} /\ host' = "dead"
  /\ endpoint' \in IF dispatch = "settled" THEN {endpoint} ELSE {endpoint, "stale"}
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, hostAttempts, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, workerStarts>>
  /\ action' = "HostLoss"
Reject(c) ==
  \* Invalid/replayed authorization or non-Launch traffic cannot create a Host.
  /\ phase[c] = "opening"
  /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Reject"
Handoff ==
  /\ dispatch = "authorized" /\ host = "live" /\ worker /\ dispatchWorker = incarnation
  /\ dispatch' = "settled"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "Handoff"
\* A pre-handoff failure: preparation, lease expiry, a Host start that failed, or no room left for
\* the creation. The Launch is answered and a start in progress has failed. The Worker keeps
\* serving unless the failed start leaves no further one: a Host still running at the start
\* deadline, an endpoint left behind, or its last start. Then it answers and exits, so no later
\* StartHost can meet that Host, and the Session's stop cleans up. A Windows or macOS registration
\* that outlives its process is this action followed by WorkerLoss.
StartFailed == host \in {"starting", "dead"}
StartLeft == host = "dead" /\ endpoint = "absent" /\ workerStarts < MaxHostStarts
ExpireDispatch ==
  /\ dispatch = "authorized" /\ dispatch' = "failed"
  /\ host' = IF StartFailed THEN "failed" ELSE host
  /\ worker' = IF StartFailed THEN worker /\ StartLeft ELSE worker
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, lifetime, workerAttempts, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "ExpireDispatch"
\* A start that meets an endpoint already there fails before any Host process, as every later start
\* would: the Worker answers its Launch and exits.
RefusedStart ==
  /\ dispatch = "authorized" /\ worker /\ host \in {"absent", "failed"}
  /\ workerStarts < MaxHostStarts /\ endpoint = "stale" /\ dispatchWorker = incarnation
  /\ dispatch' = "failed" /\ worker' = FALSE
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, workerAttempts, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "RefusedStart"
\* The Server replaces a lost Worker, keeping its Worker id under a new generation. The replacement
\* adopts a live Host, or serves with no Host when none was started or the last start failed; it
\* starts, replays and launches nothing, and makes its own Host starts. A Host still starting, or one
\* gone, ends the Server instead (ServerLoss). The model bounds a generation at one replacement.
ReplaceWorker ==
  /\ ~worker /\ server = "live" /\ lifetime /\ workerAttempts = 1 /\ incarnation = 1
  /\ host \in {"absent", "live", "failed"}
  /\ worker' = TRUE /\ incarnation' = incarnation + 1
  /\ hostOwner' = IF host = "live" THEN incarnation + 1 ELSE hostOwner
  /\ workerStarts' = 0
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, dispatchWorker, endpoint>>
  /\ action' = "ReplaceWorker"
ClientLoss(c) ==
  /\ phase[c] \in Active /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ guard' = IF guard = c THEN Nobody ELSE guard
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>
  /\ action' = "ClientLoss"
Next ==
  \/ \E c \in Clients: Acquire(c) \/ Commit(c) \/ Spawn(c) \/ ClientLoss(c) \/ Reject(c) \/ Wait(c) \/ Recovery(c)
      \/ Connect(c) \/ Open(c) \/ FirstFrame(c) \/ Timeout(c) \/ Disconnect(c)
  \/ AcquireLifetime \/ Handoff \/ ExpireDispatch \/ RefusedStart \/ WorkerReady \/ Authorize \/ StartHost \/ HostReady \/ ServerLoss \/ WorkerLoss \/ HostLoss
  \/ ReplaceWorker
Spec == Init /\ [][Next]_vars /\ \A c \in Clients: WF_vars(Timeout(c))
TypeOK ==
  /\ phase \in [Clients -> Active \cup {"ready", "failed"}]
  /\ guard \in Clients \cup {Nobody} /\ marker \in BOOLEAN /\ launches \in 0..1
  /\ server \in {"absent", "starting", "live", "dead"} /\ worker \in BOOLEAN
  /\ authorized \in BOOLEAN /\ hostAttempts \in 0..2
  /\ host \in {"absent", "starting", "live", "dead", "failed"}
  /\ lifetime \in BOOLEAN /\ workerAttempts \in 0..1
  /\ dispatch \in {"none", "authorized", "settled", "failed"}
  /\ incarnation \in 0..2 /\ dispatchWorker \in 0..2 /\ hostOwner \in 0..2
  /\ endpoint \in {"absent", "stale"} /\ workerStarts \in 0..MaxHostStarts
Singleton == (launches = 1 => marker) /\ (server # "absent" => marker)
LazyHost == hostAttempts > 0 => authorized /\ launches = 1
GuardOwnership == guard # Nobody => phase[guard] \in {"locked", "committed", "waiting"}
NoReplay == [][(marker => marker') /\ (hostAttempts <= hostAttempts')]_vars
RetainedDispatch == [][(action' = "StartHost" => UNCHANGED dispatch)]_vars
\* A Worker still serving after its own failed start can start again: nothing at the Host's address
\* and a start left. Every other failed start has ended it.
StartableAfterFailure ==
  worker /\ host = "failed" /\ hostOwner = incarnation => endpoint = "absent" /\ workerStarts < MaxHostStarts
\* A pre-handoff failure that started no Host keeps the Worker.
PreHandoffFailureKeepsWorker == [][(action' = "ExpireDispatch" /\ ~StartFailed => UNCHANGED worker)]_vars
RejectPreservesHost == [][(action' = "Reject" => UNCHANGED <<host, hostAttempts, incarnation, dispatchWorker, hostOwner, endpoint, workerStarts>>)]_vars
ClientLossPreservesSession == [][(action' = "ClientLoss" => UNCHANGED <<marker, launches, server, worker, host, incarnation, dispatchWorker, hostOwner, endpoint>>)]_vars
ReadyRequiresFrame == [][(\A c \in Clients: phase[c] # "ready" /\ phase'[c] = "ready"
                     => action' = "FirstFrame")]_vars
\* A Host that is starting answers the Launch authorized to the Worker that starts it.
HostAnswersItsOwnLaunch == host = "starting" => hostOwner = dispatchWorker /\ hostOwner = incarnation
ReplacementStartsNothing ==
  [][(action' = "ReplaceWorker" => UNCHANGED <<marker, launches, server, host, hostAttempts, dispatch>>)]_vars
AttemptTerminates == \A c \in Clients: phase[c] \in Active ~> phase[c] \in {"ready", "failed"}
=============================================================================
