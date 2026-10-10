------------------------- MODULE NativeTabCloseSet -------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

\* Refinement above NativeClientCloseControl/Handoff: a validated Closed fact may
\* arrive after halt. Neither a fact nor a later Snapshot authorizes another Start.
\* Targets 2 and 3 are frozen; 1 is an unrelated survivor. One peer edit and one
\* local-selection edit bound the independent actors, without modeling transport.
VARIABLE s
vars == <<s>>
Targets == <<2, 3>>
Phases == {"prompt", "ready", "checking", "cas", "waiting", "verifying", "halted", "complete"}
Active == s.phase \notin {"halted", "complete"}
CurrentLocal == s.local = s.expectedLocal
Init == s = [phase |-> "prompt", shown |-> FALSE, confirmed |-> FALSE,
             cutoff |-> FALSE, expectedRevision |-> 0, cacheRevision |-> 0,
             revision |-> 0, local |-> 0, expectedLocal |-> 0,
             peerEdit |-> FALSE, localEdit |-> FALSE, starts |-> <<>>,
             pending |-> 0, published |-> FALSE, closedRevision |-> 0,
             knownClosed |-> {}, completed |-> 0, installed |-> 0,
             live |-> {1, 2, 3}, coverage |-> "Init"]

Draw == /\ s.phase = "prompt"
        /\ s' = [s EXCEPT !.shown = TRUE, !.coverage = "Draw"]
Resize == /\ s.phase = "prompt" /\ s.shown
          /\ s' = [s EXCEPT !.shown = FALSE, !.coverage = "Resize"]
Cancel == /\ s.phase = "prompt"
          /\ s' = [s EXCEPT !.phase = "halted", !.coverage = "Cancel"]
Confirm == /\ s.phase = "prompt" /\ s.shown /\ CurrentLocal
           /\ s' = [s EXCEPT !.phase = "ready", !.confirmed = TRUE, !.coverage = "Confirm"]
Expire == /\ s.confirmed /\ ~s.cutoff
          /\ s' = [s EXCEPT !.cutoff = TRUE, !.coverage = "Expire"]
PeerEdit == /\ ~s.peerEdit
            /\ s' = [s EXCEPT !.peerEdit = TRUE, !.revision = @ + 1, !.coverage = "PeerEdit"]
LocalEdit == /\ ~s.localEdit
             /\ s' = [s EXCEPT !.localEdit = TRUE, !.local = @ + 1, !.coverage = "LocalEdit"]
Halt == /\ Active /\ (s.cutoff \/ ~CurrentLocal)
        /\ s' = [s EXCEPT !.phase = "halted", !.coverage = "Halt"]

Admit == /\ s.phase = "ready" /\ ~s.cutoff /\ CurrentLocal
         /\ s.cacheRevision = s.expectedRevision /\ s.completed < Len(Targets)
         /\ s' = [s EXCEPT !.phase = "checking", !.coverage = "Admit"]
Preflight == /\ s.phase = "checking" /\ ~s.cutoff /\ CurrentLocal
             /\ s' = [s EXCEPT !.phase = IF s.revision = s.expectedRevision THEN "cas" ELSE "halted",
                      !.coverage = "Preflight"]
Start == /\ s.phase = "cas" /\ ~s.cutoff /\ CurrentLocal
         /\ s.revision = s.expectedRevision
         /\ s' = [s EXCEPT !.phase = "waiting", !.pending = Targets[s.completed + 1],
                  !.starts = Append(@, Targets[s.completed + 1]), !.published = FALSE,
                  !.coverage = "Start"]
RejectCASRace == /\ s.phase = "cas" /\ s.revision # s.expectedRevision
                 /\ s' = [s EXCEPT !.phase = "halted", !.coverage = "RejectCASRace"]
Unknown == /\ s.phase = "waiting"
           /\ s' = [s EXCEPT !.phase = "halted", !.coverage = "Unknown"]
Publish == /\ s.pending # 0 /\ ~s.published
           /\ s' = [s EXCEPT !.live = @ \ {s.pending}, !.revision = @ + 1,
                    !.closedRevision = s.revision + 1, !.published = TRUE, !.coverage = "Publish"]
