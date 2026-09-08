-------------------- MODULE SearchProviderCoordination --------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

Providers == {"agents", "remote-actions"}
None == "none"
Phases == {"idle", "loading", "ready", "failed"}
MaxSession == 2
MaxRevision == 2
MaxAttempt == 1

VARIABLES
  open,
  session,
  revision,
  phase,
  attempt,
  runSession,
  runRevision,
  visible,
  visibleSession,
  visibleRevision,
  highlighted,
  lastRetryProvider,
  previousPhase,
  previousAttempt,
  previousVisible,
  previousVisibleSession,
  previousVisibleRevision

vars ==
  <<open, session, revision, phase, attempt, runSession, runRevision,
    visible, visibleSession, visibleRevision, highlighted,
    lastRetryProvider, previousPhase, previousAttempt, previousVisible,
    previousVisibleSession, previousVisibleRevision>>

RecordNonRetry ==
  /\ lastRetryProvider' = None
  /\ previousPhase' = phase
  /\ previousAttempt' = attempt
  /\ previousVisible' = visible
  /\ previousVisibleSession' = visibleSession
  /\ previousVisibleRevision' = visibleRevision

RecordRetry(provider) ==
  /\ lastRetryProvider' = provider
  /\ previousPhase' = phase
  /\ previousAttempt' = attempt
  /\ previousVisible' = visible
  /\ previousVisibleSession' = visibleSession
  /\ previousVisibleRevision' = visibleRevision

Init ==
  /\ open = FALSE
  /\ session = 0
  /\ revision = 0
  /\ phase = [provider \in Providers |-> "idle"]
  /\ attempt = [provider \in Providers |-> 0]
  /\ runSession = [provider \in Providers |-> 0]
  /\ runRevision = [provider \in Providers |-> 0]
  /\ visible = [provider \in Providers |-> FALSE]
  /\ visibleSession = [provider \in Providers |-> 0]
  /\ visibleRevision = [provider \in Providers |-> 0]
  /\ highlighted = None
  /\ lastRetryProvider = None
  /\ previousPhase = phase
  /\ previousAttempt = attempt
  /\ previousVisible = visible
  /\ previousVisibleSession = visibleSession
  /\ previousVisibleRevision = visibleRevision

Open ==
  /\ session < MaxSession
  /\ open' = TRUE
  /\ session' = session + 1
  /\ revision' = 0
  /\ phase' = [provider \in Providers |-> "loading"]
  /\ attempt' = [provider \in Providers |-> 0]
  /\ runSession' = [provider \in Providers |-> session']
  /\ runRevision' = [provider \in Providers |-> revision']
  /\ visible' = [provider \in Providers |-> FALSE]
  /\ visibleSession' = [provider \in Providers |-> 0]
  /\ visibleRevision' = [provider \in Providers |-> 0]
  /\ highlighted' = None
  /\ RecordNonRetry

Revise ==
  /\ open
  /\ revision < MaxRevision
  /\ revision' = revision + 1
  /\ phase' = [provider \in Providers |-> "loading"]
  /\ attempt' = [provider \in Providers |-> 0]
  /\ runSession' = [provider \in Providers |-> session]
  /\ runRevision' = [provider \in Providers |-> revision']
  /\ visible' = [provider \in Providers |-> FALSE]
  /\ visibleSession' = [provider \in Providers |-> 0]
  /\ visibleRevision' = [provider \in Providers |-> 0]
  /\ highlighted' = None
  /\ UNCHANGED <<open, session>>
  /\ RecordNonRetry

Close ==
  /\ open
  /\ open' = FALSE
  /\ phase' = [provider \in Providers |-> "idle"]
  /\ visible' = [provider \in Providers |-> FALSE]
  /\ visibleSession' = [provider \in Providers |-> 0]
  /\ visibleRevision' = [provider \in Providers |-> 0]
  /\ highlighted' = None
  /\ UNCHANGED <<session, revision, attempt, runSession, runRevision>>
  /\ RecordNonRetry

