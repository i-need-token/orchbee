-------------------------- MODULE NativeQuickWorkspace --------------------------
EXTENDS Naturals, TLC

Phases == {"Idle", "Prepare", "Edit", "ProjectSubmit", "ProjectWait", "ProjectRead",
           "LaunchSubmit", "LaunchWait", "LaunchRead", "Attach", "Ready", "Stopped", "Unknown"}
Modes == {"zero", "one", "many", "source"}
VARIABLES phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame, coverage
vars == <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
          fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW,
          knownP, knownW, started, freshTarget, firstFrame, coverage>>

Init == /\ phase = "Idle" /\ mode \in Modes /\ revision = 0 /\ captured = 0
        /\ peerWritten = FALSE /\ openings = 0 /\ token = 0 /\ drawn = 0
        /\ fieldChanges = 0 /\ selected = FALSE /\ valid = TRUE /\ confirmed = FALSE
        /\ acceptedDraw = 0 /\ issuedP = 0 /\ issuedW = 0
        /\ knownP = FALSE /\ knownW = FALSE /\ started = FALSE
        /\ freshTarget = FALSE /\ firstFrame = FALSE /\ coverage = "Init"

Open == /\ phase = "Idle" /\ openings < 2
        /\ phase' = "Prepare" /\ openings' = openings + 1 /\ token' = token + 1
        /\ captured' = revision /\ drawn' = 0 /\ coverage' = "Open"
        /\ UNCHANGED <<mode, revision, peerWritten, fieldChanges, selected, valid,
              confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
Prepared == /\ phase = "Prepare" /\ captured = revision
            /\ phase' = "Edit" /\ token' = token + 1 /\ selected' = (mode # "many")
            /\ coverage' = "Prepared"
            /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, drawn, fieldChanges,
                  valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
PrepareRefused == /\ phase = "Prepare" /\ phase' = "Idle" /\ coverage' = "PrepareRefused"
                  /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                      selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
Cancel == /\ phase \in {"Prepare", "Edit"} /\ phase' = "Idle" /\ drawn' = 0
          /\ coverage' = "Cancel"
          /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, fieldChanges,
              selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
LatePreparation == /\ phase = "Idle" /\ openings > 0 /\ coverage' = "LatePreparation"
                   /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
                        fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
Draw == /\ phase = "Edit" /\ drawn' = token /\ coverage' = "Draw"
        /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, fieldChanges,
              selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
ChangeField == /\ phase = "Edit" /\ fieldChanges < 2 /\ fieldChanges' = fieldChanges + 1
               /\ token' = token + 1 /\ drawn' = 0 /\ coverage' = "ChangeField"
               /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, selected,
                     valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
SelectProject == /\ phase = "Edit" /\ ~selected /\ selected' = TRUE /\ token' = token + 1
                 /\ drawn' = 0 /\ coverage' = "SelectProject"
                 /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, fieldChanges,
                       valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
EditValidity == /\ phase = "Edit" /\ valid' \in BOOLEAN /\ coverage' = "EditValidity"
                /\ UNCHANGED <<phase, mode, revision, captured, peerWritten, openings, token, drawn,
                      fieldChanges, selected, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
Confirm == /\ phase = "Edit" /\ valid /\ selected /\ drawn = token
           /\ confirmed' = TRUE /\ acceptedDraw' = drawn
           /\ phase' = IF mode = "zero" THEN "ProjectSubmit" ELSE "LaunchSubmit"
           /\ coverage' = "Confirm"
           /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                 fieldChanges, selected, valid, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
SubmitProject == /\ phase = "ProjectSubmit" /\ issuedP = 0 /\ phase' = "ProjectWait"
                 /\ issuedP' = issuedP + 1 /\ coverage' = "SubmitProject"
                 /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                       fieldChanges, selected, valid, confirmed, acceptedDraw, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
ProjectKnown == /\ phase = "ProjectWait" /\ captured = revision /\ revision' = revision + 1
                /\ knownP' = TRUE /\ phase' = "ProjectRead" /\ coverage' = "ProjectKnown"
                /\ UNCHANGED <<mode, captured, peerWritten, openings, token, drawn, fieldChanges,
                      selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownW, started, freshTarget, firstFrame>>
ProjectRead == /\ phase = "ProjectRead" /\ captured' = revision /\ phase' = "LaunchSubmit"
               /\ coverage' = "ProjectRead"
               /\ UNCHANGED <<mode, revision, peerWritten, openings, token, drawn, fieldChanges,
                     selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