Closed == /\ s.phase = "waiting" /\ s.published
          /\ s' = [s EXCEPT !.knownClosed = @ \cup {s.pending}, !.completed = @ + 1,
                   !.phase = "verifying", !.coverage = "Closed"]
LateClosed == /\ s.phase = "halted" /\ s.pending # 0 /\ s.published
              /\ s.pending \notin s.knownClosed
              /\ s' = [s EXCEPT !.knownClosed = @ \cup {s.pending}, !.completed = @ + 1,
                       !.coverage = "LateClosed"]

\* The pre-install local fence precedes any own survivor-selection transition.
\* Full snapshot equality includes unrelated contexts/metadata, not only revision.
InstallExact == /\ s.phase = "verifying" /\ ~s.cutoff /\ CurrentLocal
                /\ s.closedRevision = s.expectedRevision + 1
                /\ s.revision = s.closedRevision /\ s.cacheRevision = s.expectedRevision
                /\ s' = [s EXCEPT !.expectedRevision = s.revision, !.cacheRevision = s.revision,
                         !.local = @ + 1, !.expectedLocal = s.local + 1, !.installed = @ + 1,
                         !.pending = 0, !.published = FALSE,
                         !.phase = IF s.completed = Len(Targets) THEN "complete" ELSE "ready",
                         !.coverage = "InstallExact"]
InstallAfterLocalDrift == /\ s.phase = "verifying" /\ ~CurrentLocal
                         /\ s' = [s EXCEPT !.phase = "halted", !.cacheRevision = s.revision,
                                  !.coverage = "InstallAfterLocalDrift"]
InstallNewerRevision == /\ s.phase = "verifying"
                        /\ (s.closedRevision # s.expectedRevision + 1 \/ s.revision # s.closedRevision)
                        /\ s' = [s EXCEPT !.phase = "halted", !.cacheRevision = s.revision,
                                 !.coverage = "InstallNewerRevision"]
\* A wrong target/source install cannot advance even at a plausible revision.
InstallWrongSource == /\ s.phase = "verifying"
                      /\ s' = [s EXCEPT !.phase = "halted", !.coverage = "InstallWrongSource"]

Next == Draw \/ Resize \/ Cancel \/ Confirm \/ Expire \/ PeerEdit \/ LocalEdit \/ Halt
        \/ Admit \/ Preflight \/ Start \/ RejectCASRace \/ Unknown \/ Publish \/ Closed
        \/ LateClosed \/ InstallExact \/ InstallAfterLocalDrift \/ InstallNewerRevision \/ InstallWrongSource
Spec == Init /\ [][Next]_vars

TypeOK == /\ s.phase \in Phases /\ s.completed \in 0..2 /\ s.installed \in 0..2
          /\ s.revision \in 0..3 /\ s.pending \in {0, 2, 3}
          /\ s.starts \in {<<>>, <<2>>, <<2, 3>>} /\ s.live \subseteq {1, 2, 3}
          /\ s.knownClosed \subseteq {2, 3}
NoUnconfirmedStart == Len(s.starts) > 0 => s.confirmed
FrozenOrderWithoutReplay == s.starts = SubSeq(Targets, 1, Len(s.starts))
UnrelatedSurvivorPreserved == 1 \in s.live
CountIsKnownFact == s.completed = Cardinality(s.knownClosed)
InstallRequiresKnownClose == s.installed <= s.completed
NoNextStartBeforeOwnedInstall == Len(s.starts) <= s.installed + 1
CompleteIsInstalled == s.phase = "complete" => s.installed = Len(Targets)
HaltedNeverStarts == [][s.phase = "halted" => s'.starts = s.starts]_vars
CutoffNeverStarts == [][s.cutoff => s'.starts = s.starts]_vars
StoppingPreservesObservation == [][s.phase = "halted" => s.knownClosed \subseteq s'.knownClosed]_vars
LocalDriftCannotBecomeOwnTransition == [][(s.phase = "verifying" /\ ~CurrentLocal) => s'.installed = s.installed]_vars
WrongRevisionCannotAdvance == [][(s.phase = "verifying" /\ s.revision # s.expectedRevision + 1) => s'.installed = s.installed]_vars
=============================================================================
