---------------------------- MODULE SharedSplitRatios ----------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS Clients, Ratios, Half, MaxRevision
VARIABLES canonical, views, requests, received, interrupted, selected,
          results, drawn, writes, replayed, commits
vars == <<canonical, views, requests, received, interrupted, selected,
          results, drawn, writes, replayed, commits>>

EmptyRequest == [phase |-> "idle", source |-> [site |-> 1, revision |-> 0,
                 incarnation |-> 1, ratio |-> Half], ratio |-> Half,
                 response |-> [site |-> 1, revision |-> 0,
                 incarnation |-> 1, ratio |-> Half]]
Init ==
  /\ canonical = [site |-> 1, revision |-> 0, incarnation |-> 1, ratio |-> Half]
  /\ views = [c \in Clients |-> canonical]
  /\ requests = [c \in Clients |-> EmptyRequest]
  /\ received = [c \in Clients |-> FALSE]
  /\ interrupted = [c \in Clients |-> FALSE]
  /\ selected \in [Clients -> BOOLEAN]
  /\ results = [c \in Clients |-> "none"]
  /\ drawn = [c \in Clients |-> canonical]
  /\ writes = [c \in Clients |-> 0]
  /\ replayed = [c \in Clients |-> FALSE]
  /\ commits = <<>>

Prepare(c, ratio) ==
  /\ requests[c].phase = "idle"
  /\ requests' = [requests EXCEPT ![c].phase = "prepared",
                   ![c].source = views[c], ![c].ratio = ratio]
  /\ UNCHANGED <<canonical, views, received, interrupted, selected,
                  results, drawn, writes, replayed, commits>>
Submit(c) ==
  /\ requests[c].phase = "prepared"
  /\ ~interrupted[c]
  /\ requests' = [requests EXCEPT ![c].phase = "submitted"]
  /\ UNCHANGED <<canonical, views, received, interrupted, selected,
                  results, drawn, writes, replayed, commits>>
Eligible(c) ==
  /\ requests[c].source = canonical
  /\ requests[c].ratio = canonical.ratio \/ canonical.revision < MaxRevision
BackendCommit(c) ==
  /\ requests[c].phase = "submitted"
  /\ Eligible(c)
  /\ LET changed == requests[c].ratio # canonical.ratio
         next == [canonical EXCEPT !.ratio = requests[c].ratio,
                   !.revision = @ + IF changed THEN 1 ELSE 0]
     IN /\ canonical' = next
        /\ requests' = [requests EXCEPT ![c].phase = "committed", ![c].response = next]
        /\ writes' = [writes EXCEPT ![c] = @ + IF changed THEN 1 ELSE 0]
        /\ commits' = IF changed THEN Append(commits,
              [client |-> c, source |-> requests[c].source, before |-> canonical,
               after |-> next]) ELSE commits
  /\ UNCHANGED <<views, received, interrupted, selected, results, drawn, replayed>>
BackendReject(c) ==
  /\ requests[c].phase = "submitted"
  /\ ~Eligible(c)
  /\ requests' = [requests EXCEPT ![c].phase = "rejected"]
  /\ results' = [results EXCEPT ![c] = IF interrupted[c] THEN "incomplete" ELSE "rejected"]
  /\ UNCHANGED <<canonical, views, received, interrupted, selected,
                  drawn, writes, replayed, commits>>
Replay(c) ==
  /\ requests[c].phase \in {"committed", "rejected"}
  /\ ~replayed[c]
  /\ replayed' = [replayed EXCEPT ![c] = TRUE]
  /\ UNCHANGED <<canonical, views, requests, received, interrupted, selected,
                  results, drawn, writes, commits>>
Receive(c) ==
  /\ requests[c].phase = "committed"
  /\ ~received[c]
  /\ received' = [received EXCEPT ![c] = TRUE]
  /\ UNCHANGED <<canonical, views, requests, interrupted, selected,
                  results, drawn, writes, replayed, commits>>
LoseReply(c) ==
  /\ requests[c].phase = "committed"
  /\ ~received[c]
  /\ requests' = [requests EXCEPT ![c].phase = "unknown"]
  /\ results' = [results EXCEPT ![c] = "incomplete"]
  /\ UNCHANGED <<canonical, views, received, interrupted, selected,
                  drawn, writes, replayed, commits>>
