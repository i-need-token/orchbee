---------------- MODULE WorkbenchDocumentProposalAcknowledgement ----------------
EXTENDS Naturals

Values == {"initial", "candidate", "later", "divergent"}
Phases == {"ready", "active", "exact-published", "divergent-published"}
Revisions == 0..2

VARIABLES revision, canonical, projection, candidate, phase,
  exactLaterPreserved, divergentApplied

vars == <<revision, canonical, projection, candidate, phase,
  exactLaterPreserved, divergentApplied>>

Init ==
  /\ revision = 0
  /\ canonical = "initial"
  /\ projection = "initial"
  /\ candidate = "initial"
  /\ phase = "ready"
  /\ exactLaterPreserved = FALSE
  /\ divergentApplied = FALSE

BeginFirstProposal ==
  /\ revision = 0
  /\ phase = "ready"
  /\ projection = "initial"
  /\ candidate' = "candidate"
  /\ projection' = "candidate"
  /\ phase' = "active"
  /\ exactLaterPreserved' = FALSE
  /\ divergentApplied' = FALSE
  /\ UNCHANGED <<revision, canonical>>

LocalEditBeforeExactPublication ==
  /\ revision = 0
  /\ phase = "active"
  /\ projection = "candidate"
  /\ projection' = "later"
  /\ exactLaterPreserved' = FALSE
  /\ divergentApplied' = FALSE
  /\ UNCHANGED <<revision, canonical, candidate, phase>>

LocalEditAfterExactPublication ==
  /\ revision = 1
  /\ phase = "exact-published"
  /\ canonical = candidate
  /\ projection = "candidate"
  /\ projection' = "later"
  /\ exactLaterPreserved' = TRUE
  /\ divergentApplied' = FALSE
  /\ UNCHANGED <<revision, canonical, candidate, phase>>

PublishExact ==
  /\ phase = "active"
  /\ revision < 2
  /\ revision' = revision + 1
  /\ canonical' = candidate
  /\ phase' = "exact-published"
  /\ exactLaterPreserved' = (projection # candidate)
  /\ divergentApplied' = FALSE
  /\ UNCHANGED <<projection, candidate>>

PublishDivergent ==
  /\ revision = 0
  /\ phase = "active"
  /\ revision' = 1
  /\ canonical' = "divergent"
  /\ projection' = "divergent"
  /\ phase' = "divergent-published"
  /\ exactLaterPreserved' = FALSE
  /\ divergentApplied' = TRUE
  /\ UNCHANGED candidate

SettleExactWithoutPublication ==
  /\ phase = "active"
  /\ revision < 2
  /\ revision' = revision + 1
  /\ canonical' = candidate
  /\ phase' = "ready"
  /\ exactLaterPreserved' = (projection # candidate)
  /\ divergentApplied' = FALSE
  /\ UNCHANGED <<projection, candidate>>

SettleStaleWithoutPublication ==
  /\ revision = 0
  /\ phase = "active"
  /\ revision' = 1
  /\ canonical' = "divergent"
  /\ projection' = "divergent"
  /\ phase' = "ready"
  /\ exactLaterPreserved' = FALSE
  /\ divergentApplied' = TRUE
  /\ UNCHANGED candidate

SettlePublished ==
  /\ phase \in {"exact-published", "divergent-published"}
  /\ phase' = "ready"
  /\ UNCHANGED <<revision, canonical, projection, candidate,
       exactLaterPreserved, divergentApplied>>

BeginNextProposal ==
  /\ revision = 1
  /\ phase = "ready"
  /\ canonical = "candidate"
  /\ projection = "later"
  /\ candidate' = projection
  /\ phase' = "active"
  /\ exactLaterPreserved' = FALSE
  /\ divergentApplied' = FALSE
  /\ UNCHANGED <<revision, canonical, projection>>

Next ==
  \/ BeginFirstProposal
  \/ LocalEditBeforeExactPublication
  \/ LocalEditAfterExactPublication
  \/ PublishExact
  \/ PublishDivergent
  \/ SettleExactWithoutPublication
  \/ SettleStaleWithoutPublication
  \/ SettlePublished
  \/ BeginNextProposal

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(BeginFirstProposal)
  /\ WF_vars(PublishExact)
  /\ WF_vars(SettlePublished)
  /\ WF_vars(BeginNextProposal)

TypeOK ==
  /\ revision \in Revisions
  /\ canonical \in Values
  /\ projection \in Values
  /\ candidate \in Values
  /\ phase \in Phases
  /\ exactLaterPreserved \in BOOLEAN
  /\ divergentApplied \in BOOLEAN

ExactAcknowledgementPreservesLaterProjection ==
  exactLaterPreserved => projection = "later"

DivergentCanonicalReplacesProjection ==
  divergentApplied => projection = canonical

PublishedCandidateMatchesCanonical ==
  phase = "exact-published" => candidate = canonical

LaterIntentEventuallyReproposed ==
  exactLaterPreserved ~>
    (revision = 1 /\ phase = "active" /\ candidate = "later") \/ revision = 2

NextProposalEventuallyAcknowledged ==
  (revision = 1 /\ phase = "active") ~> revision = 2

=============================================================================