Highlight(provider) ==
  /\ open
  /\ visible[provider]
  /\ highlighted' = provider
  /\ UNCHANGED
       <<open, session, revision, phase, attempt, runSession, runRevision,
         visible, visibleSession, visibleRevision>>
  /\ RecordNonRetry

Retry(provider) ==
  /\ open
  /\ phase[provider] = "failed"
  /\ attempt[provider] < MaxAttempt
  /\ phase' = [phase EXCEPT ![provider] = "loading"]
  /\ attempt' = [attempt EXCEPT ![provider] = @ + 1]
  /\ runSession' = [runSession EXCEPT ![provider] = session]
  /\ runRevision' = [runRevision EXCEPT ![provider] = revision]
  /\ visible' = [visible EXCEPT ![provider] = FALSE]
  /\ visibleSession' = [visibleSession EXCEPT ![provider] = 0]
  /\ visibleRevision' = [visibleRevision EXCEPT ![provider] = 0]
  /\ highlighted' = IF highlighted = provider THEN None ELSE highlighted
  /\ UNCHANGED <<open, session, revision>>
  /\ RecordRetry(provider)

ProviderModelChanged(provider) ==
  /\ open
  /\ attempt[provider] < MaxAttempt
  /\ phase' = [phase EXCEPT ![provider] = "loading"]
  /\ attempt' = [attempt EXCEPT ![provider] = @ + 1]
  /\ runSession' = [runSession EXCEPT ![provider] = session]
  /\ runRevision' = [runRevision EXCEPT ![provider] = revision]
  /\ visible' = [visible EXCEPT ![provider] = FALSE]
  /\ visibleSession' = [visibleSession EXCEPT ![provider] = 0]
  /\ visibleRevision' = [visibleRevision EXCEPT ![provider] = 0]
  /\ highlighted' = IF highlighted = provider THEN None ELSE highlighted
  /\ UNCHANGED <<open, session, revision>>
  /\ RecordNonRetry

IsCurrent(provider, candidateSession, candidateRevision, candidateAttempt) ==
  /\ open
  /\ phase[provider] = "loading"
  /\ runSession[provider] = candidateSession
  /\ runRevision[provider] = candidateRevision
  /\ attempt[provider] = candidateAttempt
  /\ candidateSession = session
  /\ candidateRevision = revision

ProviderCompletes(provider, candidateSession, candidateRevision, candidateAttempt) ==
  /\ candidateSession \in 1..MaxSession
  /\ candidateRevision \in 0..MaxRevision
  /\ candidateAttempt \in 0..MaxAttempt
  /\ IF IsCurrent(provider, candidateSession, candidateRevision, candidateAttempt)
     THEN
       /\ phase' = [phase EXCEPT ![provider] = "ready"]
       /\ visible' = [visible EXCEPT ![provider] = TRUE]
       /\ visibleSession' = [visibleSession EXCEPT ![provider] = candidateSession]
       /\ visibleRevision' = [visibleRevision EXCEPT ![provider] = candidateRevision]
     ELSE
       /\ UNCHANGED <<phase, visible, visibleSession, visibleRevision>>
  /\ UNCHANGED
       <<open, session, revision, attempt, runSession, runRevision, highlighted>>
  /\ RecordNonRetry

ProviderFails(provider, candidateSession, candidateRevision, candidateAttempt) ==
  /\ candidateSession \in 1..MaxSession
  /\ candidateRevision \in 0..MaxRevision
  /\ candidateAttempt \in 0..MaxAttempt
  /\ IF IsCurrent(provider, candidateSession, candidateRevision, candidateAttempt)
     THEN
       /\ phase' = [phase EXCEPT ![provider] = "failed"]
       /\ visible' = [visible EXCEPT ![provider] = FALSE]
       /\ visibleSession' = [visibleSession EXCEPT ![provider] = 0]
       /\ visibleRevision' = [visibleRevision EXCEPT ![provider] = 0]
       /\ highlighted' = IF highlighted = provider THEN None ELSE highlighted
     ELSE
       /\ UNCHANGED <<phase, visible, visibleSession, visibleRevision, highlighted>>
  /\ UNCHANGED <<open, session, revision, attempt, runSession, runRevision>>
  /\ RecordNonRetry

