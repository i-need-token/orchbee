----------------------- MODULE PaneIncarnationFencing -----------------------
EXTENDS Naturals

\* One queued command and one worker flight expose replacement between
\* admission, execution and completion. Panes may disappear and reappear once.
\* Layout permutations and operation-specific results are implementation tests.
\* A navigation the user requested stays pending until it lands or is abandoned;
\* typed input never reaches the pane the user is leaving in between.
Panes == {1, 2}
Empty == [pane |-> 0, incarnation |-> 0, anchor |-> 0, kind |-> "none"]
VARIABLE s
vars == <<s>>
Init == s = [
  phase |-> [p \in Panes |-> "absent"],
  incarnation |-> [p \in Panes |-> 0],
  anchor |-> 1, focus |-> 1, intent |-> 0, queued |-> Empty, flight |-> Empty,
  frame |-> [p \in Panes |-> Empty],
  cleanup |-> [p \in Panes |-> 0],
  effectSafe |-> TRUE, cleanupSafe |-> TRUE, inputSafe |-> TRUE]
Current(tag) == tag.pane \in Panes /\
  s.phase[tag.pane] = "live" /\
  tag.incarnation = s.incarnation[tag.pane] /\ tag.anchor = s.anchor
Mint(p) ==
  /\ s.phase[p] \in {"absent", "departing"} /\ s.incarnation[p] < 2
  /\ s' = [s EXCEPT !.phase[p] = "live", !.incarnation[p] = @ + 1,
       !.frame[p] = Empty]
Depart(p) ==
  /\ s.phase[p] = "live"
  /\ s' = [s EXCEPT !.phase[p] = "departing",
       !.cleanup[p] = s.incarnation[p], !.frame[p] = Empty]
Cleanup(p) ==
  /\ s.cleanup[p] # 0
  /\ LET exact == s.cleanup[p] = s.incarnation[p]
         after == IF exact /\ s.phase[p] = "departing" THEN "absent" ELSE s.phase[p]
     IN s' = [s EXCEPT !.phase[p] = after, !.cleanup[p] = 0,
          !.cleanupSafe = ~exact => after = s.phase[p]]
Select(p) == /\ s.focus # p /\ s' = [s EXCEPT !.focus = p]
\* A requested navigation (a sidebar click, a Workspace switch) does not move
\* focus until its own refresh lands; a later request replaces the target.
Intend(p) == /\ s.focus # p /\ s' = [s EXCEPT !.intent = p]
Land == /\ s.intent # 0 /\ s' = [s EXCEPT !.focus = s.intent, !.intent = 0]
Abandon == /\ s.intent # 0 /\ s' = [s EXCEPT !.intent = 0]
AdvanceAnchor ==
  /\ s.anchor = 1
  /\ s' = [s EXCEPT !.anchor = 2, !.frame = [p \in Panes |-> Empty]]
Admit(p, kind) ==
  /\ s.phase[p] = "live" /\ s.queued = Empty
  /\ kind = "frame" \/ (p = s.focus /\ s.intent = 0)
  /\ s' = [s EXCEPT !.queued =
       [pane |-> p, incarnation |-> s.incarnation[p], anchor |-> s.anchor, kind |-> kind],
       !.inputSafe = @ /\ (kind = "frame" \/ s.intent = 0)]
Start ==
  /\ s.queued # Empty /\ s.flight = Empty
  /\ s' = [s EXCEPT !.flight = IF Current(s.queued) THEN s.queued ELSE Empty,
       !.queued = Empty]
Complete ==
  /\ s.flight # Empty
  /\ LET accepted == Current(s.flight)
     IN s' = [s EXCEPT
       !.effectSafe = ~accepted \/ Current(s.flight),
       !.frame = IF accepted /\ s.flight.kind = "frame"
         THEN [s.frame EXCEPT ![s.flight.pane] = s.flight] ELSE @,
       !.flight = Empty]
Draw(p) ==
  /\ s.frame[p] # Empty
  /\ s' = [s EXCEPT !.effectSafe = Current(s.frame[p]), !.frame[p] = Empty]
Next ==
  \/ \E p \in Panes: Mint(p) \/ Depart(p) \/ Cleanup(p) \/ Select(p) \/ Draw(p)
  \/ \E p \in Panes, kind \in {"input", "frame"}: Admit(p, kind)
  \/ \E p \in Panes: Intend(p)
  \/ AdvanceAnchor \/ Start \/ Complete \/ Land \/ Abandon
Spec == Init /\ [][Next]_vars /\ WF_vars(Start) /\ WF_vars(Complete) /\
  WF_vars(Land) /\ \A p \in Panes: WF_vars(Cleanup(p))

TypeOK ==
  /\ s.phase \in [Panes -> {"absent", "live", "departing"}]
  /\ s.incarnation \in [Panes -> 0..2]
  /\ s.anchor \in 1..2 /\ s.focus \in Panes /\ s.intent \in {0} \cup Panes
  /\ s.cleanup \in [Panes -> 0..2]
FramesAreCurrent == \A p \in Panes: s.frame[p] # Empty => Current(s.frame[p])
EffectsUseCurrentIncarnation == s.effectSafe
LateCleanupPreservesReplacement == s.cleanupSafe
InputFollowsSettledNavigation == s.inputSafe
DeparturesEventuallySettle == \A p \in Panes:
  s.phase[p] = "departing" ~> s.phase[p] # "departing"
NavigationEventuallySettles == s.intent # 0 ~> s.intent = 0
=============================================================================
