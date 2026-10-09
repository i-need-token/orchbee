-------------------------- MODULE NativeQuickWorkspace --------------------------
EXTENDS Naturals, TLC

Phases == {"Idle", "Prepare", "Edit", "ProjectSubmit", "ProjectWait", "ProjectRead",
           "LaunchSubmit", "LaunchWait", "LaunchRead", "Attach", "Ready", "Stopped", "Unknown"}
Modes == {"zero", "one", "many", "source", "target"}
ReturnStages == {"none", "submitted", "entering", "consumed", "cancelled"}
Surfaces == {"compact", "docked", "hidden", "other"}
VARIABLES phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame, proof, coverage
vars == <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame, proof, coverage>>

Init == /\ phase = "Idle" /\ mode \in Modes /\ revision = 0 /\ captured = 0
        /\ peerWritten = FALSE /\ openings = 0 /\ token = 0 /\ drawn = 0
        /\ fieldChanges = 0 /\ selected = FALSE /\ valid = TRUE /\ confirmed = FALSE
        /\ acceptedDraw = 0 /\ issuedP = 0 /\ issuedW = 0
        /\ knownP = FALSE /\ knownW = FALSE /\ started = FALSE
        /\ freshTarget = FALSE /\ firstFrame = FALSE /\ coverage = "Init"
        /\ \E surface \in Surfaces:
             proof = [source |-> TRUE, target |-> TRUE, accepted |-> FALSE,
                     project |-> IF mode = "target" THEN 2 ELSE 1, launched |-> 0,
                     returning |-> "none", eligible |-> FALSE, surface |-> surface,
                     newer |-> FALSE, returns |-> 0, reopened |-> FALSE]

Open == /\ phase = "Idle" /\ openings < 2
        /\ phase' = "Prepare" /\ openings' = openings + 1 /\ token' = token + 1
        /\ captured' = revision /\ drawn' = 0 /\ coverage' = "Open"
        /\ proof' = [proof EXCEPT !.source = TRUE, !.target = TRUE,
                      !.eligible = mode = "target" /\ proof.surface = "compact"]
        /\ UNCHANGED <<mode, revision, peerWritten, fieldChanges, selected, valid,
              confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
