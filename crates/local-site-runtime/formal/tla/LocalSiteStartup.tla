-------------------------- MODULE LocalSiteStartup --------------------------
EXTENDS Naturals, FiniteSets, TLC
CONSTANTS Clients, Nobody
VARIABLES phase, guard, marker, launches, server, worker, authorized, hostAttempts,
          host, lifetime, workerAttempts, dispatch, action
vars == <<phase, guard, marker, launches, server, worker, authorized, hostAttempts,
          host, lifetime, workerAttempts, dispatch, action>>
Active == {"connecting", "locked", "committed", "waiting", "opening", "frame"}
Init ==
  /\ phase = [c \in Clients |-> "connecting"] /\ guard = Nobody
  /\ marker = FALSE /\ launches = 0 /\ server = "absent" /\ worker = FALSE
  /\ authorized = FALSE /\ hostAttempts = 0 /\ host = "absent" /\ action = "Init"
  /\ lifetime \in BOOLEAN /\ workerAttempts = 0 /\ dispatch = "none"
Acquire(c) ==
  /\ phase[c] = "connecting" /\ guard = Nobody
  /\ phase' = [phase EXCEPT ![c] = "locked"] /\ guard' = c
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Acquire"
Commit(c) ==
  /\ phase[c] = "locked" /\ ~marker /\ ~lifetime /\ server = "absent"
  /\ marker' = TRUE
  /\ phase' = [phase EXCEPT ![c] = "committed"]
  /\ UNCHANGED <<guard, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Commit"
Spawn(c) ==
  /\ phase[c] = "committed" /\ marker /\ launches = 0
  /\ launches' = 1 /\ server' = "starting"
  /\ phase' = [phase EXCEPT ![c] = "waiting"]
  /\ UNCHANGED <<guard, marker, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Spawn"
AcquireLifetime ==
  /\ server = "starting" /\ marker /\ ~lifetime
  /\ lifetime' = TRUE /\ server' = "live"
  /\ UNCHANGED <<phase, guard, marker, launches, worker, authorized, hostAttempts, host, workerAttempts, dispatch>>
  /\ action' = "AcquireLifetime"
Wait(c) ==
  /\ phase[c] = "locked" /\ marker /\ lifetime
  /\ phase' = [phase EXCEPT ![c] = "waiting"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Wait"
Recovery(c) ==
  /\ phase[c] = "locked" /\ ((marker /\ ~lifetime) \/ (~marker /\ lifetime))
  /\ phase' = [phase EXCEPT ![c] = "failed"] /\ guard' = Nobody
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Recovery"
WorkerReady ==
  /\ server = "live" /\ lifetime /\ workerAttempts = 0 /\ worker' = TRUE /\ workerAttempts' = 1
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, dispatch>>
  /\ action' = "WorkerReady"
Connect(c) ==
  /\ phase[c] \in {"connecting", "locked", "waiting"} /\ server = "live" /\ worker
  /\ phase' = [phase EXCEPT ![c] = "opening"]
  /\ guard' = IF guard = c THEN Nobody ELSE guard
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Connect"
Authorize ==
  /\ \E c \in Clients: phase[c] = "opening"
  /\ server = "live" /\ worker /\ ~authorized
  /\ authorized' = TRUE /\ dispatch' = "authorized"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, hostAttempts, host, lifetime, workerAttempts>>
  /\ action' = "Authorize"
StartHost ==
  /\ dispatch = "authorized" /\ worker /\ hostAttempts = 0
  /\ hostAttempts' = 1 /\ host' = "starting"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, lifetime, workerAttempts, dispatch>>
  /\ action' = "StartHost"
HostReady ==
  /\ host = "starting" /\ worker
  /\ host' = "live"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, hostAttempts, lifetime, workerAttempts, dispatch>>
  /\ action' = "HostReady"
Open(c) ==
  /\ phase[c] = "opening" /\ worker /\ host = "live" /\ dispatch = "settled"
  /\ phase' = [phase EXCEPT ![c] = "frame"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Open"
FirstFrame(c) ==
  /\ phase[c] = "frame" /\ worker /\ host = "live"
  /\ phase' = [phase EXCEPT ![c] = "ready"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "FirstFrame"
