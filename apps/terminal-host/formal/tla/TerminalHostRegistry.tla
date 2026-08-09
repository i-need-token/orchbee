------------------------ MODULE TerminalHostRegistry ------------------------
EXTENDS FiniteSets, TLC

\* Refines runtime-model claims TH-001, TH-004, and TH-005 through Connect,
\* AcceptHello, AttemptCreate, Settle, Disconnect, and Dispose. This model keeps
\* two terminal slots because identity uniqueness and broker-independent
\* retention are genuinely cross-terminal promises. Runtime completion,
\* failure containment, terminal-visible versions, and shutdown are abstracted
\* behind the Running/Settled registry status.

CONSTANTS Terminals, CreationIds, Fingerprints

None == "none"
BrokerPhases == {"none", "awaiting_hello", "active"}
RegistryStatuses == {"absent", "running", "settled"}

VARIABLES brokerPhase, creationOf, fingerprintOf, statusOf

registryVars == <<creationOf, fingerprintOf, statusOf>>
vars == <<brokerPhase, registryVars>>

PresentTerminals == {terminal \in Terminals: statusOf[terminal] # "absent"}
UsedCreations == {creationOf[terminal]: terminal \in PresentTerminals}

Init ==
  /\ brokerPhase = "none"
  /\ creationOf = [terminal \in Terminals |-> None]
  /\ fingerprintOf = [terminal \in Terminals |-> None]
  /\ statusOf = [terminal \in Terminals |-> "absent"]

Connect ==
  /\ brokerPhase = "none"
  /\ brokerPhase' = "awaiting_hello"
  /\ UNCHANGED registryVars

AcceptHello ==
  /\ brokerPhase = "awaiting_hello"
  /\ brokerPhase' = "active"
  /\ UNCHANGED registryVars

RejectHello ==
  /\ brokerPhase = "awaiting_hello"
  /\ brokerPhase' = "none"
  /\ UNCHANGED registryVars

Disconnect ==
  /\ brokerPhase \in {"awaiting_hello", "active"}
  /\ brokerPhase' = "none"
  /\ UNCHANGED registryVars

AttemptCreate(creation, fingerprint, terminal, spawnSucceeds) ==
  /\ brokerPhase = "active"
  /\ creation \in CreationIds
  /\ fingerprint \in Fingerprints
  /\ terminal \in Terminals
  /\ spawnSucceeds \in BOOLEAN
  /\ IF creation \notin UsedCreations
        /\ statusOf[terminal] = "absent"
        /\ spawnSucceeds
     THEN
       /\ creationOf' = [creationOf EXCEPT ![terminal] = creation]
       /\ fingerprintOf' = [fingerprintOf EXCEPT ![terminal] = fingerprint]
       /\ statusOf' = [statusOf EXCEPT ![terminal] = "running"]
     ELSE
       \* Spawn failure, an exact replay, a conflicting fingerprint, and an
       \* occupied slot all leave the committed registry unchanged.
       UNCHANGED registryVars
  /\ UNCHANGED brokerPhase

Settle(terminal) ==
  /\ terminal \in PresentTerminals
  /\ statusOf[terminal] = "running"
  /\ statusOf' = [statusOf EXCEPT ![terminal] = "settled"]
  /\ UNCHANGED <<brokerPhase, creationOf, fingerprintOf>>

Dispose(terminal) ==
  /\ brokerPhase = "active"
  /\ terminal \in PresentTerminals
  /\ statusOf[terminal] = "settled"
  /\ creationOf' = [creationOf EXCEPT ![terminal] = None]
  /\ fingerprintOf' = [fingerprintOf EXCEPT ![terminal] = None]
  /\ statusOf' = [statusOf EXCEPT ![terminal] = "absent"]
  /\ UNCHANGED brokerPhase

Next ==
  \/ Connect
  \/ AcceptHello
  \/ RejectHello
  \/ Disconnect
  \/ \E creation \in CreationIds, fingerprint \in Fingerprints,
        terminal \in Terminals, spawnSucceeds \in BOOLEAN:
       AttemptCreate(creation, fingerprint, terminal, spawnSucceeds)
  \/ \E terminal \in Terminals: Settle(terminal)
  \/ \E terminal \in Terminals: Dispose(terminal)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ brokerPhase \in BrokerPhases
  /\ creationOf \in [Terminals -> (CreationIds \cup {None})]
  /\ fingerprintOf \in [Terminals -> (Fingerprints \cup {None})]
  /\ statusOf \in [Terminals -> RegistryStatuses]

RegistryEntryIntegrity ==
  /\ Cardinality(UsedCreations) = Cardinality(PresentTerminals)
  /\ \A terminal \in Terminals:
       /\ (terminal \in PresentTerminals) <=>
            creationOf[terminal] \in CreationIds
       /\ (terminal \in PresentTerminals) <=>
            fingerprintOf[terminal] \in Fingerprints
       /\ (terminal \notin PresentTerminals) =>
            /\ creationOf[terminal] = None
            /\ fingerprintOf[terminal] = None

DisconnectPreservesRetainedTerminals ==
  [][Disconnect => UNCHANGED registryVars]_vars

FailedSpawnIsAtomic ==
  [][\A creation \in CreationIds, fingerprint \in Fingerprints,
       terminal \in Terminals:
       AttemptCreate(creation, fingerprint, terminal, FALSE) =>
         UNCHANGED registryVars]_vars

DisposeReleasesWholeEntry ==
  [][\A terminal \in Terminals:
       Dispose(terminal) =>
         /\ creationOf'[terminal] = None
         /\ fingerprintOf'[terminal] = None
         /\ statusOf'[terminal] = "absent"]_vars

=============================================================================
