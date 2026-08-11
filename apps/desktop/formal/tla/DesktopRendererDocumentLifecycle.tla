---------------- MODULE DesktopRendererDocumentLifecycle ----------------
EXTENDS Naturals, TLC

CONSTANT MaxGeneration

NoGeneration == MaxGeneration + 1
NavigationKinds == {
  "main-document",
  "main-same-document",
  "subframe-document",
  "subframe-same-document"
}
Actions == {
  "init",
  "start-document",
  "ignored-navigation",
  "finish-document"
}

VARIABLES
  documentGeneration,
  pendingGeneration,
  controlGeneration,
  lastAction,
  lastNavigationKind,
  previousDocumentGeneration,
  previousPendingGeneration,
  previousControlGeneration

vars == <<
  documentGeneration,
  pendingGeneration,
  controlGeneration,
  lastAction,
  lastNavigationKind,
  previousDocumentGeneration,
  previousPendingGeneration,
  previousControlGeneration
>>

Record(action, navigationKind) ==
  /\ lastAction' = action
  /\ lastNavigationKind' = navigationKind
  /\ previousDocumentGeneration' = documentGeneration
  /\ previousPendingGeneration' = pendingGeneration
  /\ previousControlGeneration' = controlGeneration

Init ==
  /\ documentGeneration = 0
  /\ pendingGeneration = NoGeneration
  /\ controlGeneration = NoGeneration
  /\ lastAction = "init"
  /\ lastNavigationKind = "main-document"
  /\ previousDocumentGeneration = documentGeneration
  /\ previousPendingGeneration = pendingGeneration
  /\ previousControlGeneration = controlGeneration

StartDocumentNavigation ==
  /\ documentGeneration < MaxGeneration
  /\ documentGeneration' = documentGeneration + 1
  /\ pendingGeneration' = documentGeneration + 1
  /\ controlGeneration' = NoGeneration
  /\ Record("start-document", "main-document")

StartIgnoredNavigation(navigationKind) ==
  /\ navigationKind \in NavigationKinds \ {"main-document"}
  /\ UNCHANGED <<documentGeneration, pendingGeneration, controlGeneration>>
  /\ Record("ignored-navigation", navigationKind)

FinishDocument ==
  /\ pendingGeneration # NoGeneration
  /\ pendingGeneration' = NoGeneration
  /\ controlGeneration' = documentGeneration
  /\ UNCHANGED documentGeneration
  /\ Record("finish-document", "main-document")

FinishDocumentWithoutObservedStart ==
  /\ pendingGeneration = NoGeneration
  /\ documentGeneration < MaxGeneration
  /\ documentGeneration' = documentGeneration + 1
  /\ pendingGeneration' = NoGeneration
  /\ controlGeneration' = documentGeneration + 1
  /\ Record("finish-document", "main-document")

Next ==
  \/ StartDocumentNavigation
  \/ \E navigationKind \in NavigationKinds \ {"main-document"}:
       StartIgnoredNavigation(navigationKind)
  \/ FinishDocument
  \/ FinishDocumentWithoutObservedStart

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(FinishDocument)

TypeOK ==
  /\ documentGeneration \in 0..MaxGeneration
  /\ pendingGeneration \in 0..NoGeneration
  /\ controlGeneration \in 0..NoGeneration
  /\ lastAction \in Actions
  /\ lastNavigationKind \in NavigationKinds
  /\ previousDocumentGeneration \in 0..MaxGeneration
  /\ previousPendingGeneration \in 0..NoGeneration
  /\ previousControlGeneration \in 0..NoGeneration

ControlsBelongToCurrentDocument ==
  controlGeneration = NoGeneration \/ controlGeneration = documentGeneration

PendingDocumentOwnsNoControls ==
  pendingGeneration = NoGeneration \/
    /\ pendingGeneration = documentGeneration
    /\ controlGeneration = NoGeneration

IgnoredNavigationPreservesDocument ==
  lastAction = "ignored-navigation" =>
    /\ documentGeneration = previousDocumentGeneration
    /\ pendingGeneration = previousPendingGeneration
    /\ controlGeneration = previousControlGeneration

DocumentStartRetiresPreviousControls ==
  lastAction = "start-document" =>
    /\ pendingGeneration = documentGeneration
    /\ controlGeneration = NoGeneration

DocumentFinishPublishesCurrentControls ==
  lastAction = "finish-document" =>
    /\ pendingGeneration = NoGeneration
    /\ controlGeneration = documentGeneration

PendingDocumentEventuallyFinishes ==
  pendingGeneration # NoGeneration ~>
    /\ pendingGeneration = NoGeneration
    /\ controlGeneration = documentGeneration

=============================================================================
