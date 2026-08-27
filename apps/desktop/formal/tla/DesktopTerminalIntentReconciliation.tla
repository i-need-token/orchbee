--------------- MODULE DesktopTerminalIntentReconciliation ---------------
EXTENDS FiniteSets, Naturals, Sequences

Presentations == {"presentation-a", "presentation-b"}
Phases == {"none", "durable", "dispatched", "published"}
Revisions == 0..2

VARIABLES revision, phase, publicationOrder

vars == <<revision, phase, publicationOrder>>

AllPublished == \A presentation \in Presentations: phase[presentation] = "published"

Init ==
  /\ revision = 0
  /\ phase = [presentation \in Presentations |-> "none"]
  /\ publicationOrder = <<>>

CommitBatch ==
  /\ revision = 0
  /\ \A presentation \in Presentations: phase[presentation] = "none"
  /\ revision' = 1
  /\ phase' = [presentation \in Presentations |-> "durable"]
  /\ UNCHANGED publicationOrder

DispatchBatch ==
  /\ revision = 1
  /\ \A presentation \in Presentations: phase[presentation] = "durable"
  /\ phase' = [presentation \in Presentations |-> "dispatched"]
  /\ UNCHANGED <<revision, publicationOrder>>

CompleteAndPublish(presentation) ==
  /\ revision = 1
  /\ phase[presentation] = "dispatched"
  /\ phase' = [phase EXCEPT ![presentation] = "published"]
  /\ publicationOrder' = Append(publicationOrder, presentation)
  /\ UNCHANGED revision

CommitNext ==
  /\ revision = 1
  /\ AllPublished
  /\ revision' = 2
  /\ UNCHANGED <<phase, publicationOrder>>

Next ==
  \/ CommitBatch
  \/ DispatchBatch
  \/ \E presentation \in Presentations: CompleteAndPublish(presentation)
  \/ CommitNext

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(CommitBatch)
  /\ WF_vars(DispatchBatch)
  /\ \A presentation \in Presentations:
       WF_vars(CompleteAndPublish(presentation))
  /\ WF_vars(CommitNext)

TypeOK ==
  /\ revision \in Revisions
  /\ phase \in [Presentations -> Phases]
  /\ publicationOrder \in Seq(Presentations)

BatchDispatchIsAtomic ==
  (\E presentation \in Presentations: phase[presentation] = "durable") =>
    \A presentation \in Presentations: phase[presentation] = "durable"

PublishedPresentationWasDispatched ==
  (\E presentation \in Presentations: phase[presentation] = "published") =>
    \A presentation \in Presentations:
      phase[presentation] \in {"dispatched", "published"}

PublicationIdentityIsUnique ==
  /\ \A index \in DOMAIN publicationOrder:
       phase[publicationOrder[index]] = "published"
  /\ \A left, right \in DOMAIN publicationOrder:
       left # right => publicationOrder[left] # publicationOrder[right]

LaterCommitWaitsForBatch ==
  revision = 2 => AllPublished

BatchEventuallyPublishes ==
  revision = 1 ~> AllPublished

LaterCommitEventuallyAdmitted ==
  revision = 1 ~> revision = 2

=============================================================================