Prepared == /\ phase = "Prepare" /\ captured = revision
            /\ (mode # "target" \/ (proof.source /\ proof.target))
            /\ phase' = "Edit" /\ token' = token + 1 /\ selected' = (mode # "many")
            /\ coverage' = "Prepared"
            /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, drawn, fieldChanges,
                  valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
PrepareRefused == /\ phase = "Prepare" /\ phase' = "Idle" /\ coverage' = "PrepareRefused"
                  /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                      selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
Cancel == /\ phase \in {"Prepare", "Edit"} /\ phase' = "Idle" /\ drawn' = 0
          /\ coverage' = "Cancel"
          /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, fieldChanges,
              selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
LatePreparation == /\ phase = "Idle" /\ openings > 0 /\ coverage' = "LatePreparation"
                   /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
                        fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
Draw == /\ phase = "Edit" /\ drawn' = token /\ coverage' = "Draw"
        /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, fieldChanges,
              selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
ChangeField == /\ phase = "Edit" /\ fieldChanges < 2 /\ fieldChanges' = fieldChanges + 1
               /\ token' = token + 1 /\ drawn' = 0 /\ coverage' = "ChangeField"
               /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, selected,
                     valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
SelectProject == /\ phase = "Edit" /\ ~selected /\ selected' = TRUE /\ token' = token + 1
                 /\ drawn' = 0 /\ coverage' = "SelectProject"
                 /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, fieldChanges,
                       valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
EditValidity == /\ phase = "Edit" /\ valid' \in BOOLEAN /\ coverage' = "EditValidity"
                /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
                      fieldChanges, selected, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
Confirm == /\ phase = "Edit" /\ valid /\ selected /\ drawn = token
           /\ (mode # "target" \/ (proof.source /\ proof.target))
           /\ confirmed' = TRUE /\ acceptedDraw' = drawn
           /\ phase' = IF mode = "zero" THEN "ProjectSubmit" ELSE "LaunchSubmit"
           /\ coverage' = "Confirm"
           /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                 fieldChanges, selected, valid, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
SubmitProject == /\ phase = "ProjectSubmit" /\ issuedP = 0 /\ phase' = "ProjectWait"
                 /\ issuedP' = issuedP + 1 /\ coverage' = "SubmitProject"
                 /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                       fieldChanges, selected, valid, confirmed, acceptedDraw, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
ProjectKnown == /\ phase = "ProjectWait" /\ captured = revision /\ revision' = revision + 1
                /\ knownP' = TRUE /\ phase' = "ProjectRead" /\ coverage' = "ProjectKnown"
                /\ UNCHANGED <<mode, captured, peerWritten, openings, token, drawn, fieldChanges,
                      selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownW, started, freshTarget, firstFrame, proof>>
ProjectRead == /\ phase = "ProjectRead" /\ captured' = revision /\ phase' = "LaunchSubmit"
               /\ coverage' = "ProjectRead"
               /\ UNCHANGED <<mode, revision, peerWritten, openings, token, drawn, fieldChanges,
                     selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
CancelContinuation == /\ phase = "ProjectRead" /\ phase' = "Stopped" /\ coverage' = "CancelContinuation"
                      /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                            fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
SubmitLaunch == /\ phase = "LaunchSubmit" /\ issuedW = 0 /\ phase' = "LaunchWait"
                /\ (mode # "target" \/ (proof.source /\ proof.target))
                /\ proof' = [proof EXCEPT !.launched = proof.project,
                                         !.returning = IF mode = "target" THEN "submitted" ELSE "none",
                                         !.accepted = proof.source /\ proof.target]
                /\ issuedW' = issuedW + 1 /\ coverage' = "SubmitLaunch"
                /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                      selected, valid, confirmed, acceptedDraw, issuedP, knownP, knownW, started, freshTarget, firstFrame>>
LaunchKnown == /\ phase = "LaunchWait" /\ captured = revision /\ revision' = revision + 1
               /\ knownW' = TRUE /\ started' \in BOOLEAN /\ phase' = "LaunchRead"
               /\ coverage' = "LaunchKnown"
               /\ UNCHANGED <<mode, captured, peerWritten, openings, token, drawn, fieldChanges,
                     selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, freshTarget, firstFrame, proof>>
Reject == /\ phase \in {"ProjectWait", "LaunchWait"} /\ phase' = "Stopped" /\ coverage' = "Reject"
          /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
Unknown == /\ phase \in {"ProjectWait", "LaunchWait"} /\ phase' = "Unknown" /\ coverage' = "Unknown"
           /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                 selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
ReadFailure == /\ phase \in {"ProjectRead", "LaunchRead"} /\ phase' = "Stopped" /\ coverage' = "ReadFailure"
               /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                     selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>
LaunchRead == /\ phase = "LaunchRead" /\ started /\ freshTarget' = TRUE /\ phase' = "Attach"
              /\ coverage' = "LaunchRead"
              /\ proof' = [proof EXCEPT !.returning =
                    IF proof.returning = "submitted" THEN "entering" ELSE @]
              /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                    selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, firstFrame>>
FirstFrame == /\ phase = "Attach" /\ firstFrame' = TRUE /\ phase' = "Ready" /\ coverage' = "FirstFrame"
              /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                    selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, proof>>
PeerWrite == /\ ~peerWritten /\ peerWritten' = TRUE /\ revision' = revision + 1 /\ coverage' = "PeerWrite"
             /\ UNCHANGED <<phase, mode, captured, openings, token, drawn, fieldChanges, selected, valid,
                   confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame, proof>>

SourceChanges == /\ mode = "target" /\ phase \in {"Prepare", "Edit", "LaunchSubmit"} /\ proof.source
                 /\ proof' = [proof EXCEPT !.source = FALSE] /\ coverage' = "SourceChanges"
                 /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
                      fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
                      knownP, knownW, started, freshTarget, firstFrame>>
TargetChanges == /\ mode = "target" /\ phase \in {"Prepare", "Edit", "LaunchSubmit"} /\ proof.target
                 /\ proof' = [proof EXCEPT !.target = FALSE] /\ coverage' = "TargetChanges"
                 /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
                      fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
                      knownP, knownW, started, freshTarget, firstFrame>>
ProofRefused == /\ mode = "target" /\ phase \in {"Prepare", "Edit", "LaunchSubmit"}
                /\ ~(proof.source /\ proof.target) /\ phase' = "Stopped" /\ coverage' = "ProofRefused"
                /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                     fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
                     knownP, knownW, started, freshTarget, firstFrame, proof>>
\* Exact member/desired/presentation checks refine freshTarget. New intent includes
\* refused focus, Project-row/disclosure focus, hide/reopen, native capture and a
\* newer operation; it wins even when Ready is already queued. It never rewrites
\* the accepted launch evidence.
NewerReturnIntent ==
    /\ mode = "target" /\ phase \in {"LaunchWait", "LaunchRead", "Attach", "Ready"}
    /\ ~proof.newer /\ proof.returning \in {"submitted", "entering"}
    /\ \E surface \in Surfaces:
         proof' = [proof EXCEPT !.newer = TRUE, !.eligible = FALSE,
                   !.returning = "cancelled", !.surface = surface]
    /\ coverage' = "NewerReturnIntent"
    /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame>>
