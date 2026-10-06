---------------- MODULE NativeSettingsSave ----------------
EXTENDS Naturals, FiniteSets, TLC

\* Complete admitted payloads are atoms. Source generations abstract exact
\* content/absence tokens. No filesystem atomicity or delivery fairness is inferred.
CONSTANT MaxEpisodes, Clients
ASSUME MaxEpisodes = 2 /\ Cardinality(Clients) = 2
ClientSymmetry == Permutations(Clients)
Payloads == {0, 1}
Stages == {"hidden", "open", "submitted", "prepared", "saved", "done"}
Outcomes == {"none", "rejected", "no-op", "saved", "saved-not-applied", "unknown"}
VARIABLES fileVersion, filePayload, fileOther, revision, active,
  stage, draft, basisVersion, basisRevision, basisOther, episode, drawn,
  visible, submissions, writes, wrote, outcome, owner, lastWriteVersion,
  lastWriteOther, published
vars == <<fileVersion, filePayload, fileOther, revision, active,
  stage, draft, basisVersion, basisRevision, basisOther, episode, drawn,
  visible, submissions, writes, wrote, outcome, owner, lastWriteVersion,
  lastWriteOther, published>>

TypeOK ==
  /\ fileVersion \in 0..3 /\ filePayload \in Payloads /\ fileOther \in Payloads
  /\ revision \in 0..3 /\ active \in Payloads
  /\ stage \in [Clients -> Stages] /\ draft \in [Clients -> Payloads]
  /\ basisVersion \in [Clients -> 0..3] /\ basisRevision \in [Clients -> 0..3]
  /\ basisOther \in [Clients -> Payloads] /\ episode \in [Clients -> 0..MaxEpisodes]
  /\ drawn \in [Clients -> BOOLEAN] /\ visible \in [Clients -> BOOLEAN]
  /\ submissions \in [Clients -> 0..MaxEpisodes] /\ writes \in [Clients -> 0..MaxEpisodes]
  /\ wrote \in [Clients -> BOOLEAN] /\ outcome \in [Clients -> Outcomes]
  /\ owner \in Clients \cup {"none"} /\ lastWriteVersion \in 0..3
  /\ lastWriteOther \in Payloads /\ published \subseteq [rev: 0..3, payload: Payloads]
SingleSubmission == \A c \in Clients: writes[c] <= submissions[c] /\ submissions[c] <= episode[c]
OneWriter == owner # "none" => stage[owner] \in {"prepared", "saved"}
OtherSettingsPreserved == fileVersion = lastWriteVersion => fileOther = lastWriteOther
ActiveIsPublished == [rev |-> revision, payload |-> active] \in published
ImmutablePublication == \A x,y \in published: x.rev = y.rev => x.payload = y.payload
SavedReceiptRequiresWrite == \A c \in Clients: outcome[c] \in {"saved", "saved-not-applied"} => wrote[c]

Init ==
  /\ fileVersion = 0 /\ filePayload = 0 /\ fileOther = 0
  /\ revision = 0 /\ active = 0 /\ owner = "none"
  /\ lastWriteVersion = 0 /\ lastWriteOther = 0
  /\ published = {[rev |-> 0, payload |-> 0]}
  /\ stage = [c \in Clients |-> "hidden"] /\ draft = [c \in Clients |-> 0]
  /\ basisVersion = [c \in Clients |-> 0] /\ basisRevision = [c \in Clients |-> 0]
  /\ basisOther = [c \in Clients |-> 0] /\ episode = [c \in Clients |-> 0]
  /\ drawn = [c \in Clients |-> FALSE] /\ visible = [c \in Clients |-> FALSE]
  /\ submissions = [c \in Clients |-> 0] /\ writes = [c \in Clients |-> 0]
  /\ wrote = [c \in Clients |-> FALSE] /\ outcome = [c \in Clients |-> "none"]

