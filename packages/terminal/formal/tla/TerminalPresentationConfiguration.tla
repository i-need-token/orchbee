---------------- MODULE TerminalPresentationConfiguration ----------------
EXTENDS Naturals, TLC

Requests == {1, 2}
Fields == {
  "fontFamily",
  "fontSize",
  "fontWeight",
  "fontWeightBold",
  "fontLigatures",
  "fontMetricModifiers",
  "terminalPadding",
  "paddingBalance",
  "allowTransparency",
  "backgroundOpacity",
  "defaultColorOverride"
}
PreparationPhases == {"absent", "in-flight", "ready", "failed"}
Outcomes == {"absent", "pending", "ready", "completed", "cancelled", "failed"}

VARIABLES
  preparation,
  outcome,
  eligible,
  latest,
  committed,
  published,
  disposed,
  lastAction,
  previousPreparation,
  previousOutcome,
  previousEligible,
  previousLatest,
  previousCommitted,
  previousPublished

vars == <<preparation, outcome, eligible, latest, committed, published,
  disposed, lastAction, previousPreparation, previousOutcome,
  previousEligible, previousLatest, previousCommitted, previousPublished>>

Record(action) ==
  /\ lastAction' = action
  /\ previousPreparation' = preparation
  /\ previousOutcome' = outcome
  /\ previousEligible' = eligible
  /\ previousLatest' = latest
  /\ previousCommitted' = committed
  /\ previousPublished' = published

Init ==
  /\ preparation = [request \in Requests |-> "absent"]
  /\ outcome = [request \in Requests |-> "absent"]
  /\ eligible = [request \in Requests |-> FALSE]
  /\ latest = 0
  /\ committed = 0
  /\ published = [field \in Fields |-> 0]
  /\ disposed = FALSE
  /\ lastAction = "init"
  /\ previousPreparation = preparation
  /\ previousOutcome = outcome
  /\ previousEligible = eligible
  /\ previousLatest = latest
  /\ previousCommitted = committed
  /\ previousPublished = published

CommitEligible(request) ==
  /\ ~disposed
  /\ request = latest
  /\ eligible[request]

CommitReady(request) ==
  /\ CommitEligible(request)
  /\ preparation[request] = "ready"
  /\ outcome[request] = "ready"

Request(request) ==
  /\ ~disposed
  /\ request = latest + 1
  /\ request \in Requests
  /\ preparation[request] = "absent"
  /\ preparation' = [preparation EXCEPT ![request] = "in-flight"]
  /\ outcome' = [candidate \in Requests |->
       IF candidate = request
       THEN "pending"
       ELSE IF eligible[candidate] /\ outcome[candidate] \in {"pending", "ready"}
       THEN "cancelled"
       ELSE outcome[candidate]]
  /\ eligible' = [candidate \in Requests |-> candidate = request]
  /\ latest' = request
  /\ UNCHANGED <<committed, published, disposed>>
  /\ Record(IF latest = 0 THEN "request" ELSE "supersede")

PreparationCompletes(request) ==
  /\ preparation[request] = "in-flight"
  /\ preparation' = [preparation EXCEPT ![request] = "ready"]
  /\ outcome' = [outcome EXCEPT ![request] =
       IF CommitEligible(request) THEN "ready" ELSE @]
  /\ eligible' = [eligible EXCEPT ![request] = CommitEligible(request)]
  /\ UNCHANGED <<latest, committed, published, disposed>>
  /\ Record(IF CommitEligible(request) THEN "prepared" ELSE "retired-completion")

PreparationFails(request) ==
  /\ preparation[request] = "in-flight"
  /\ preparation' = [preparation EXCEPT ![request] = "failed"]
  /\ outcome' = [outcome EXCEPT ![request] =
       IF CommitEligible(request) THEN "failed" ELSE @]
  /\ eligible' = [eligible EXCEPT ![request] = FALSE]
  /\ UNCHANGED <<latest, committed, published, disposed>>
  /\ Record(IF CommitEligible(request) THEN "preparation-failed" ELSE "retired-failure")

