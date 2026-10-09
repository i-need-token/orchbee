------------------------ MODULE TerminalScrollbar ------------------------
(* One retained viewport and its pointer capture. Pixel/cell mapping is checked
   by Rust boundary tests. Window validation, worker leases and demand legs
   refine TerminalClientViewport; this model checks the additional absolute
   intent replacement, capture fences and return-to-live ordering. *)
EXTENDS Integers, TLC

VARIABLES focus, attached, generation, maximum, capture,
          phase, facts, pending, flight, recovered
vars == <<focus, attached, generation, maximum, capture,
          phase, facts, pending, flight, recovered>>

Init ==
    /\ focus = 1 /\ attached = TRUE /\ generation = 1 /\ maximum = 2
    /\ capture = <<0, 0>> /\ phase = "live" /\ facts = FALSE
    /\ pending = 0 /\ flight = 0 /\ recovered = FALSE

Press ==
    /\ focus = 1 /\ attached /\ phase \notin {"recovering", "read", "ack"}
    /\ capture' = <<generation, maximum>>
    /\ UNCHANGED <<focus, attached, generation, maximum, phase, facts,
                   pending, flight, recovered>>

Move(offset) ==
    /\ capture = <<generation, maximum>> /\ focus = 1 /\ attached
    /\ phase # "recovering" /\ offset \in 1..maximum
    /\ pending' = offset
    /\ phase' = IF phase = "live" THEN "waiting" ELSE phase
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, facts,
                   flight, recovered>>

Release ==
    /\ capture # <<0, 0>> /\ capture' = <<0, 0>>
    /\ UNCHANGED <<focus, attached, generation, maximum, phase, facts,
                   pending, flight, recovered>>

Cancel ==
    /\ capture # <<0, 0>> /\ capture' = <<0, 0>>
    /\ UNCHANGED <<focus, attached, generation, maximum, phase, facts,
                   pending, flight, recovered>>

Switch ==
    /\ focus' = 3 - focus /\ capture' = <<0, 0>>
    /\ UNCHANGED <<attached, generation, maximum, phase, facts,
                   pending, flight, recovered>>

Close ==
    /\ attached /\ attached' = FALSE /\ capture' = <<0, 0>>
    /\ UNCHANGED <<focus, generation, maximum, phase, facts,
                   pending, flight, recovered>>

Resize ==
    /\ generation = 1 /\ generation' = 2
    /\ capture' = <<0, 0>> /\ facts' = FALSE
    /\ UNCHANGED <<focus, attached, maximum, phase, pending, flight, recovered>>

OutputGrowth ==
    /\ maximum = 2 /\ maximum' = 3
    /\ capture' = <<0, 0>>
    /\ UNCHANGED <<focus, attached, generation, phase, facts,
                   pending, flight, recovered>>

Facts ==
    /\ ~facts /\ facts' = TRUE
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, phase,
                   pending, flight, recovered>>

Read ==
    /\ phase = "waiting" /\ pending > 0 /\ facts /\ attached
    /\ flight' = pending /\ phase' = "read"
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, facts,
                   pending, recovered>>

Install ==
    /\ phase = "read" /\ phase' = "ack"
    /\ pending' = IF pending = flight THEN 0 ELSE pending
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, facts,
                   flight, recovered>>

Ack ==
    /\ phase = "ack" /\ phase' = "waiting" /\ flight' = 0
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, facts,
                   pending, recovered>>

Bottom ==
    /\ capture = <<generation, maximum>> /\ focus = 1 /\ attached
    /\ phase \notin {"live", "recovering"}
    /\ phase' = "recovering" /\ pending' = 0 /\ recovered' = FALSE
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, facts, flight>>

RecoveryApplied ==
    /\ phase = "recovering" /\ ~recovered /\ recovered' = TRUE
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, phase,
                   facts, pending, flight>>

Frame ==
    /\ phase = "recovering" /\ recovered
    /\ phase' = "live" /\ flight' = 0 /\ pending' = 0
    /\ UNCHANGED <<focus, attached, generation, maximum, capture, facts, recovered>>

Next ==
    \/ Press \/ (\E offset \in 1..3 : Move(offset)) \/ Release \/ Cancel
    \/ Switch \/ Close \/ Resize \/ OutputGrowth \/ Facts
    \/ Read \/ Install \/ Ack \/ Bottom \/ RecoveryApplied \/ Frame

Spec == Init /\ [][Next]_vars
    /\ WF_vars(RecoveryApplied) /\ WF_vars(Frame)

TypeOK ==
    /\ focus \in {1, 2} /\ attached \in BOOLEAN /\ generation \in {1, 2}
    /\ maximum \in {2, 3} /\ capture \in {<<0, 0>>} \cup ({1, 2} \X {2, 3})
    /\ phase \in {"live", "waiting", "read", "ack", "recovering"}
    /\ facts \in BOOLEAN /\ pending \in 0..3 /\ flight \in 0..3
    /\ recovered \in BOOLEAN
Bounded == pending <= maximum /\ flight <= maximum
RequestsFenced == [][(pending' # pending /\ pending' > 0) =>
    (focus = 1 /\ attached /\ capture = <<generation, maximum>>)]_vars
ReadAfterFacts == [][(phase # "read" /\ phase' = "read") =>
    (phase = "waiting" /\ facts /\ attached)]_vars
LatestSurvives == [][(phase = "read" /\ phase' = "ack") =>
    pending' = (IF pending = flight THEN 0 ELSE pending)]_vars
RecoveryBeforeLive == [][(phase = "recovering" /\ phase' = "live") => recovered]_vars
RecoveryCompletes == (phase = "recovering") ~> (phase = "live")
=============================================================================
