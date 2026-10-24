---------------------- MODULE SettingsCommitPublication ----------------------
EXTENDS Naturals, FiniteSets

\* Two OS processes share one record lock. Each explicit edit/reset is submitted
\* once. Desktop imports are not admitted. One noncooperating
\* external edit challenges the final source comparison. Concrete schema and
\* filesystem durability remain implementation-test obligations.
Clients == {1, 2}
Keys == {"a", "b"}
Defaults == [k \in Keys |-> 0]
Stages == {"idle", "prepared", "owned", "written", "done"}
VARIABLE s
vars == <<s>>
Init == s = [
  phase |-> [c \in Clients |-> "idle"], owner |-> 0,
  basis |-> [c \in Clients |-> 0], basisValues |-> [c \in Clients |-> Defaults],
  key |-> [c \in Clients |-> "a"], desired |-> [c \in Clients |-> 0],
  version |-> 0, values |-> Defaults, claimed |-> {}, external |-> FALSE,
  writes |-> [c \in Clients |-> 0], receipt |-> [c \in Clients |-> "none"],
  saved |-> [c \in Clients |-> Defaults], savedVersion |-> [c \in Clients |-> 0],
  active |-> [c \in Clients |-> Defaults], commits |-> {}, published |-> {},
  writePreservedOther |-> TRUE]

Fresh(c) == s.basis[c] = s.version
Prepare(c) ==
  /\ s.phase[c] = "idle"
  /\ \E k \in Keys, v \in {0, 2}:
       s' = [s EXCEPT !.phase[c] = "prepared", !.basis[c] = s.version,
         !.key[c] = k, !.desired[c] = v]
Acquire(c) ==
  /\ s.phase[c] = "prepared" /\ s.owner = 0 /\ Fresh(c)
  /\ s' = [s EXCEPT !.phase[c] = "owned", !.owner = c,
       !.basisValues[c] = s.values]
Reject(c) ==
  /\ s.phase[c] \in {"prepared", "owned"}
  /\ s' = [s EXCEPT !.phase[c] = "done",
       !.owner = IF s.owner = c THEN 0 ELSE @,
       !.receipt[c] = IF @ = "unknown" THEN @ ELSE "rejected"]
Write(c) ==
  /\ s.phase[c] = "owned" /\ s.owner = c /\ Fresh(c)
  /\ LET next == [s.basisValues[c] EXCEPT ![s.key[c]] = s.desired[c]] IN
       s' = [s EXCEPT !.phase[c] = "written", !.version = @ + 1,
         !.values = next, !.claimed = @ \cup {s.key[c]}, !.writes[c] = @ + 1,
         !.saved[c] = next, !.savedVersion[c] = s.version + 1,
         !.commits = @ \cup {[version |-> s.version + 1, values |-> next]},
         !.writePreservedOther = \A k \in Keys \ {s.key[c]}: next[k] = s.values[k]]
Publish(c) ==
  /\ s.phase[c] = "written" /\ s.owner = c
  /\ s' = [s EXCEPT !.phase[c] = "done", !.owner = 0,
       !.active[c] = s.saved[c],
       !.published = @ \cup {[version |-> s.savedVersion[c], values |-> s.saved[c]]},
       !.receipt[c] = IF @ = "unknown" THEN @ ELSE "saved"]
ApplyFails(c) ==
  /\ s.phase[c] = "written" /\ s.owner = c
  /\ s' = [s EXCEPT !.phase[c] = "done", !.owner = 0,
       !.receipt[c] = IF @ = "unknown" THEN @ ELSE "saved-not-applied"]
LoseReply(c) ==
  /\ s.phase[c] \in {"prepared", "owned", "written"}
  /\ s.receipt[c] # "unknown"
  /\ s' = [s EXCEPT !.receipt[c] = "unknown"]
ExternalEdit ==
  /\ ~s.external
  /\ s' = [s EXCEPT !.external = TRUE, !.version = @ + 1,
       !.values = [k \in Keys |-> 1]]
Next == ExternalEdit
  \/ (\E c \in Clients: Prepare(c) \/ Acquire(c) \/ Reject(c) \/ Write(c) \/
      Publish(c) \/ ApplyFails(c) \/ LoseReply(c))
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.phase \in [Clients -> Stages] /\ s.owner \in 0..2
  /\ s.version \in 0..3 /\ s.values \in [Keys -> 0..2]
  /\ s.claimed \subseteq Keys /\ s.writes \in [Clients -> 0..1]
OneWriter ==
  /\ (\A c \in Clients: (s.phase[c] \in {"owned", "written"}) <=> s.owner = c)
OtherSettingsPreserved == s.writePreservedOther
PublicationHasCommittedWrite == s.published \subseteq s.commits
PublicationIsComplete == \A p \in s.published: p.values \in [Keys -> 0..2]
PublicationIdentity == \A a,b \in s.published: a.version = b.version => a.values = b.values
SavedRequiresWrite == \A c \in Clients:
  s.receipt[c] \in {"saved", "saved-not-applied"} => s.writes[c] = 1
UnknownNeverReplays == \A c \in Clients: s.writes[c] <= 1
UnknownIsTerminalEvidence ==
  [][\A c \in Clients: s.receipt[c] = "unknown" => s'.receipt[c] = "unknown"]_vars
=============================================================================