Publish(c) ==
  /\ views[c] # canonical
  /\ views' = [views EXCEPT ![c] = canonical]
  /\ UNCHANGED <<canonical, requests, received, interrupted, selected,
                  results, drawn, writes, replayed, commits>>
SameTarget(view, response) ==
  /\ view.site = response.site /\ view.incarnation = response.incarnation
  /\ view.ratio = response.ratio /\ view.revision >= response.revision
Draw(c) ==
  /\ requests[c].phase = "committed" /\ received[c]
  /\ selected[c] /\ ~interrupted[c] /\ results[c] = "none"
  /\ SameTarget(views[c], requests[c].response) /\ views[c].site = canonical.site
  /\ drawn' = [drawn EXCEPT ![c] = views[c]]
  /\ results' = [results EXCEPT ![c] = "success"]
  /\ UNCHANGED <<canonical, views, requests, received, interrupted, selected,
                  writes, replayed, commits>>
CompleteInactive(c) ==
  /\ requests[c].phase = "committed" /\ received[c]
  /\ ~selected[c] /\ ~interrupted[c] /\ results[c] = "none"
  /\ results' = [results EXCEPT ![c] = "success"]
  /\ UNCHANGED <<canonical, views, requests, received, interrupted, selected,
                  drawn, writes, replayed, commits>>
Interrupt(c) ==
  /\ requests[c].phase \in {"prepared", "submitted", "committed"}
  /\ ~interrupted[c] /\ results[c] = "none"
  /\ interrupted' = [interrupted EXCEPT ![c] = TRUE]
  /\ results' = [results EXCEPT ![c] = IF requests[c].phase = "prepared"
                   THEN "rejected" ELSE "incomplete"]
  /\ UNCHANGED <<canonical, views, requests, received, selected,
                  drawn, writes, replayed, commits>>
ReplaceSplit ==
  /\ canonical.incarnation = 1 /\ canonical.revision < MaxRevision
  /\ canonical' = [canonical EXCEPT !.incarnation = 2, !.ratio = Half, !.revision = @ + 1]
  /\ UNCHANGED <<views, requests, received, interrupted, selected,
                  results, drawn, writes, replayed, commits>>
OtherMetadata ==
  /\ canonical.revision < MaxRevision
  /\ canonical' = [canonical EXCEPT !.revision = @ + 1]
  /\ UNCHANGED <<views, requests, received, interrupted, selected,
                  results, drawn, writes, replayed, commits>>
Finish == \E c \in Clients : BackendCommit(c) \/ BackendReject(c)
Next == ReplaceSplit \/ OtherMetadata \/
  \E c \in Clients : (\E r \in Ratios : Prepare(c, r)) \/ Submit(c) \/
    BackendCommit(c) \/ BackendReject(c) \/ Replay(c) \/ Receive(c) \/ LoseReply(c) \/
    Publish(c) \/ Draw(c) \/ CompleteInactive(c) \/ Interrupt(c)
Spec == Init /\ [][Next]_vars /\ WF_vars(Finish)

CanonicalType == [site : {1}, revision : 0..MaxRevision, incarnation : 1..2, ratio : Ratios]
TypeOK == /\ canonical \in CanonicalType /\ views \in [Clients -> CanonicalType]
  /\ selected \in [Clients -> BOOLEAN] /\ interrupted \in [Clients -> BOOLEAN]
  /\ received \in [Clients -> BOOLEAN] /\ writes \in [Clients -> 0..1]
  /\ results \in [Clients -> {"none", "success", "rejected", "incomplete"}]
AtMostOneWrite == \A c \in Clients : writes[c] <= 1
CommitIsFenced == \A i \in 1..Len(commits) :
  /\ commits[i].source = commits[i].before
  /\ commits[i].after.site = commits[i].before.site
  /\ commits[i].after.incarnation = commits[i].before.incarnation
  /\ commits[i].after.revision = commits[i].before.revision + 1
SuccessIsHonest == \A c \in Clients : results[c] = "success" =>
  /\ requests[c].phase = "committed" /\ received[c] /\ ~interrupted[c]
  /\ selected[c] => SameTarget(drawn[c], requests[c].response)
UnknownDoesNotSucceed == \A c \in Clients : requests[c].phase = "unknown" =>
  results[c] = "incomplete"
SubmittedSettles == \A c \in Clients : requests[c].phase = "submitted" ~>
  requests[c].phase \in {"committed", "rejected", "unknown"}
=============================================================================
