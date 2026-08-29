---------------- MODULE SshEnvironmentRemoteTerminalOutcome ----------------
EXTENDS FiniteSets, Naturals, TLC

\* Bounded model of Slice 3.2D. Terminal Host observations are fenced by the
\* current Environment connection. Exit outcomes enter an Environment-scoped
\* durable journal before the listener returns, then advance through durable
\* binding outcome, reconciler reservation, publication ledger, Client
\* visibility, and journal compaction. Canonical deletion or retargeting cannot
\* redirect an outcome. Owner restart preserves durable stages and requires a
\* later explicit Environment connection before recovery can publish.

CONSTANT Environments, Identities, MaxRevision, MaxJournalEntries, Scenario

ASSUME Cardinality(Environments) = 2
ASSUME Cardinality(Identities) = 1
ASSUME MaxRevision \in Nat \ {0}
ASSUME MaxJournalEntries \in Nat \ {0}

Keys == Environments \X Identities
NoTarget == "no-target"
NoKey == "no-key"
Targets == Environments \cup {NoTarget}
Revisions == 0..MaxRevision
Scenarios == {
  "metadata", "failure", "exit-retry", "admission-rejection", "restart",
  "isolation"
}
ExitScenarios == {"exit-retry", "admission-rejection", "restart", "isolation"}

ASSUME Scenario \in Scenarios

OwnEnvironment(key) == key[1]
KeysFor(environment) == {key \in Keys : OwnEnvironment(key) = environment}
OtherEnvironment(key) ==
  CHOOSE environment \in Environments : environment # OwnEnvironment(key)
FocusKey == CHOOSE key \in Keys : TRUE
KeyInScenario(key) ==
  key = FocusKey \/ Scenario \in {"admission-rejection", "isolation"}
EnvironmentInScenario(environment) ==
  environment = OwnEnvironment(FocusKey) \/
    Scenario \in {"admission-rejection", "isolation"}

Actions == {
  "init", "explicit-connect", "load-recovery", "subscribe-current-record",
  "initialize-runtime", "associate-identity", "observe-metadata",
  "publish-metadata", "observe-failure", "record-failure",
  "publish-failure", "admit-exit", "reject-exit-admission",
  "retry-rejected-admission", "persist-binding-outcome",
  "reserve-exit-publication", "commit-publication-ledger",
  "publish-exit", "fail-exit-publication", "retry-exit-publication",
  "compact-exit-journal", "delete-canonical-target",
  "retarget-canonical-target", "restore-canonical-target",
  "stale-envelope-ignored", "disconnect", "release-owner",
  "acquire-owner", "recover-ledger-visibility", "recover-failure-visibility"
}

VARIABLE s

vars == <<s>>

Init ==
  s = [
    ownerPresent |-> TRUE,
    connected |-> {},
    recoveryLoaded |-> {},
    subscribed |-> {},
    runtimeReady |-> {},
    bindingKnown |-> Keys,
    associated |-> {},
    canonicalTarget |-> [key \in Keys |-> OwnEnvironment(key)],
    metadataSeen |-> [key \in Keys |-> 0],
    metadataVisible |-> [key \in Keys |-> 0],
    metadataTarget |-> [key \in Keys |-> NoTarget],
    failureSeen |-> {},
    failureDurable |-> {},
    failureVisible |-> {},
    failureTarget |-> [key \in Keys |-> NoTarget],
    journal |-> {},
    rejected |-> {},
    bindingOutcomeDurable |-> {},
    reserved |-> {},
    ledger |-> {},
    publicationFailed |-> {},
    exitVisible |-> {},
    publicationTarget |-> [key \in Keys |-> NoTarget],
    compacted |-> {},
    lastAction |-> "init",
    previousConnected |-> {},
    previousMetadataVisible |-> [key \in Keys |-> 0]
  ]

Record(next, action, ignoredKey) ==
  [next EXCEPT
    !.lastAction = action,
    !.previousConnected = s.connected,
    !.previousMetadataVisible = s.metadataVisible]