ReturnToTerminal ==
    /\ phase = "Ready" /\ mode = "target" /\ proof.returning = "entering"
    /\ proof' = [proof EXCEPT !.returning = "consumed", !.eligible = FALSE,
          !.returns = @+1,
          !.surface = IF proof.eligible /\ proof.surface = "compact" THEN "hidden" ELSE @]
    /\ coverage' = "ReturnToTerminal"
    /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame>>
ReopenAfterReturn ==
    /\ proof.returning \in {"consumed", "cancelled"} /\ ~proof.reopened
    /\ proof' = [proof EXCEPT !.surface = "compact", !.reopened = TRUE]
    /\ coverage' = "ReopenAfterReturn"
    /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame>>
LateReturnCompletion ==
    /\ proof.returning \in {"consumed", "cancelled"}
    /\ coverage' = "LateReturnCompletion"
    /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame, proof>>
Next == Open \/ Prepared \/ PrepareRefused \/ Cancel \/ LatePreparation \/ Draw \/ ChangeField
        \/ SelectProject \/ EditValidity \/ Confirm \/ SubmitProject \/ ProjectKnown \/ ProjectRead
        \/ CancelContinuation \/ SubmitLaunch \/ LaunchKnown \/ Reject \/ Unknown \/ ReadFailure
        \/ LaunchRead \/ FirstFrame \/ PeerWrite \/ SourceChanges \/ TargetChanges \/ ProofRefused
        \/ NewerReturnIntent \/ ReturnToTerminal \/ ReopenAfterReturn \/ LateReturnCompletion
TypeOK == /\ phase \in Phases /\ mode \in Modes /\ revision \in 0..3 /\ captured \in 0..3
          /\ openings \in 0..2 /\ token \in 0..8 /\ drawn \in 0..8 /\ acceptedDraw \in 0..8
          /\ fieldChanges \in 0..2
          /\ proof \in [source: BOOLEAN, target: BOOLEAN, accepted: BOOLEAN, project: 1..2, launched: 0..2,
                returning: ReturnStages, eligible: BOOLEAN, surface: Surfaces,
                newer: BOOLEAN, returns: 0..1, reopened: BOOLEAN]
          /\ <<peerWritten, selected, valid, confirmed, knownP, knownW,
                started, freshTarget, firstFrame>> \in BOOLEAN \X BOOLEAN \X BOOLEAN \X BOOLEAN
                  \X BOOLEAN \X BOOLEAN \X BOOLEAN \X BOOLEAN \X BOOLEAN
AtMostOnce == /\ issuedP \in 0..1 /\ issuedW \in 0..1
ConfirmationBeforeMutation == (issuedP > 0 \/ issuedW > 0) => (confirmed /\ acceptedDraw > 0)
WorkspaceRequiresProject == issuedW > 0 => (mode # "zero" \/ knownP)
CancelledPreparationIsReadOnly == phase = "Idle" => (~confirmed /\ issuedP = 0 /\ issuedW = 0)
NoFalseReady == phase = "Ready" => (knownW /\ started /\ freshTarget /\ firstFrame)
ExactTargetProject == (mode = "target" /\ issuedW > 0) => proof.launched = 2
TargetProofBeforeLaunch == (mode = "target" /\ issuedW > 0) => proof.accepted
ExactReadyBeforeReturn == proof.returns > 0 =>
    (mode = "target" /\ knownW /\ started /\ freshTarget /\ firstFrame)
NewerIntentPreventsReturn == proof.newer => proof.returns = 0
ReturnPreservesOtherSurfaces == [][ReturnToTerminal /\ proof.surface # "compact" =>
    proof'.surface = proof.surface]_vars
ReturnNeverRearms == [][proof.returning \in {"consumed", "cancelled"} =>
    proof'.returning = proof.returning]_vars
KnownReceiptsPreserved == [][(knownP => knownP') /\ (knownW => knownW')]_vars
UnknownNeverReplays == [][phase = "Unknown" => UNCHANGED <<issuedP, issuedW>>]_vars
Spec == Init /\ [][Next]_vars
=============================================================================
