---------------------- MODULE NativeWorktreeLaunch ----------------------
EXTENDS Naturals, Sequences, TLC
CONSTANT MaxOpenings
VARIABLES phase, opening, token, painted, complete, edits, origin, visible,
          project, projectReceipt, confirmed, starts, outcome, focus, stale
vars == <<phase, opening, token, painted, complete, edits, origin, visible,
          project, projectReceipt, confirmed, starts, outcome, focus, stale>>

Init == /\ phase = "hidden" /\ opening = 0 /\ token = 0 /\ painted = FALSE
        /\ complete = TRUE /\ edits = 0 /\ origin = TRUE /\ visible = FALSE
        /\ project \in {"existing", "choose", "default"}
        /\ projectReceipt = FALSE
        /\ confirmed = [i \in 1..MaxOpenings |-> FALSE]
        /\ starts = [i \in 1..MaxOpenings |-> 0]
        /\ outcome = "none" /\ focus = FALSE /\ stale = 0
Open == /\ phase = "hidden" /\ opening < MaxOpenings
        /\ opening' = opening + 1 /\ phase' = "prepare"
        /\ token' = token + 1 /\ painted' = FALSE /\ complete' = TRUE
        /\ edits' = 0 /\ origin' = TRUE /\ visible' = TRUE
        /\ outcome' = "none" /\ focus' = FALSE
        /\ UNCHANGED <<project, projectReceipt, confirmed, starts, stale>>
Prepared == /\ phase = "prepare" /\ phase' = "draft"
            /\ token' = token + 1 /\ painted' = FALSE
            /\ UNCHANGED <<opening, complete, edits, origin, visible, project,
                           projectReceipt, confirmed, starts, outcome, focus, stale>>
PrepareUnavailable == /\ phase = "prepare" /\ phase' = "failed"
                      /\ UNCHANGED <<opening, token, painted, complete, edits,
                         origin, visible, project, projectReceipt, confirmed,
                         starts, outcome, focus, stale>>
ChooseProject == /\ phase = "draft" /\ project = "choose"
                 /\ project' = "existing" /\ token' = token + 1 /\ painted' = FALSE
                 /\ UNCHANGED <<phase, opening, complete, edits, origin, visible,
                    projectReceipt, confirmed, starts, outcome, focus, stale>>
Edit == /\ phase = "draft" /\ edits < 2
        /\ edits' = edits + 1 /\ token' = token + 1 /\ painted' = FALSE
        /\ UNCHANGED <<phase, opening, complete, origin, visible, project,
                       projectReceipt, confirmed, starts, outcome, focus, stale>>
Draw == /\ visible /\ phase = "draft" /\ ~painted
        /\ painted' = TRUE
        /\ UNCHANGED <<phase, opening, token, complete, edits, origin, visible,
                       project, projectReceipt, confirmed, starts, outcome, focus, stale>>
IncompletePacket == /\ phase = "draft" /\ complete /\ complete' = FALSE
                    /\ painted' = FALSE
                    /\ UNCHANGED <<phase, opening, token, edits, origin, visible,
                       project, projectReceipt, confirmed, starts, outcome, focus, stale>>
CompletePacket == /\ phase = "draft" /\ ~complete /\ complete' = TRUE
                  /\ UNCHANGED <<phase, opening, token, painted, edits, origin,
                     visible, project, projectReceipt, confirmed, starts, outcome, focus, stale>>
Confirm == /\ phase = "draft" /\ origin /\ visible /\ painted /\ complete
           /\ project # "choose"
           /\ confirmed' = [confirmed EXCEPT ![opening] = TRUE]
           /\ phase' = IF project = "default" /\ ~projectReceipt
                        THEN "project" ELSE "start"
           /\ painted' = FALSE
           /\ UNCHANGED <<opening, token, complete, edits, origin, visible,
                          project, projectReceipt, starts, outcome, focus, stale>>
ProjectKnown == /\ phase = "project" /\ projectReceipt' = TRUE
                /\ phase' = IF origin /\ visible THEN "start" ELSE "failed"
                /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                   visible, project, confirmed, starts, outcome, focus, stale>>
ProjectUnknown == /\ phase = "project" /\ phase' = "failed"
                  /\ outcome' = "project-unknown"
                  /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                     visible, project, projectReceipt, confirmed, starts, focus, stale>>
StartAccepted == /\ phase = "start" /\ origin /\ visible /\ starts[opening] = 0
                 /\ starts' = [starts EXCEPT ![opening] = 1]
                 /\ phase' = "observe" /\ outcome' = "pending"
                 /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                    visible, project, projectReceipt, confirmed, focus, stale>>