SettleCurrent(provider) ==
  \/ ProviderCompletes(provider, session, revision, attempt[provider])
  \/ ProviderFails(provider, session, revision, attempt[provider])

Next ==
  \/ Open
  \/ Revise
  \/ Close
  \/ \E provider \in Providers: Highlight(provider)
  \/ \E provider \in Providers: Retry(provider)
  \/ \E provider \in Providers: ProviderModelChanged(provider)
  \/ \E provider \in Providers,
       candidateSession \in 1..MaxSession,
       candidateRevision \in 0..MaxRevision,
       candidateAttempt \in 0..MaxAttempt:
       ProviderCompletes(provider, candidateSession, candidateRevision, candidateAttempt)
  \/ \E provider \in Providers,
       candidateSession \in 1..MaxSession,
       candidateRevision \in 0..MaxRevision,
       candidateAttempt \in 0..MaxAttempt:
       ProviderFails(provider, candidateSession, candidateRevision, candidateAttempt)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A provider \in Providers: WF_vars(SettleCurrent(provider))

TypeOK ==
  /\ open \in BOOLEAN
  /\ session \in 0..MaxSession
  /\ revision \in 0..MaxRevision
  /\ phase \in [Providers -> Phases]
  /\ attempt \in [Providers -> 0..MaxAttempt]
  /\ runSession \in [Providers -> 0..MaxSession]
  /\ runRevision \in [Providers -> 0..MaxRevision]
  /\ visible \in [Providers -> BOOLEAN]
  /\ visibleSession \in [Providers -> 0..MaxSession]
  /\ visibleRevision \in [Providers -> 0..MaxRevision]
  /\ highlighted \in Providers \cup {None}
  /\ lastRetryProvider \in Providers \cup {None}
  /\ previousPhase \in [Providers -> Phases]
  /\ previousAttempt \in [Providers -> 0..MaxAttempt]
  /\ previousVisible \in [Providers -> BOOLEAN]
  /\ previousVisibleSession \in [Providers -> 0..MaxSession]
  /\ previousVisibleRevision \in [Providers -> 0..MaxRevision]

VisibleResultsAreCurrent ==
  \A provider \in Providers:
    visible[provider] =>
      /\ open
      /\ phase[provider] = "ready"
      /\ visibleSession[provider] = session
      /\ visibleRevision[provider] = revision

ClosedHasNoVisibleProviderResults ==
  ~open => \A provider \in Providers: ~visible[provider]

HighlightIsVisibleOrNone ==
  highlighted = None \/ visible[highlighted]

ProviderProjectionIsCurrent ==
  \A provider \in Providers:
    phase[provider] \in {"loading", "ready", "failed"} =>
      /\ open
      /\ runSession[provider] = session
      /\ runRevision[provider] = revision

RetryDoesNotInvalidateOtherProviders ==
  lastRetryProvider # None =>
    \A provider \in Providers \ {lastRetryProvider}:
      /\ phase[provider] = previousPhase[provider]
      /\ attempt[provider] = previousAttempt[provider]
      /\ visible[provider] = previousVisible[provider]
      /\ visibleSession[provider] = previousVisibleSession[provider]
      /\ visibleRevision[provider] = previousVisibleRevision[provider]

CurrentRunsEventuallySettle ==
  \A provider \in Providers:
    (open /\ phase[provider] = "loading")
      ~> (~open \/ phase[provider] # "loading")

=============================================================================
