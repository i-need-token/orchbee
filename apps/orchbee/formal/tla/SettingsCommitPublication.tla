---------------------- MODULE SettingsCommitPublication ----------------------
EXTENDS Naturals, FiniteSets

\* Two independently prepared saves, one Server writer, and one external edit.
\* Complete payloads are atoms. Editor visibility, parsing, and filesystem
\* replacement mechanics are implementation-test obligations, not this protocol.
Clients == {1, 2}
Stages == {"idle", "prepared", "owned", "written", "done"}
VARIABLE s
vars == <<s>>
Init == s = [
  phase |-> [c \in Clients |-> "idle"],
  basis |-> [c \in Clients |-> 0],
  basisRevision |-> [c \in Clients |-> 0],
  basisOther |-> [c \in Clients |-> 0],
  owner |-> 0, version |-> 0, revision |-> 0, payload |-> 0, other |-> 0,
  active |-> 0, external |-> FALSE,
  writes |-> [c \in Clients |-> 0],
  receipt |-> [c \in Clients |-> "none"],
  published |-> {}, writePreservedOther |-> TRUE]

Fresh(c) == s.basis[c] = s.version /\ s.basisRevision[c] = s.revision
Prepare(c) ==
  /\ s.phase[c] = "idle"
  /\ s' = [s EXCEPT !.phase[c] = "prepared", !.basis[c] = s.version,
       !.basisRevision[c] = s.revision, !.basisOther[c] = s.other]
Acquire(c) ==
  /\ s.phase[c] = "prepared" /\ s.owner = 0 /\ Fresh(c)
  /\ s' = [s EXCEPT !.phase[c] = "owned", !.owner = c]
Reject(c) ==
  /\ s.phase[c] \in {"prepared", "owned"}
  /\ s' = [s EXCEPT !.phase[c] = "done",
       !.owner = IF s.owner = c THEN 0 ELSE @,
       !.receipt[c] = IF @ = "unknown" THEN @ ELSE "rejected"]
Write(c) ==
  /\ s.phase[c] = "owned" /\ s.owner = c /\ Fresh(c)
  /\ s' = [s EXCEPT !.phase[c] = "written", !.version = @ + 1,
       !.payload = c, !.other = s.basisOther[c], !.writes[c] = @ + 1,
       !.writePreservedOther = s.basisOther[c] = s.other]
Publish(c) ==
  /\ s.phase[c] = "written" /\ s.owner = c
  /\ s' = [s EXCEPT !.phase[c] = "done", !.owner = 0,
       !.revision = @ + 1, !.active = c,
       !.published = @ \cup {[revision |-> s.revision + 1, payload |-> c]},
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
       !.payload = 0, !.other = 1]
Next == ExternalEdit \/ \E c \in Clients:
  Prepare(c) \/ Acquire(c) \/ Reject(c) \/ Write(c) \/
  Publish(c) \/ ApplyFails(c) \/ LoseReply(c)
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.phase \in [Clients -> Stages]
  /\ s.owner \in Clients \cup {0}
  /\ s.version \in 0..3 /\ s.revision \in 0..2
  /\ s.payload \in 0..2 /\ s.active \in 0..2 /\ s.other \in 0..1
  /\ s.writes \in [Clients -> 0..1]
OneWriter == \A c \in Clients:
  (s.phase[c] \in {"owned", "written"}) <=> s.owner = c
OtherSettingsPreserved == s.writePreservedOther
PublicationHasCommittedWrite == s.active # 0 => s.writes[s.active] = 1
PublicationIsComplete == s.revision # 0 =>
  [revision |-> s.revision, payload |-> s.active] \in s.published
PublicationIdentity == \A a,b \in s.published:
  a.revision = b.revision => a.payload = b.payload
SavedRequiresWrite == \A c \in Clients:
  s.receipt[c] \in {"saved", "saved-not-applied"} => s.writes[c] = 1
UnknownNeverReplays == \A c \in Clients: s.writes[c] <= 1
UnknownIsTerminalEvidence ==
  [][\A c \in Clients: s.receipt[c] = "unknown" => s'.receipt[c] = "unknown"]_vars
=============================================================================
