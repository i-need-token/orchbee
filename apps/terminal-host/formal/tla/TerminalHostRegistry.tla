------------------------ MODULE TerminalHostRegistry ------------------------
EXTENDS FiniteSets, TLC

\* Refines runtime-model claims TH-001, TH-004, and TH-005 through Connect,
\* AcceptHello, AttemptCreate, Settle, Disconnect, and Dispose. This model keeps
\* two terminal slots because identity uniqueness and broker-independent
\* retention are genuinely cross-terminal promises. Runtime completion,
\* failure containment, terminal-visible versions, and shutdown are abstracted
\* behind the Running/Settled registry status.

CONSTANTS Terminals, CreationIds, LaunchFingerprints, OverlayFingerprints

None == "none"
BrokerPhases == {"none", "awaiting_hello", "active"}
RegistryStatuses == {"absent", "running", "settled"}
CreateResults == {"none", "created", "replayed", "conflict", "rejected"}

VARIABLES brokerPhase, creationOf, launchFingerprintOf, overlayFingerprintOf,
          statusOf, lastCreateResult

registryVars == <<creationOf, launchFingerprintOf, overlayFingerprintOf, statusOf>>
vars == <<brokerPhase, registryVars, lastCreateResult>>

PresentTerminals == {terminal \in Terminals: statusOf[terminal] # "absent"}
FreeTerminals == Terminals \ PresentTerminals
UsedCreations == {creationOf[terminal]: terminal \in PresentTerminals}

Init ==
  /\ brokerPhase = "none"
  /\ creationOf = [terminal \in Terminals |-> None]
  /\ launchFingerprintOf = [terminal \in Terminals |-> None]
  /\ overlayFingerprintOf = [terminal \in Terminals |-> None]
  /\ statusOf = [terminal \in Terminals |-> "absent"]
  /\ lastCreateResult = "none"

Connect ==
  /\ brokerPhase = "none"
  /\ brokerPhase' = "awaiting_hello"
  /\ UNCHANGED <<registryVars, lastCreateResult>>

AcceptHello ==
  /\ brokerPhase = "awaiting_hello"
  /\ brokerPhase' = "active"
  /\ UNCHANGED <<registryVars, lastCreateResult>>

RejectHello ==
  /\ brokerPhase = "awaiting_hello"
  /\ brokerPhase' = "none"
  /\ UNCHANGED <<registryVars, lastCreateResult>>

Disconnect ==
  /\ brokerPhase \in {"awaiting_hello", "active"}
  /\ brokerPhase' = "none"
  /\ UNCHANGED <<registryVars, lastCreateResult>>

TerminalForCreation(creation) ==
  CHOOSE terminal \in PresentTerminals: creationOf[terminal] = creation

AttemptCreate(creation, launchFingerprint, overlayFingerprint, enforceOverlayMatch, terminal, spawnSucceeds) ==
  /\ brokerPhase = "active"
  /\ creation \in CreationIds
  /\ launchFingerprint \in LaunchFingerprints
  /\ overlayFingerprint \in OverlayFingerprints
  /\ enforceOverlayMatch \in BOOLEAN
  /\ terminal \in Terminals
  /\ spawnSucceeds \in BOOLEAN
  \* A fresh terminal ID is generated internally after capacity reservation.
  \* A collision is retried before this post-reservation action is observed, so
  \* an available slot may never be exposed as a rejected occupied selection.
  /\ ((creation \notin UsedCreations /\ FreeTerminals # {}) =>
       terminal \in FreeTerminals)
  /\ IF creation \notin UsedCreations
        /\ FreeTerminals # {}
        /\ spawnSucceeds
     THEN
       /\ creationOf' = [creationOf EXCEPT ![terminal] = creation]
       /\ launchFingerprintOf' =
            [launchFingerprintOf EXCEPT ![terminal] = launchFingerprint]
       /\ overlayFingerprintOf' =
            [overlayFingerprintOf EXCEPT ![terminal] = overlayFingerprint]
       /\ statusOf' = [statusOf EXCEPT ![terminal] = "running"]
     ELSE
       \* Capacity or spawn failure, replay, and conflict leave the committed
       \* registry unchanged.
       UNCHANGED registryVars
  /\ IF creation \in UsedCreations
       THEN
         LET existing == TerminalForCreation(creation) IN
           IF launchFingerprintOf[existing] = launchFingerprint /\
              (~enforceOverlayMatch \/
               overlayFingerprintOf[existing] = overlayFingerprint)
           THEN lastCreateResult' = "replayed"
           ELSE lastCreateResult' = "conflict"
       ELSE IF FreeTerminals # {} /\ spawnSucceeds
         THEN lastCreateResult' = "created"
         ELSE lastCreateResult' = "rejected"
  /\ UNCHANGED brokerPhase

Settle(terminal) ==
  /\ terminal \in PresentTerminals
  /\ statusOf[terminal] = "running"
  /\ statusOf' = [statusOf EXCEPT ![terminal] = "settled"]
  /\ UNCHANGED <<brokerPhase, creationOf, launchFingerprintOf,
                  overlayFingerprintOf, lastCreateResult>>

Dispose(terminal) ==
  /\ brokerPhase = "active"
  /\ terminal \in PresentTerminals
  /\ statusOf[terminal] = "settled"
  /\ creationOf' = [creationOf EXCEPT ![terminal] = None]
  /\ launchFingerprintOf' = [launchFingerprintOf EXCEPT ![terminal] = None]
  /\ overlayFingerprintOf' = [overlayFingerprintOf EXCEPT ![terminal] = None]
  /\ statusOf' = [statusOf EXCEPT ![terminal] = "absent"]
  /\ UNCHANGED <<brokerPhase, lastCreateResult>>

Next ==
  \/ Connect
  \/ AcceptHello
  \/ RejectHello
  \/ Disconnect
  \/ \E creation \in CreationIds,
        launchFingerprint \in LaunchFingerprints,
        overlayFingerprint \in OverlayFingerprints,
        enforceOverlayMatch \in BOOLEAN,
        terminal \in Terminals, spawnSucceeds \in BOOLEAN:
       AttemptCreate(creation, launchFingerprint, overlayFingerprint,
                     enforceOverlayMatch, terminal, spawnSucceeds)
  \/ \E terminal \in Terminals: Settle(terminal)
  \/ \E terminal \in Terminals: Dispose(terminal)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ brokerPhase \in BrokerPhases
  /\ creationOf \in [Terminals -> (CreationIds \cup {None})]
  /\ launchFingerprintOf \in [Terminals -> (LaunchFingerprints \cup {None})]
  /\ overlayFingerprintOf \in [Terminals -> (OverlayFingerprints \cup {None})]
  /\ statusOf \in [Terminals -> RegistryStatuses]
  /\ lastCreateResult \in CreateResults

RegistryEntryIntegrity ==
  /\ Cardinality(UsedCreations) = Cardinality(PresentTerminals)
  /\ \A terminal \in Terminals:
       /\ (terminal \in PresentTerminals) <=>
            creationOf[terminal] \in CreationIds
       /\ (terminal \in PresentTerminals) <=>
            launchFingerprintOf[terminal] \in LaunchFingerprints
       /\ (terminal \in PresentTerminals) <=>
            overlayFingerprintOf[terminal] \in OverlayFingerprints
       /\ (terminal \notin PresentTerminals) =>
            /\ creationOf[terminal] = None
            /\ launchFingerprintOf[terminal] = None
            /\ overlayFingerprintOf[terminal] = None

DisconnectPreservesRetainedTerminals ==
  [][Disconnect => UNCHANGED registryVars]_vars

FailedSpawnIsAtomic ==
  [][\A creation \in CreationIds,
       launchFingerprint \in LaunchFingerprints,
       overlayFingerprint \in OverlayFingerprints,
       enforceOverlayMatch \in BOOLEAN, terminal \in Terminals:
       AttemptCreate(creation, launchFingerprint, overlayFingerprint,
                     enforceOverlayMatch, terminal, FALSE) =>
         UNCHANGED registryVars]_vars

FreshSuccessfulCreateCommitsWholeEntry ==
  [][\A creation \in CreationIds,
       launchFingerprint \in LaunchFingerprints,
       overlayFingerprint \in OverlayFingerprints,
       enforceOverlayMatch \in BOOLEAN, terminal \in Terminals:
       /\ creation \notin UsedCreations
       /\ FreeTerminals # {}
       /\ AttemptCreate(creation, launchFingerprint, overlayFingerprint,
                        enforceOverlayMatch, terminal, TRUE)
       => /\ terminal \in FreeTerminals
          /\ lastCreateResult' = "created"
          /\ creationOf'[terminal] = creation
          /\ launchFingerprintOf'[terminal] = launchFingerprint
          /\ overlayFingerprintOf'[terminal] = overlayFingerprint
          /\ statusOf'[terminal] = "running"]_vars

WildcardReplayIgnoresOverlayFingerprint ==
  [][\A creation \in UsedCreations,
       launchFingerprint \in LaunchFingerprints,
       overlayFingerprint \in OverlayFingerprints,
       terminal \in Terminals, spawnSucceeds \in BOOLEAN:
       launchFingerprintOf[TerminalForCreation(creation)] = launchFingerprint /\
       AttemptCreate(creation, launchFingerprint, overlayFingerprint,
                     FALSE, terminal, spawnSucceeds) =>
         lastCreateResult' = "replayed"]_vars

SuppliedOverlayConflictIsAtomic ==
  [][\A creation \in UsedCreations,
       launchFingerprint \in LaunchFingerprints,
       overlayFingerprint \in OverlayFingerprints,
       terminal \in Terminals, spawnSucceeds \in BOOLEAN:
       launchFingerprintOf[TerminalForCreation(creation)] = launchFingerprint /\
       overlayFingerprintOf[TerminalForCreation(creation)] # overlayFingerprint /\
       AttemptCreate(creation, launchFingerprint, overlayFingerprint,
                     TRUE, terminal, spawnSucceeds) =>
         /\ lastCreateResult' = "conflict"
         /\ UNCHANGED registryVars]_vars

DisposeReleasesWholeEntry ==
  [][\A terminal \in Terminals:
       Dispose(terminal) =>
         /\ creationOf'[terminal] = None
         /\ launchFingerprintOf'[terminal] = None
         /\ overlayFingerprintOf'[terminal] = None
         /\ statusOf'[terminal] = "absent"]_vars

=============================================================================
