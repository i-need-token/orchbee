-------------------- MODULE ClineObservationOrdering --------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

Sessions == {"session-1", "session-2"}
Turns == {"turn-1", "turn-2", "turn-3"}
NoTurn == "no-turn"
TombstoneLimit == 2

VARIABLES
  activeTurn,
  completedTurns,
  lastAction,
  lastSession,
  lastTurn,
  previousActiveTurn,
  previousCompletedTurns

core == <<activeTurn, completedTurns>>
history ==
  <<lastAction, lastSession, lastTurn, previousActiveTurn,
    previousCompletedTurns>>
vars == <<core, history>>

SeqSet(sequence) == {sequence[index] : index \in 1..Len(sequence)}

Retained(session, turn) == turn \in SeqSet(completedTurns[session])

Remember(sequence, turn) ==
  IF Len(sequence) < TombstoneLimit
  THEN Append(sequence, turn)
  ELSE Append(Tail(sequence), turn)

Record(action, session, turn) ==
  /\ lastAction' = action
  /\ lastSession' = session
  /\ lastTurn' = turn
  /\ previousActiveTurn' = activeTurn
  /\ previousCompletedTurns' = completedTurns

Init ==
  /\ activeTurn = [session \in Sessions |-> NoTurn]
  /\ completedTurns = [session \in Sessions |-> <<>>]
  /\ lastAction = "init"
  /\ lastSession = "session-1"
  /\ lastTurn = "turn-1"
  /\ previousActiveTurn = activeTurn
  /\ previousCompletedTurns = completedTurns

DeliverStart(session, turn) ==
  /\ ~Retained(session, turn)
  /\ activeTurn' = [activeTurn EXCEPT ![session] = turn]
  /\ UNCHANGED completedTurns
  /\ Record("DeliverStart", session, turn)

SuppressStart(session, turn) ==
  /\ Retained(session, turn)
  /\ UNCHANGED core
  /\ Record("SuppressStart", session, turn)

DeliverActivity(session, turn) ==
  /\ ~Retained(session, turn)
  /\ activeTurn' =
       IF activeTurn[session] = NoTurn
       THEN [activeTurn EXCEPT ![session] = turn]
       ELSE activeTurn
  /\ UNCHANGED completedTurns
  /\ Record("DeliverActivity", session, turn)

SuppressActivity(session, turn) ==
  /\ Retained(session, turn)
  /\ UNCHANGED core
  /\ Record("SuppressActivity", session, turn)

DeliverCompletion(session, turn) ==
  /\ ~Retained(session, turn)
  /\ Len(completedTurns[session]) < TombstoneLimit
  /\ activeTurn' =
       IF activeTurn[session] = turn
       THEN [activeTurn EXCEPT ![session] = NoTurn]
       ELSE activeTurn
  /\ completedTurns' =
       [completedTurns EXCEPT ![session] = Remember(@, turn)]
  /\ Record("DeliverCompletion", session, turn)

DeliverCompletionWithEviction(session, turn) ==
  /\ ~Retained(session, turn)
  /\ Len(completedTurns[session]) = TombstoneLimit
  /\ activeTurn' =
       IF activeTurn[session] = turn
       THEN [activeTurn EXCEPT ![session] = NoTurn]
       ELSE activeTurn
  /\ completedTurns' =
       [completedTurns EXCEPT ![session] = Remember(@, turn)]
  /\ Record("DeliverCompletionWithEviction", session, turn)

SuppressCompletion(session, turn) ==
  /\ Retained(session, turn)
  /\ UNCHANGED core
  /\ Record("SuppressCompletion", session, turn)

Next ==
  \/ \E session \in Sessions, turn \in Turns: DeliverStart(session, turn)
  \/ \E session \in Sessions, turn \in Turns: SuppressStart(session, turn)
  \/ \E session \in Sessions, turn \in Turns: DeliverActivity(session, turn)
  \/ \E session \in Sessions, turn \in Turns: SuppressActivity(session, turn)
  \/ \E session \in Sessions, turn \in Turns: DeliverCompletion(session, turn)
  \/ \E session \in Sessions, turn \in Turns:
       DeliverCompletionWithEviction(session, turn)
  \/ \E session \in Sessions, turn \in Turns: SuppressCompletion(session, turn)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ activeTurn \in [Sessions -> (Turns \cup {NoTurn})]
  /\ completedTurns \in [Sessions -> Seq(Turns)]
  /\ \A session \in Sessions:
       /\ Len(completedTurns[session]) <= TombstoneLimit
       /\ Cardinality(SeqSet(completedTurns[session])) =
            Len(completedTurns[session])
  /\ lastAction \in {
       "init",
       "DeliverStart",
       "SuppressStart",
       "DeliverActivity",
       "SuppressActivity",
       "DeliverCompletion",
       "DeliverCompletionWithEviction",
       "SuppressCompletion"
     }
  /\ lastSession \in Sessions
  /\ lastTurn \in Turns
  /\ previousActiveTurn \in [Sessions -> (Turns \cup {NoTurn})]
  /\ previousCompletedTurns \in [Sessions -> Seq(Turns)]

RetainedCompletionCannotBeActive ==
  \A session \in Sessions:
    activeTurn[session] \notin SeqSet(completedTurns[session])

DeliveredWorkWasNotAlreadyCompleted ==
  lastAction \in {"DeliverStart", "DeliverActivity"} =>
    lastTurn \notin SeqSet(previousCompletedTurns[lastSession])

SuppressionPreservesState ==
  lastAction \in {"SuppressStart", "SuppressActivity", "SuppressCompletion"} =>
    /\ activeTurn = previousActiveTurn
    /\ completedTurns = previousCompletedTurns

CompletionIsSessionLocal ==
  lastAction \in {"DeliverCompletion", "DeliverCompletionWithEviction"} =>
    \A session \in Sessions \ {lastSession}:
      /\ activeTurn[session] = previousActiveTurn[session]
      /\ completedTurns[session] = previousCompletedTurns[session]

=============================================================================