Timeout(c) ==
  /\ phase[c] \in Active
  /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ guard' = IF guard = c THEN Nobody ELSE guard
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Timeout"
Disconnect(c) ==
  /\ phase[c] \in {"ready", "failed"}
  /\ phase' = [phase EXCEPT ![c] = "connecting"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Disconnect"
ServerLoss ==
  /\ server \in {"starting", "live"} /\ server' = "dead" /\ worker' = FALSE /\ lifetime' = FALSE
  /\ UNCHANGED <<phase, guard, marker, launches, authorized, hostAttempts, host, workerAttempts, dispatch>>
  /\ action' = "ServerLoss"
WorkerLoss ==
  /\ worker /\ worker' = FALSE
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "WorkerLoss"
HostLoss ==
  /\ host \in {"starting", "live"} /\ host' = "dead"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, hostAttempts, lifetime, workerAttempts, dispatch>>
  /\ action' = "HostLoss"
Reject(c) ==
  \* Invalid/replayed authorization or non-Launch traffic cannot create a Host.
  /\ phase[c] = "opening"
  /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ UNCHANGED <<guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "Reject"
Handoff ==
  /\ dispatch = "authorized" /\ host = "live" /\ worker
  /\ dispatch' = "settled"
  /\ UNCHANGED <<phase, guard, marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts>>
  /\ action' = "Handoff"
ExpireDispatch ==
  /\ dispatch = "authorized" /\ dispatch' = "failed" /\ worker' = FALSE
  /\ UNCHANGED <<phase, guard, marker, launches, server, authorized, hostAttempts, host, lifetime, workerAttempts>>
  /\ action' = "ExpireDispatch"
ClientLoss(c) ==
  /\ phase[c] \in Active /\ phase' = [phase EXCEPT ![c] = "failed"]
  /\ guard' = IF guard = c THEN Nobody ELSE guard
  /\ UNCHANGED <<marker, launches, server, worker, authorized, hostAttempts, host, lifetime, workerAttempts, dispatch>>
  /\ action' = "ClientLoss"
Next ==
  \/ \E c \in Clients: Acquire(c) \/ Commit(c) \/ Spawn(c) \/ ClientLoss(c) \/ Reject(c) \/ Wait(c) \/ Recovery(c)
      \/ Connect(c) \/ Open(c) \/ FirstFrame(c) \/ Timeout(c) \/ Disconnect(c)
  \/ AcquireLifetime \/ Handoff \/ ExpireDispatch \/ WorkerReady \/ Authorize \/ StartHost \/ HostReady \/ ServerLoss \/ WorkerLoss \/ HostLoss
Spec == Init /\ [][Next]_vars /\ \A c \in Clients: WF_vars(Timeout(c))
TypeOK ==
  /\ phase \in [Clients -> Active \cup {"ready", "failed"}]
  /\ guard \in Clients \cup {Nobody} /\ marker \in BOOLEAN /\ launches \in 0..1
  /\ server \in {"absent", "starting", "live", "dead"} /\ worker \in BOOLEAN
  /\ authorized \in BOOLEAN /\ hostAttempts \in 0..1
  /\ host \in {"absent", "starting", "live", "dead"}
  /\ lifetime \in BOOLEAN /\ workerAttempts \in 0..1
  /\ dispatch \in {"none", "authorized", "settled", "failed"}
Singleton == (launches = 1 => marker) /\ (server # "absent" => marker)
LazyHost == hostAttempts > 0 => authorized /\ launches = 1
GuardOwnership == guard # Nobody => phase[guard] \in {"locked", "committed", "waiting"}
NoReplay == [][(marker => marker') /\ (hostAttempts <= hostAttempts')]_vars
RetainedDispatch == [][(action' = "StartHost" => UNCHANGED dispatch)]_vars
RejectPreservesHost == [][(action' = "Reject" => UNCHANGED <<host, hostAttempts>>)]_vars
ClientLossPreservesSite == [][(action' = "ClientLoss" => UNCHANGED <<marker, launches, server, worker, host>>)]_vars
ReadyRequiresFrame == [][(\A c \in Clients: phase[c] # "ready" /\ phase'[c] = "ready"
                     => action' = "FirstFrame")]_vars
AttemptTerminates == \A c \in Clients: phase[c] \in Active ~> phase[c] \in {"ready", "failed"}
=============================================================================