StartUncertain == /\ phase = "start" /\ origin /\ visible /\ starts[opening] = 0
                  /\ starts' = [starts EXCEPT ![opening] = 1]
                  /\ phase' = "observe" /\ outcome' = "unknown"
                  /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                     visible, project, projectReceipt, confirmed, focus, stale>>
StartRefused == /\ phase = "start" /\ phase' = "failed"
                /\ outcome' = "not-submitted"
                /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                   visible, project, projectReceipt, confirmed, starts, focus, stale>>
ReadPending == /\ phase = "observe" /\ outcome # "pending" /\ outcome' = "pending"
               /\ UNCHANGED <<phase, opening, token, painted, complete, edits,
                  origin, visible, project, projectReceipt, confirmed, starts, focus, stale>>
ReadOpened == /\ phase = "observe" /\ phase' = "opened" /\ outcome' = "opened"
              /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                 visible, project, projectReceipt, confirmed, starts, focus, stale>>
ReadFailed == /\ phase = "observe" /\ phase' = "failed" /\ outcome' = "failed"
              /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                 visible, project, projectReceipt, confirmed, starts, focus, stale>>
ReadLost == /\ phase = "observe" /\ outcome # "unknown" /\ outcome' = "unknown"
            /\ UNCHANGED <<phase, opening, token, painted, complete, edits, origin,
               visible, project, projectReceipt, confirmed, starts, focus, stale>>
OriginChanged == /\ phase # "hidden" /\ origin
                 /\ origin' = FALSE /\ painted' = FALSE /\ focus' = FALSE
                 /\ UNCHANGED <<phase, opening, token, complete, edits, visible,
                    project, projectReceipt, confirmed, starts, outcome, stale>>
Dismiss == /\ visible /\ visible' = FALSE /\ painted' = FALSE /\ focus' = FALSE
           /\ phase' = IF phase \in {"prepare", "draft", "failed", "ready"}
                       THEN "hidden" ELSE phase
           /\ origin' = IF phase \in {"project", "start", "observe", "opened"}
                        THEN FALSE ELSE origin
           /\ UNCHANGED <<opening, token, complete, edits, project,
                          projectReceipt, confirmed, starts, outcome, stale>>
Reopen == /\ ~visible /\ phase \in {"observe", "opened", "project", "start"}
          /\ visible' = TRUE
          /\ UNCHANGED <<phase, opening, token, painted, complete, edits, origin,
                         project, projectReceipt, confirmed, starts, outcome, focus, stale>>
Navigate == /\ phase = "opened" /\ visible /\ origin
            /\ phase' = "navigate"
            /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
               visible, project, projectReceipt, confirmed, starts, outcome, focus, stale>>
FirstFrame == /\ phase = "navigate" /\ visible /\ origin
              /\ phase' = "ready" /\ focus' = TRUE
              /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                 visible, project, projectReceipt, confirmed, starts, outcome, stale>>
EntryFailed == /\ phase \in {"opened", "navigate"} /\ phase' = "failed"
               /\ UNCHANGED <<opening, token, painted, complete, edits, origin,
                  visible, project, projectReceipt, confirmed, starts, outcome, focus, stale>>
StaleCallback == /\ opening > 0 /\ stale = 0 /\ stale' = 1
                 /\ UNCHANGED <<phase, opening, token, painted, complete, edits,
                    origin, visible, project, projectReceipt, confirmed, starts, outcome, focus>>
Next == Open \/ Prepared \/ PrepareUnavailable \/ ChooseProject \/ Edit \/ Draw
        \/ IncompletePacket \/ CompletePacket \/ Confirm \/ ProjectKnown
        \/ ProjectUnknown \/ StartAccepted \/ StartUncertain \/ StartRefused
        \/ ReadPending \/ ReadOpened \/ ReadFailed \/ ReadLost \/ OriginChanged
        \/ Dismiss \/ Reopen \/ Navigate \/ FirstFrame \/ EntryFailed \/ StaleCallback
TypeOK == /\ opening \in 0..MaxOpenings /\ edits \in 0..2 /\ stale \in 0..1
          /\ starts \in [1..MaxOpenings -> 0..1]
          /\ confirmed \in [1..MaxOpenings -> BOOLEAN]
ConfirmedMutation == \A i \in 1..MaxOpenings: starts[i] = 1 => confirmed[i]
ProjectPrerequisite == phase = "start" => project # "choose"
                       /\ (project = "default" => projectReceipt)
FocusGate == focus => phase = "ready" /\ outcome = "opened" /\ origin /\ visible
RetainProject == [][projectReceipt => projectReceipt']_vars
OneStart == [][\A i \in 1..MaxOpenings:
                 starts'[i] > starts[i] => phase = "start" /\ origin /\ visible]_vars
Spec == Init /\ [][Next]_vars
=============================================================================