Open == \E c \in Clients:
  /\ stage[c] \in {"hidden", "done"} /\ episode[c] < MaxEpisodes
  /\ stage' = [stage EXCEPT ![c] = "open"]
  /\ draft' = [draft EXCEPT ![c] = active]
  /\ basisVersion' = [basisVersion EXCEPT ![c] = fileVersion]
  /\ basisRevision' = [basisRevision EXCEPT ![c] = revision]
  /\ basisOther' = [basisOther EXCEPT ![c] = fileOther]
  /\ episode' = [episode EXCEPT ![c] = @ + 1]
  /\ drawn' = [drawn EXCEPT ![c] = FALSE]
  /\ visible' = [visible EXCEPT ![c] = TRUE]
  /\ wrote' = [wrote EXCEPT ![c] = FALSE]
  /\ outcome' = [outcome EXCEPT ![c] = "none"]
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,submissions,writes,
                 owner,lastWriteVersion,lastWriteOther,published>>
Edit == \E c \in Clients, value \in Payloads:
  /\ stage[c] = "open" /\ visible[c]
  /\ draft' = [draft EXCEPT ![c] = value]
  /\ drawn' = [drawn EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,stage,basisVersion,
      basisRevision,basisOther,episode,visible,submissions,writes,wrote,outcome,owner,
      lastWriteVersion,lastWriteOther,published>>
Draw == \E c \in Clients:
  /\ stage[c] = "open" /\ visible[c] /\ ~drawn[c]
  /\ drawn' = [drawn EXCEPT ![c] = TRUE]
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,stage,draft,basisVersion,
      basisRevision,basisOther,episode,visible,submissions,writes,wrote,outcome,owner,
      lastWriteVersion,lastWriteOther,published>>
Submit == \E c \in Clients:
  /\ stage[c] = "open" /\ visible[c] /\ drawn[c] /\ submissions[c] < episode[c]
  /\ stage' = [stage EXCEPT ![c] = "submitted"]
  /\ submissions' = [submissions EXCEPT ![c] = @ + 1]
  /\ drawn' = [drawn EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,draft,basisVersion,
      basisRevision,basisOther,episode,visible,writes,wrote,outcome,owner,
      lastWriteVersion,lastWriteOther,published>>
Fresh(c) == basisVersion[c] = fileVersion /\ basisRevision[c] = revision
Receipt(c, value) == IF outcome[c] = "unknown" THEN "unknown" ELSE value
\* Completed episodes no longer own their draft/source observation.
ClearDraft(c) ==
  /\ draft' = [draft EXCEPT ![c] = 0]
  /\ basisVersion' = [basisVersion EXCEPT ![c] = 0]
  /\ basisRevision' = [basisRevision EXCEPT ![c] = 0]
  /\ basisOther' = [basisOther EXCEPT ![c] = 0]
  /\ drawn' = [drawn EXCEPT ![c] = FALSE]
  /\ visible' = [visible EXCEPT ![c] = FALSE]
Admit == \E c \in Clients:
  /\ stage[c] = "submitted" /\ owner = "none" /\ Fresh(c)
  /\ fileVersion < 3 /\ revision < 3
  /\ stage' = [stage EXCEPT ![c] = "prepared"] /\ owner' = c
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,draft,basisVersion,
      basisRevision,basisOther,episode,drawn,visible,submissions,writes,wrote,outcome,
      lastWriteVersion,lastWriteOther,published>>
Reject == \E c \in Clients:
  /\ stage[c] = "submitted"
  /\ stage' = [stage EXCEPT ![c] = "done"]
  /\ outcome' = [outcome EXCEPT ![c] = Receipt(c,"rejected")]
  /\ ClearDraft(c)
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,episode,submissions,writes,wrote,owner,lastWriteVersion,lastWriteOther,published>>
FinishBeforeWrite(c, code) ==
  /\ owner = c /\ stage[c] = "prepared"
  /\ stage' = [stage EXCEPT ![c] = "done"] /\ owner' = "none"
  /\ outcome' = [outcome EXCEPT ![c] = Receipt(c,code)]
  /\ ClearDraft(c)
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,episode,submissions,writes,wrote,lastWriteVersion,lastWriteOther,published>>
SourceConflict == \E c \in Clients: /\ ~Fresh(c) /\ FinishBeforeWrite(c,"rejected")
WriteFails == \E c \in Clients: /\ Fresh(c) /\ FinishBeforeWrite(c,"rejected")
NoOp == \E c \in Clients:
  /\ Fresh(c) /\ draft[c] = filePayload /\ draft[c] = active
  /\ FinishBeforeWrite(c,"no-op")