ExplicitConnect(environment) ==
  /\ s.ownerPresent
  /\ EnvironmentInScenario(environment)
  /\ environment \notin s.connected
  /\ s' = Record(
       [s EXCEPT !.connected = @ \cup {environment}],
       "explicit-connect",
       NoKey)

LoadRecovery(environment) ==
  /\ environment \in s.connected
  /\ environment \notin s.recoveryLoaded
  /\ s' = Record(
       [s EXCEPT !.recoveryLoaded = @ \cup {environment}],
       "load-recovery",
       NoKey)

SubscribeCurrentRecord(environment) ==
  /\ environment \in s.connected \cap s.recoveryLoaded
  /\ environment \notin s.subscribed
  /\ s' = Record(
       [s EXCEPT !.subscribed = @ \cup {environment}],
       "subscribe-current-record",
       NoKey)

InitializeRuntime(environment) ==
  /\ environment \in s.subscribed
  /\ environment \notin s.runtimeReady
  /\ s' = Record(
       [s EXCEPT !.runtimeReady = @ \cup {environment}],
       "initialize-runtime",
       NoKey)

AssociateIdentity(key) ==
  /\ KeyInScenario(key)
  /\ key \in s.bindingKnown
  /\ OwnEnvironment(key) \in s.runtimeReady
  /\ key \notin s.associated
  /\ s' = Record(
       [s EXCEPT !.associated = @ \cup {key}],
       "associate-identity",
       key)

ObserveMetadata(key, revision) ==
  /\ Scenario = "metadata"
  /\ KeyInScenario(key)
  /\ OwnEnvironment(key) \in s.subscribed
  /\ revision \in Revisions
  /\ revision > s.metadataSeen[key]
  /\ s' = Record(
       [s EXCEPT !.metadataSeen[key] = revision],
       "observe-metadata",
       key)

PublishMetadata(key) ==
  /\ Scenario = "metadata"
  /\ KeyInScenario(key)
  /\ key \in s.associated
  /\ OwnEnvironment(key) \in s.runtimeReady
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s.metadataSeen[key] > s.metadataVisible[key]
  /\ s' = Record(
       [s EXCEPT
         !.metadataVisible[key] = s.metadataSeen[key],
         !.metadataTarget[key] = OwnEnvironment(key)],
       "publish-metadata",
       key)

ObserveFailure(key) ==
  /\ Scenario = "failure"
  /\ KeyInScenario(key)
  /\ OwnEnvironment(key) \in s.subscribed
  /\ key \notin s.failureSeen
  /\ s' = Record(
       [s EXCEPT !.failureSeen = @ \cup {key}],
       "observe-failure",
       key)

RecordFailure(key) ==
  /\ Scenario = "failure"
  /\ KeyInScenario(key)
  /\ key \in s.failureSeen \cap s.associated
  /\ OwnEnvironment(key) \in s.runtimeReady
  /\ key \notin s.failureDurable
  /\ s' = Record(
       [s EXCEPT !.failureDurable = @ \cup {key}],
       "record-failure",
       key)

PublishFailure(key) ==
  /\ Scenario = "failure"
  /\ KeyInScenario(key)
  /\ key \in s.failureDurable
  /\ key \notin s.failureVisible
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT
         !.failureVisible = @ \cup {key},
         !.failureTarget[key] = OwnEnvironment(key)],
       "publish-failure",
       key)

AdmitExit(key) ==
  /\ Scenario \in ExitScenarios
  /\ KeyInScenario(key)
  /\ OwnEnvironment(key) \in s.subscribed
  /\ key \notin s.journal \cup s.rejected \cup s.ledger
  /\ Cardinality(s.journal) < MaxJournalEntries
  /\ s' = Record(
       [s EXCEPT !.journal = @ \cup {key}],
       "admit-exit",
       key)

