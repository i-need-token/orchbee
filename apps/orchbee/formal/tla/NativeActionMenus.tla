----------------------------- MODULE NativeActionMenus -----------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANT MaxEpisodes
VARIABLES phase, episode, origin, current, target, valid, available, prerequisite,
          drawn, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability
vars == <<phase, episode, origin, current, target, valid, available, prerequisite,
          drawn, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>

Init == /\ phase = "closed" /\ episode = 0
        /\ origin = 1 /\ current = 1 /\ target = 1
        /\ valid = TRUE /\ available = TRUE /\ prerequisite = TRUE
        /\ drawn = FALSE /\ row = "member" /\ navigation = "none" /\ committed = FALSE
        /\ dispatched = [e \in 1..MaxEpisodes |-> 0]
        /\ badTarget = FALSE /\ leaked = FALSE /\ packet = FALSE /\ capture = FALSE
        /\ badAvailability = FALSE

Open(t, kind) == /\ phase = "closed" /\ episode < MaxEpisodes /\ ~packet /\ ~capture
           /\ phase' = "open" /\ episode' = episode + 1
           /\ origin' = current /\ target' = t
           /\ valid' = TRUE /\ available' = (kind # "project" \/ current = t)
           /\ prerequisite' = TRUE /\ drawn' = FALSE
           /\ row' = kind /\ navigation' = "none" /\ committed' = FALSE /\ packet' = TRUE
           /\ UNCHANGED <<current, dispatched, badTarget, leaked, capture, badAvailability>>
OpenCurrent == /\ phase = "closed" /\ Open(current, "member")
OpenOther == /\ phase = "closed" /\ Open(3-current, "member")
OpenProjectCurrent == /\ phase = "closed" /\ Open(current, "project")
OpenProjectOther == /\ phase = "closed" /\ Open(3-current, "project")

Draw == /\ phase = "open" /\ valid /\ current = origin
        /\ drawn' = TRUE /\ UNCHANGED <<phase, episode, origin, current, target, valid,
            available, prerequisite, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
Select == /\ phase = "open" /\ ~packet
          /\ row' \in (IF row = "project" THEN {"project"}
                      ELSE {"local", "member", "toggle", "close"} \ {row})
          /\ drawn' = FALSE /\ UNCHANGED <<phase, episode, origin, current, target, valid,
            available, prerequisite, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
Confirm == /\ phase = "open" /\ drawn /\ valid /\ ~packet /\ current = origin /\ available
           /\ phase' = "confirmed" /\ committed' = TRUE /\ packet' = TRUE /\ drawn' = FALSE
           /\ badAvailability' = badAvailability \/ ~available
           /\ UNCHANGED <<episode, origin, current, target, valid, available,
                prerequisite, row, navigation, dispatched, badTarget, leaked, capture>>
AlreadyExact == /\ phase = "confirmed" /\ valid
                /\ (row = "local" \/ current = target)
                /\ phase' = "offered" /\ navigation' = "exact"
                /\ UNCHANGED <<episode, origin, current, target, valid, available,
                    prerequisite, drawn, row, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
Admit(kind) == /\ phase = "confirmed" /\ valid /\ current = origin
               /\ row \notin {"local", "project"} /\ current # target
               /\ phase' = "transfer" /\ navigation' = kind
               /\ UNCHANGED <<episode, origin, current, target, valid, available,
                    prerequisite, drawn, row, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
AdmitContext == /\ phase = "confirmed" /\ Admit("context")
AdmitTab == /\ phase = "confirmed" /\ Admit("tab")
AdmitFocus == /\ phase = "confirmed" /\ Admit("focus")
OwnedCompletes(kind) == /\ phase = "transfer" /\ navigation = kind /\ valid
                        /\ current = origin
                        /\ phase' = "offered" /\ navigation' = "owned"
                        /\ current' = target
                        /\ UNCHANGED <<episode, origin, target, valid, available,
                            prerequisite, drawn, row, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
ContextCompletes == /\ phase = "transfer" /\ OwnedCompletes("context")
TabCompletes == /\ phase = "transfer" /\ OwnedCompletes("tab")
FocusCompletes == /\ phase = "transfer" /\ OwnedCompletes("focus")
ForeignCompletes == /\ phase \in {"open", "confirmed", "transfer", "offered"}
                    /\ current' = 3-current /\ navigation' = "foreign" /\ drawn' = FALSE
                    /\ UNCHANGED <<phase, episode, origin, target, valid, available,
                        prerequisite, row, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
Handoff == /\ phase = "offered" /\ committed /\ valid /\ available
           /\ navigation \in {"exact", "owned"}
           /\ IF row = "local" THEN current = origin ELSE current = target
           /\ row = "toggle" => prerequisite
           /\ phase' = "closed" /\ committed' = FALSE /\ packet' = TRUE
           /\ dispatched' = [dispatched EXCEPT ![episode] = @ + 1]
           /\ badTarget' = badTarget \/ (row # "local" /\ current # target)
           /\ badAvailability' = badAvailability \/ ~available
           /\ UNCHANGED <<episode, origin, current, target, valid, available,
                prerequisite, drawn, row, navigation, leaked, capture>>
Close == /\ phase # "closed"
         /\ phase' = "closed" /\ committed' = FALSE /\ drawn' = FALSE /\ packet' = TRUE
         /\ UNCHANGED <<episode, origin, current, target, valid, available,
            prerequisite, row, navigation, dispatched, badTarget, leaked, capture, badAvailability>>