WriteFile == \E c \in Clients:
  /\ owner = c /\ stage[c] = "prepared" /\ Fresh(c) /\ fileVersion < 3
  /\ fileVersion' = fileVersion + 1 /\ filePayload' = draft[c]
  /\ fileOther' = basisOther[c]
  /\ lastWriteVersion' = fileVersion + 1 /\ lastWriteOther' = fileOther
  /\ stage' = [stage EXCEPT ![c] = "saved"]
  /\ writes' = [writes EXCEPT ![c] = @ + 1] /\ wrote' = [wrote EXCEPT ![c] = TRUE]
  /\ UNCHANGED <<revision,active,draft,basisVersion,basisRevision,basisOther,episode,
      drawn,visible,submissions,outcome,owner,published>>
Publish == \E c \in Clients:
  /\ owner = c /\ stage[c] = "saved" /\ revision < 3
  /\ revision' = revision + 1 /\ active' = draft[c]
  /\ published' = published \cup {[rev |-> revision + 1, payload |-> draft[c]]}
  /\ stage' = [stage EXCEPT ![c] = "done"] /\ owner' = "none"
  /\ outcome' = [outcome EXCEPT ![c] = Receipt(c,"saved")]
  /\ ClearDraft(c)
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,episode,submissions,writes,wrote,lastWriteVersion,lastWriteOther>>
ApplyFails == \E c \in Clients:
  /\ owner = c /\ stage[c] = "saved"
  /\ stage' = [stage EXCEPT ![c] = "done"] /\ owner' = "none"
  /\ outcome' = [outcome EXCEPT ![c] = Receipt(c,"saved-not-applied")]
  /\ ClearDraft(c)
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,episode,submissions,writes,wrote,lastWriteVersion,lastWriteOther,published>>
ExternalEdit == \E value \in Payloads:
  /\ fileVersion < 3 /\ fileVersion' = fileVersion + 1
  /\ filePayload' = value /\ fileOther' = 1 - fileOther
  /\ UNCHANGED <<revision,active,stage,draft,basisVersion,basisRevision,basisOther,episode,
      drawn,visible,submissions,writes,wrote,outcome,owner,lastWriteVersion,lastWriteOther,published>>
Reload ==
  /\ owner = "none" /\ revision < 3
  /\ revision' = revision + 1 /\ active' = filePayload
  /\ published' = published \cup {[rev |-> revision + 1, payload |-> filePayload]}
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,stage,draft,basisVersion,basisRevision,
      basisOther,episode,drawn,visible,submissions,writes,wrote,outcome,owner,
      lastWriteVersion,lastWriteOther>>
Cancel == \E c \in Clients:
  /\ stage[c] = "open"
  /\ stage' = [stage EXCEPT ![c] = "hidden"]
  /\ ClearDraft(c)
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,episode,submissions,writes,wrote,outcome,owner,lastWriteVersion,lastWriteOther,published>>
HidePending == \E c \in Clients:
  /\ stage[c] \in {"submitted","prepared","saved"} /\ visible[c]
  /\ visible' = [visible EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,stage,draft,basisVersion,
      basisRevision,basisOther,episode,drawn,submissions,writes,wrote,outcome,owner,
      lastWriteVersion,lastWriteOther,published>>
LoseResponse == \E c \in Clients:
  /\ stage[c] \in {"submitted","prepared","saved"} /\ outcome[c] # "unknown"
  /\ outcome' = [outcome EXCEPT ![c] = "unknown"]
  /\ UNCHANGED <<fileVersion,filePayload,fileOther,revision,active,stage,draft,basisVersion,
      basisRevision,basisOther,episode,drawn,visible,submissions,writes,wrote,owner,
      lastWriteVersion,lastWriteOther,published>>
Next == Open \/ Edit \/ Draw \/ Submit \/ Admit \/ Reject \/ SourceConflict \/ WriteFails
     \/ NoOp \/ WriteFile \/ Publish \/ ApplyFails \/ ExternalEdit \/ Reload
     \/ Cancel \/ HidePending \/ LoseResponse
Spec == Init /\ [][Next]_vars
=============================================================================