RejectExitAdmission(key) ==
  /\ Scenario = "admission-rejection"
  /\ KeyInScenario(key)
  /\ OwnEnvironment(key) \in s.subscribed
  /\ key \notin s.journal \cup s.rejected \cup s.ledger
  /\ Cardinality(s.journal) = MaxJournalEntries
  /\ s' = Record(
       [s EXCEPT !.rejected = @ \cup {key}],
       "reject-exit-admission",
       key)

RetryRejectedAdmission(key) ==
  /\ Scenario = "admission-rejection"
  /\ KeyInScenario(key)
  /\ key \in s.rejected
  /\ OwnEnvironment(key) \in s.recoveryLoaded
  /\ Cardinality(s.journal) < MaxJournalEntries
  /\ s' = Record(
       [s EXCEPT
         !.rejected = @ \ {key},
         !.journal = @ \cup {key}],
       "retry-rejected-admission",
       key)

PersistBindingOutcome(key) ==
  /\ Scenario \in ExitScenarios
  /\ KeyInScenario(key)
  /\ key \in s.journal \cap s.associated
  /\ OwnEnvironment(key) \in s.runtimeReady
  /\ key \notin s.bindingOutcomeDurable
  /\ s' = Record(
       [s EXCEPT !.bindingOutcomeDurable = @ \cup {key}],
       "persist-binding-outcome",
       key)

ReserveExitPublication(key) ==
  /\ Scenario \in ExitScenarios
  /\ KeyInScenario(key)
  /\ key \in s.journal \cap s.bindingOutcomeDurable
  /\ key \notin s.reserved
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT !.reserved = @ \cup {key}],
       "reserve-exit-publication",
       key)

CommitPublicationLedger(key) ==
  /\ Scenario \in ExitScenarios
  /\ KeyInScenario(key)
  /\ key \in s.reserved
  /\ key \notin s.ledger
  /\ s' = Record(
       [s EXCEPT !.ledger = @ \cup {key}],
       "commit-publication-ledger",
       key)

PublishExit(key) ==
  /\ Scenario \in ExitScenarios
  /\ KeyInScenario(key)
  /\ key \in s.ledger
  /\ key \notin s.exitVisible \cup s.publicationFailed
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT
         !.exitVisible = @ \cup {key},
         !.publicationTarget[key] = OwnEnvironment(key)],
       "publish-exit",
       key)

FailExitPublication(key) ==
  /\ Scenario = "exit-retry"
  /\ key = FocusKey
  /\ key \in s.ledger
  /\ key \notin s.exitVisible \cup s.publicationFailed
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT !.publicationFailed = @ \cup {key}],
       "fail-exit-publication",
       key)

RetryExitPublication(key) ==
  /\ Scenario = "exit-retry"
  /\ key = FocusKey
  /\ key \in s.publicationFailed
  /\ key \notin s.exitVisible
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT
         !.publicationFailed = @ \ {key},
         !.exitVisible = @ \cup {key},
         !.publicationTarget[key] = OwnEnvironment(key)],
       "retry-exit-publication",
       key)

CompactExitJournal(key) ==
  /\ Scenario \in ExitScenarios
  /\ KeyInScenario(key)
  /\ key \in s.journal \cap s.exitVisible \cap s.ledger
  /\ s' = Record(
       [s EXCEPT
         !.journal = @ \ {key},
         !.compacted = @ \cup {key}],
       "compact-exit-journal",
       key)

DeleteCanonicalTarget(key) ==
  /\ Scenario = "exit-retry"
  /\ key = FocusKey
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT !.canonicalTarget[key] = NoTarget],
       "delete-canonical-target",
       key)

RetargetCanonicalTarget(key) ==
  /\ Scenario = "exit-retry"
  /\ key = FocusKey
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT !.canonicalTarget[key] = OtherEnvironment(key)],
       "retarget-canonical-target",
       key)

RestoreCanonicalTarget(key) ==
  /\ Scenario = "exit-retry"
  /\ key = FocusKey
  /\ s.canonicalTarget[key] # OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT !.canonicalTarget[key] = OwnEnvironment(key)],
       "restore-canonical-target",
       key)