Cancel == /\ phase # "closed" /\ Close
BrowseTimePasses == /\ phase = "open" /\ UNCHANGED vars
ExecutionExpires == /\ phase \in {"confirmed", "transfer", "offered"} /\ Close
GeometryChange == /\ phase # "closed" /\ Close
ConfigurationChange == /\ phase # "closed" /\ Close
CoveringCapture == /\ phase # "closed" /\ Close
DrawRefused == /\ phase = "open" /\ Close
NavigationRefused == /\ phase \in {"confirmed", "transfer"} /\ Close
TargetDisappears == /\ phase # "closed" /\ valid /\ valid' = FALSE /\ drawn' = FALSE
                     /\ UNCHANGED <<phase, episode, origin, current, target, available,
                         prerequisite, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
PrerequisiteChanges == /\ phase # "closed" /\ prerequisite /\ prerequisite' = FALSE
                       /\ UNCHANGED <<phase, episode, origin, current, target, valid,
                           available, drawn, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
Unavailable == /\ phase # "closed" /\ available /\ available' = FALSE /\ drawn' = FALSE
               /\ UNCHANGED <<phase, episode, origin, current, target, valid,
                   prerequisite, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
AvailableAgain == /\ phase = "open" /\ ~available /\ available' = TRUE /\ drawn' = FALSE
                  /\ (row # "project" \/ current = target)
                  /\ UNCHANGED <<phase, episode, origin, current, target, valid,
                      prerequisite, row, navigation, committed, dispatched, badTarget, leaked, packet, capture, badAvailability>>
DisabledActivation == /\ phase = "open" /\ drawn /\ ~available /\ ~packet
                      /\ UNCHANGED vars
PacketBoundary == /\ packet /\ packet' = FALSE
                  /\ UNCHANGED <<phase, episode, origin, current, target, valid, available,
                      prerequisite, drawn, row, navigation, committed, dispatched, badTarget, leaked, capture, badAvailability>>
SwallowInput == /\ (phase # "closed" \/ packet) /\ UNCHANGED vars
ForwardInput == /\ phase = "closed" /\ ~packet
                /\ leaked' = leaked \/ (phase # "closed" \/ packet)
                /\ UNCHANGED <<phase, episode, origin, current, target, valid, available,
                    prerequisite, drawn, row, navigation, committed, dispatched, badTarget, packet, capture, badAvailability>>
CaptureBegins == /\ phase = "closed" /\ ~capture /\ capture' = TRUE
                 /\ UNCHANGED <<phase, episode, origin, current, target, valid, available,
                     prerequisite, drawn, row, navigation, committed, dispatched, badTarget, leaked, packet, badAvailability>>
CaptureEnds == /\ phase = "closed" /\ capture /\ capture' = FALSE
               /\ UNCHANGED <<phase, episode, origin, current, target, valid, available,
                   prerequisite, drawn, row, navigation, committed, dispatched, badTarget, leaked, packet, badAvailability>>
OpenBlockedByCapture == /\ phase = "closed" /\ capture /\ UNCHANGED vars
ForeignDomainCallback == /\ phase = "transfer" /\ navigation \in {"context", "tab"}
                         /\ UNCHANGED vars
Next == OpenCurrent \/ OpenOther \/ OpenProjectCurrent \/ OpenProjectOther
        \/ Draw \/ Select \/ Confirm \/ AlreadyExact
        \/ AdmitContext \/ AdmitTab \/ AdmitFocus \/ ContextCompletes \/ TabCompletes
        \/ FocusCompletes \/ ForeignCompletes \/ Handoff \/ Cancel \/ BrowseTimePasses
        \/ ExecutionExpires \/ GeometryChange
        \/ ConfigurationChange \/ CoveringCapture \/ DrawRefused \/ NavigationRefused
        \/ TargetDisappears \/ PrerequisiteChanges \/ Unavailable \/ PacketBoundary
        \/ SwallowInput \/ ForwardInput \/ CaptureBegins \/ CaptureEnds \/ OpenBlockedByCapture
        \/ ForeignDomainCallback \/ AvailableAgain \/ DisabledActivation

TypeOK == /\ phase \in {"closed", "open", "confirmed", "transfer", "offered"}
          /\ episode \in 0..MaxEpisodes /\ origin \in 1..2 /\ current \in 1..2 /\ target \in 1..2
          /\ valid \in BOOLEAN /\ available \in BOOLEAN /\ prerequisite \in BOOLEAN
          /\ drawn \in BOOLEAN /\ committed \in BOOLEAN /\ packet \in BOOLEAN /\ capture \in BOOLEAN
          /\ row \in {"local", "member", "toggle", "close", "project"}
          /\ navigation \in {"none", "exact", "context", "tab", "focus", "owned", "foreign"}
          /\ dispatched \in [1..MaxEpisodes -> Nat] /\ badTarget \in BOOLEAN /\ leaked \in BOOLEAN
          /\ badAvailability \in BOOLEAN
SingleHandoff == \A e \in 1..MaxEpisodes : dispatched[e] <= 1
CorrectTarget == ~badTarget
NoInputLeak == ~leaked
ConfirmedHandoff == phase \in {"confirmed", "transfer", "offered"} => committed

ExistingCaptureOwns == capture => phase = "closed"
NoDisabledTransfer == ~badAvailability
NoProjectNavigation == row = "project" => navigation \notin {"context", "tab", "focus", "owned"}

Spec == Init /\ [][Next]_vars
====================================================================================