Commit(request) ==
  /\ CommitReady(request)
  /\ outcome' = [outcome EXCEPT ![request] = "completed"]
  /\ eligible' = [eligible EXCEPT ![request] = FALSE]
  /\ committed' = request
  /\ published' = [field \in Fields |-> request]
  /\ UNCHANGED <<preparation, latest, disposed>>
  /\ Record("commit")

Dispose ==
  /\ ~disposed
  /\ disposed' = TRUE
  /\ eligible' = [request \in Requests |-> FALSE]
  /\ outcome' = [request \in Requests |->
       IF eligible[request]
         /\ outcome[request] \in {"pending", "ready"}
       THEN "cancelled"
       ELSE outcome[request]]
  /\ UNCHANGED <<preparation, latest, committed, published>>
  /\ Record("dispose")

Next ==
  \/ \E request \in Requests: Request(request)
  \/ \E request \in Requests: PreparationCompletes(request)
  \/ \E request \in Requests: PreparationFails(request)
  \/ \E request \in Requests: Commit(request)
  \/ Dispose

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A request \in Requests: WF_vars(Commit(request))

TypeOK ==
  /\ preparation \in [Requests -> PreparationPhases]
  /\ outcome \in [Requests -> Outcomes]
  /\ eligible \in [Requests -> BOOLEAN]
  /\ latest \in 0..2
  /\ committed \in 0..2
  /\ published \in [Fields -> 0..2]
  /\ disposed \in BOOLEAN
  /\ lastAction \in {
       "init", "request", "supersede", "prepared", "commit",
       "retired-completion", "preparation-failed", "retired-failure", "dispose"
     }
  /\ previousPreparation \in [Requests -> PreparationPhases]
  /\ previousOutcome \in [Requests -> Outcomes]
  /\ previousEligible \in [Requests -> BOOLEAN]
  /\ previousLatest \in 0..2
  /\ previousCommitted \in 0..2
  /\ previousPublished \in [Fields -> 0..2]

EligibilityConsistent ==
  \A request \in Requests:
    eligible[request] =>
      /\ ~disposed
      /\ request = latest
      /\ preparation[request] \in {"in-flight", "ready"}
      /\ outcome[request] \in {"pending", "ready"}

NoPartialPublication ==
  \A field \in Fields: published[field] = committed

NoStaleCommit ==
  lastAction = "commit" => committed = previousLatest

CancelledCannotPublish ==
  \A request \in Requests: outcome[request] = "cancelled" => committed # request

CancellationPreservesPublication ==
  lastAction \in {"retired-completion", "retired-failure", "dispose"} =>
    committed = previousCommitted /\ published = previousPublished

SupersessionPreservesPublication ==
  lastAction = "supersede" =>
    committed = previousCommitted /\ published = previousPublished

FailurePreservesPublication ==
  lastAction \in {"preparation-failed", "retired-failure"} =>
    committed = previousCommitted /\ published = previousPublished

SupersededCallerIsCancelled ==
  lastAction = "supersede" =>
    \A request \in Requests:
      previousEligible[request] => outcome[request] = "cancelled"

DisposedCallerIsCancelled ==
  lastAction = "dispose" =>
    \A request \in Requests:
      previousEligible[request] => outcome[request] = "cancelled"

LatePreparationCannotChangeCallerOutcome ==
  lastAction \in {"retired-completion", "retired-failure"} =>
    /\ outcome = previousOutcome
    /\ eligible = previousEligible
    /\ committed = previousCommitted
    /\ published = previousPublished

PublicationChangesOnlyOnCommit ==
  (committed # previousCommitted \/ published # previousPublished) =>
    lastAction = "commit"

LatestReadyEventuallyResolves ==
  \A request \in Requests:
    [](CommitReady(request)
      ~> (committed = request \/ latest # request \/ disposed))

=============================================================================