StaleEnvelopeIgnored(key) ==
  /\ KeyInScenario(key)
  /\ OwnEnvironment(key) \notin s.subscribed
  /\ s' = Record(s, "stale-envelope-ignored", key)

Disconnect(environment) ==
  /\ environment \in s.connected
  /\ s' = Record(
       [s EXCEPT
         !.connected = @ \ {environment},
         !.recoveryLoaded = @ \ {environment},
         !.subscribed = @ \ {environment},
         !.runtimeReady = @ \ {environment},
         !.associated = @ \ KeysFor(environment)],
       "disconnect",
       NoKey)

ReleaseOwner ==
  /\ Scenario \in {"failure", "restart"}
  /\ s.ownerPresent
  /\ s' = Record(
       [s EXCEPT
         !.ownerPresent = FALSE,
         !.connected = {},
         !.recoveryLoaded = {},
         !.subscribed = {},
         !.runtimeReady = {},
         !.associated = {},
         !.metadataSeen = [key \in Keys |-> 0],
         !.metadataVisible = [key \in Keys |-> 0],
         !.metadataTarget = [key \in Keys |-> NoTarget],
         !.failureVisible = {},
         !.failureTarget = [key \in Keys |-> NoTarget],
         !.publicationFailed = {},
         !.exitVisible = {},
         !.publicationTarget = [key \in Keys |-> NoTarget]],
       "release-owner",
       NoKey)

AcquireOwner ==
  /\ Scenario \in {"failure", "restart"}
  /\ ~s.ownerPresent
  /\ s' = Record(
       [s EXCEPT !.ownerPresent = TRUE],
       "acquire-owner",
       NoKey)

RecoverLedgerVisibility(key) ==
  /\ Scenario = "restart"
  /\ key = FocusKey
  /\ key \in s.ledger \cap s.associated
  /\ key \notin s.exitVisible
  /\ OwnEnvironment(key) \in s.runtimeReady
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT
         !.exitVisible = @ \cup {key},
         !.publicationTarget[key] = OwnEnvironment(key)],
       "recover-ledger-visibility",
       key)

RecoverFailureVisibility(key) ==
  /\ Scenario = "failure"
  /\ key = FocusKey
  /\ key \in s.failureDurable \cap s.associated
  /\ key \notin s.failureVisible
  /\ OwnEnvironment(key) \in s.runtimeReady
  /\ s.canonicalTarget[key] = OwnEnvironment(key)
  /\ s' = Record(
       [s EXCEPT
         !.failureVisible = @ \cup {key},
         !.failureTarget[key] = OwnEnvironment(key)],
       "recover-failure-visibility",
       key)

