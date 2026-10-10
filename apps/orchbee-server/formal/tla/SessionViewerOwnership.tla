----------------------- MODULE SessionViewerOwnership -----------------------
EXTENDS Naturals, FiniteSets

\* Two immutable viewer identities may share either Session or target. A grant
\* remains owned while cleanup is pending; a late cleanup cannot release a
\* successor's grant. Frame/history queues belong to RelayDeliveryIsolation.
Viewers == {1, 2}
Sessions == {"a", "b"}
Targets == {"x", "y"}
Empty == [session |-> "none", target |-> "none", epoch |-> 0, phase |-> "unused"]
VARIABLE s
vars == <<s>>
Init == s = [
  viewer |-> [v \in Viewers |-> Empty],
  grant |-> [t \in Targets |-> 0],
  epoch |-> 1, disposed |-> {},
  cleanup |-> {}, accepted |-> {}, staleCleanupSafe |-> TRUE]
Current(v) == s.viewer[v].epoch = s.epoch /\
  s.viewer[v].target \notin s.disposed
Attach(v, session, target) ==
  /\ s.viewer[v].phase = "unused" /\ target \notin s.disposed
  /\ ~\E other \in Viewers:
       s.viewer[other].phase = "active" /\
       s.viewer[other].session = session /\ s.viewer[other].target = target
  /\ s' = [s EXCEPT !.viewer[v] =
       [session |-> session, target |-> target, epoch |-> s.epoch, phase |-> "active"]]
Acquire(v) ==
  /\ s.viewer[v].phase = "active" /\ Current(v)
  /\ s.grant[s.viewer[v].target] = 0
  /\ s' = [s EXCEPT !.grant[s.viewer[v].target] = v]
Control(v, session, target, epoch) ==
  /\ s.viewer[v].phase = "active" /\ Current(v)
  /\ s.viewer[v].session = session /\ s.viewer[v].target = target
  /\ epoch = s.epoch /\ s.grant[target] = v
  /\ s' = [s EXCEPT !.accepted = @ \cup {
       [viewer |-> v, session |-> session, target |-> target, epoch |-> epoch,
        owned |-> s.viewer[v].session = session /\ s.viewer[v].target = target /\
          s.viewer[v].epoch = epoch /\ s.grant[target] = v]}]
Close(v) ==
  /\ s.viewer[v].phase = "active"
  /\ s' = [s EXCEPT !.viewer[v].phase = "closing", !.cleanup = @ \cup {v}]
ReleaseController(v) ==
  /\ s.viewer[v].phase = "closing"
  /\ s.grant[s.viewer[v].target] = v
  /\ s' = [s EXCEPT !.grant[s.viewer[v].target] = 0]
ReleaseViewer(v) ==
  /\ s.viewer[v].phase = "closing"
  /\ s.grant[s.viewer[v].target] # v
  /\ s' = [s EXCEPT !.viewer[v].phase = "retired"]
LateCleanup(v) ==
  /\ v \in s.cleanup /\ s.viewer[v].phase = "retired"
  /\ LET target == s.viewer[v].target
         after == IF s.grant[target] = v THEN 0 ELSE s.grant[target]
     IN s' = [s EXCEPT !.grant[target] = after,
          !.staleCleanupSafe = after = s.grant[target]]
Dispose(target) ==
  /\ target \notin s.disposed
  /\ s' = [s EXCEPT !.disposed = @ \cup {target},
       !.viewer = [v \in Viewers |->
         IF s.viewer[v].target = target /\ s.viewer[v].phase = "active"
         THEN [s.viewer[v] EXCEPT !.phase = "closing"] ELSE s.viewer[v]],
       !.cleanup = @ \cup {v \in Viewers:
         s.viewer[v].target = target /\ s.viewer[v].phase = "active"}]
ReplaceWorker ==
  /\ s.epoch = 1
  /\ s' = [s EXCEPT !.epoch = 2,
       !.viewer = [v \in Viewers |->
         IF s.viewer[v].phase = "active"
         THEN [s.viewer[v] EXCEPT !.phase = "closing"] ELSE s.viewer[v]],
       !.cleanup = @ \cup {v \in Viewers: s.viewer[v].phase = "active"}]
Next ==
  \/ \E v \in Viewers, session \in Sessions, target \in Targets: Attach(v, session, target)
  \/ \E v \in Viewers: Acquire(v) \/ Close(v) \/ ReleaseController(v) \/ ReleaseViewer(v) \/ LateCleanup(v)
  \/ \E v \in Viewers, session \in Sessions, target \in Targets, epoch \in 1..2:
       Control(v, session, target, epoch)
  \/ \E target \in Targets: Dispose(target)
  \/ ReplaceWorker
Spec == Init /\ [][Next]_vars /\
  \A v \in Viewers: WF_vars(ReleaseController(v)) /\ WF_vars(ReleaseViewer(v))

TypeOK ==
  /\ s.epoch \in 1..2 /\ s.grant \in [Targets -> Viewers \cup {0}]
  /\ s.disposed \subseteq Targets /\ s.cleanup \subseteq Viewers
  /\ \A v \in Viewers: s.viewer[v].phase \in {"unused", "active", "closing", "retired"}
ExactGrantOwnership == \A target \in Targets: s.grant[target] # 0 =>
  /\ s.viewer[s.grant[target]].target = target
  /\ s.viewer[s.grant[target]].phase \in {"active", "closing"}
ExactControlAdmission == \A effect \in s.accepted: effect.owned
CleanupPrecedesRetirement == \A v \in Viewers:
  s.viewer[v].phase = "retired" => \A target \in Targets: s.grant[target] # v
StaleCleanupPreservesSuccessor == s.staleCleanupSafe
ClosingEventuallyRetires == \A v \in Viewers:
  s.viewer[v].phase = "closing" ~> s.viewer[v].phase = "retired"
ViewerNeverRetargets ==
  [][\A v \in Viewers: s.viewer[v].phase # "unused" =>
    UNCHANGED <<s.viewer[v].session, s.viewer[v].target, s.viewer[v].epoch>>]_vars
=============================================================================