CancelContinuation == /\ phase = "ProjectRead" /\ phase' = "Stopped" /\ coverage' = "CancelContinuation"
                      /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn,
                            fieldChanges, selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
SubmitLaunch == /\ phase = "LaunchSubmit" /\ issuedW = 0 /\ phase' = "LaunchWait"
                /\ issuedW' = issuedW + 1 /\ coverage' = "SubmitLaunch"
                /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                      selected, valid, confirmed, acceptedDraw, issuedP, knownP, knownW, started, freshTarget, firstFrame>>
LaunchKnown == /\ phase = "LaunchWait" /\ captured = revision /\ revision' = revision + 1
               /\ knownW' = TRUE /\ started' \in BOOLEAN /\ phase' = "LaunchRead"
               /\ coverage' = "LaunchKnown"
               /\ UNCHANGED <<mode, captured, peerWritten, openings, token, drawn, fieldChanges,
                     selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, freshTarget, firstFrame>>
Reject == /\ phase \in {"ProjectWait", "LaunchWait"} /\ phase' = "Stopped" /\ coverage' = "Reject"
          /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
Unknown == /\ phase \in {"ProjectWait", "LaunchWait"} /\ phase' = "Unknown" /\ coverage' = "Unknown"
           /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                 selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
ReadFailure == /\ phase \in {"ProjectRead", "LaunchRead"} /\ phase' = "Stopped" /\ coverage' = "ReadFailure"
               /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                     selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>
LaunchRead == /\ phase = "LaunchRead" /\ started /\ freshTarget' = TRUE /\ phase' = "Attach"
              /\ coverage' = "LaunchRead"
              /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                    selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, firstFrame>>
FirstFrame == /\ phase = "Attach" /\ firstFrame' = TRUE /\ phase' = "Ready" /\ coverage' = "FirstFrame"
              /\ UNCHANGED <<mode, revision, captured, peerWritten, openings, token, drawn, fieldChanges,
                    selected, valid, confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget>>
PeerWrite == /\ ~peerWritten /\ peerWritten' = TRUE /\ revision' = revision + 1 /\ coverage' = "PeerWrite"
             /\ UNCHANGED <<phase, mode, captured, openings, token, drawn, fieldChanges, selected, valid,
                   confirmed, acceptedDraw, issuedP, issuedW, knownP, knownW, started, freshTarget, firstFrame>>

Next == Open \/ Prepared \/ PrepareRefused \/ Cancel \/ LatePreparation \/ Draw \/ ChangeField
        \/ SelectProject \/ EditValidity \/ Confirm \/ SubmitProject \/ ProjectKnown \/ ProjectRead
        \/ CancelContinuation \/ SubmitLaunch \/ LaunchKnown \/ Reject \/ Unknown \/ ReadFailure
        \/ LaunchRead \/ FirstFrame \/ PeerWrite
TypeOK == /\ phase \in Phases /\ mode \in Modes /\ revision \in 0..3 /\ captured \in 0..3
          /\ openings \in 0..2 /\ token \in 0..8 /\ drawn \in 0..8 /\ acceptedDraw \in 0..8
          /\ fieldChanges \in 0..2
          /\ <<peerWritten, selected, valid, confirmed, knownP, knownW,
                started, freshTarget, firstFrame>> \in BOOLEAN \X BOOLEAN \X BOOLEAN \X BOOLEAN
                  \X BOOLEAN \X BOOLEAN \X BOOLEAN \X BOOLEAN \X BOOLEAN
AtMostOnce == /\ issuedP \in 0..1 /\ issuedW \in 0..1
ConfirmationBeforeMutation == (issuedP > 0 \/ issuedW > 0) => (confirmed /\ acceptedDraw > 0)
WorkspaceRequiresProject == issuedW > 0 => (mode # "zero" \/ knownP)
CancelledPreparationIsReadOnly == phase = "Idle" => (~confirmed /\ issuedP = 0 /\ issuedW = 0)
NoFalseReady == phase = "Ready" => (knownW /\ started /\ freshTarget /\ firstFrame)
KnownReceiptsPreserved == [][(knownP => knownP') /\ (knownW => knownW')]_vars
UnknownNeverReplays == [][phase = "Unknown" => UNCHANGED <<issuedP, issuedW>>]_vars
Spec == Init /\ [][Next]_vars
=============================================================================