Next ==
  \/ \E environment \in Environments: ExplicitConnect(environment)
  \/ \E environment \in Environments: LoadRecovery(environment)
  \/ \E environment \in Environments: SubscribeCurrentRecord(environment)
  \/ \E environment \in Environments: InitializeRuntime(environment)
  \/ \E key \in Keys: AssociateIdentity(key)
  \/ \E key \in Keys, revision \in Revisions: ObserveMetadata(key, revision)
  \/ \E key \in Keys: PublishMetadata(key)
  \/ \E key \in Keys: ObserveFailure(key)
  \/ \E key \in Keys: RecordFailure(key)
  \/ \E key \in Keys: PublishFailure(key)
  \/ \E key \in Keys: AdmitExit(key)
  \/ \E key \in Keys: RejectExitAdmission(key)
  \/ \E key \in Keys: RetryRejectedAdmission(key)
  \/ \E key \in Keys: PersistBindingOutcome(key)
  \/ \E key \in Keys: ReserveExitPublication(key)
  \/ \E key \in Keys: CommitPublicationLedger(key)
  \/ \E key \in Keys: PublishExit(key)
  \/ \E key \in Keys: FailExitPublication(key)
  \/ \E key \in Keys: RetryExitPublication(key)
  \/ \E key \in Keys: CompactExitJournal(key)
  \/ \E key \in Keys: DeleteCanonicalTarget(key)
  \/ \E key \in Keys: RetargetCanonicalTarget(key)
  \/ \E key \in Keys: RestoreCanonicalTarget(key)
  \/ \E key \in Keys: StaleEnvelopeIgnored(key)
  \/ \E environment \in Environments: Disconnect(environment)
  \/ ReleaseOwner
  \/ AcquireOwner
  \/ \E key \in Keys: RecoverLedgerVisibility(key)
  \/ \E key \in Keys: RecoverFailureVisibility(key)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.ownerPresent \in BOOLEAN
  /\ s.connected \subseteq Environments
  /\ s.recoveryLoaded \subseteq Environments
  /\ s.subscribed \subseteq Environments
  /\ s.runtimeReady \subseteq Environments
  /\ s.bindingKnown \subseteq Keys
  /\ s.associated \subseteq Keys
  /\ s.canonicalTarget \in [Keys -> Targets]
  /\ s.metadataSeen \in [Keys -> Revisions]
  /\ s.metadataVisible \in [Keys -> Revisions]
  /\ s.metadataTarget \in [Keys -> Targets]
  /\ s.failureSeen \subseteq Keys
  /\ s.failureDurable \subseteq Keys
  /\ s.failureVisible \subseteq Keys
  /\ s.failureTarget \in [Keys -> Targets]
  /\ s.journal \subseteq Keys
  /\ s.rejected \subseteq Keys
  /\ s.bindingOutcomeDurable \subseteq Keys
  /\ s.reserved \subseteq Keys
  /\ s.ledger \subseteq Keys
  /\ s.publicationFailed \subseteq Keys
  /\ s.exitVisible \subseteq Keys
  /\ s.publicationTarget \in [Keys -> Targets]
  /\ s.compacted \subseteq Keys
  /\ s.lastAction \in Actions
  /\ s.previousConnected \subseteq Environments
  /\ s.previousMetadataVisible \in [Keys -> Revisions]

ConnectionStagesAreNested ==
  /\ s.runtimeReady \subseteq s.subscribed
  /\ s.subscribed \subseteq s.recoveryLoaded
  /\ s.recoveryLoaded \subseteq s.connected

EnvironmentNamespacesAreDisjoint ==
  /\ \A key \in s.exitVisible: s.publicationTarget[key] = OwnEnvironment(key)
  /\ \A key \in s.failureVisible: s.failureTarget[key] = OwnEnvironment(key)
  /\ \A key \in Keys:
       s.metadataVisible[key] > 0 => s.metadataTarget[key] = OwnEnvironment(key)

ExitPublicationIsDurablyOrdered ==
  /\ s.reserved \subseteq s.bindingOutcomeDurable
  /\ s.ledger \subseteq s.reserved
  /\ s.exitVisible \subseteq s.ledger
  /\ s.compacted \subseteq s.ledger
  /\ s.publicationFailed \subseteq s.ledger

AdmissionIsNeverLost ==
  /\ s.journal \cap s.rejected = {}
  /\ Cardinality(s.journal) <= MaxJournalEntries
  /\ s.compacted \cap s.journal = {}

FailureIsNeverSynthetic ==
  /\ s.failureDurable \subseteq s.failureSeen
  /\ s.failureVisible \subseteq s.failureDurable

MetadataIsMonotonicWithinOwner ==
  \/ s.lastAction = "release-owner"
  \/ \A key \in Keys:
       s.metadataVisible[key] >= s.previousMetadataVisible[key]

ConnectionGrowthIsExplicit ==
  s.connected \ s.previousConnected = {} \/ s.lastAction = "explicit-connect"

RecoveryNeverActivatesSsh ==
  s.lastAction \in {
    "load-recovery", "recover-ledger-visibility", "recover-failure-visibility",
    "retry-rejected-admission", "acquire-owner"
  } => s.connected \subseteq s.previousConnected

=============================================================================
