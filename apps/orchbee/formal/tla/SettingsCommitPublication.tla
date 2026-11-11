---------------------- MODULE SettingsCommitPublication ----------------------
EXTENDS Integers

\* The Server is the one writer of config.toml that Clients reach. An edit holds
\* the Registry's reload admission while it reads the file, prepares a candidate
\* that changes only its own key, rechecks the file's content (reading again once
\* when it changed), replaces the file, and reloads and publishes under the same
\* admission. The sampler takes that admission to reload a file whose write it has
\* not observed. An external editor ignores both: it may change any key or break
\* the syntax. Recheck and replace are one step, since the real window is a few
\* system calls. TOML bytes, comments, the cross-process lock, unreadable files,
\* live-policy apply failure and durability stay with the implementation tests.
Clients == {"tui", "desktop"}
Keys == {"a", "b"}
\* Each Client edits its own key, so their writes can only collide through the file.
Key == [tui |-> "a", desktop |-> "b"]
Defaults == [k \in Keys |-> 0]
Valid == [Keys -> 0..2]
Refusals == {"busy", "syntax-error", "changed"}
Phases == {"idle", "waiting", "reading", "checking", "committing", "done"}
Holding == {"reading", "checking", "committing"}
Initial == [revision |-> 0, status |-> "ok", values |-> Defaults]
\* Each Client edits once and the external editor writes twice; every write gives
\* the file a new version, as it gives it a new modification time.
MaxWrites == 4

VARIABLE s
vars == <<s>>

Init == s = [
  file |-> [parses |-> TRUE, values |-> Defaults, version |-> 0], external |-> 0,
  holder |-> "none", observed |-> -1, sampling |-> -1,
  phase |-> [c \in Clients |-> "idle"], desired |-> [c \in Clients |-> 0],
  before |-> [c \in Clients |-> [parses |-> TRUE, values |-> Defaults]],
  redone |-> [c \in Clients |-> FALSE], writes |-> [c \in Clients |-> 0],
  receipt |-> [c \in Clients |-> "none"], reply |-> [c \in Clients |-> Initial],
  current |-> Defaults, snapshot |-> Initial, published |-> {Initial},
  fileValues |-> {Defaults}, otherKeysKept |-> TRUE, brokenWritten |-> FALSE]

\* What the editor's digest compares: the bytes, not the modification time.
Content(f) == [parses |-> f.parses, values |-> f.values]
\* A file that does not parse keeps the last valid values and names the error.
Loaded(f) == IF f.parses
  THEN [status |-> "ok", values |-> f.values]
  ELSE [status |-> "syntax-error", values |-> s.current]
\* The snapshot after loading `f`: the published one when nothing it shows changed.
After(f) == LET l == Loaded(f) IN
  IF l.status = s.snapshot.status /\ l.values = s.snapshot.values THEN s.snapshot
  ELSE [revision |-> s.snapshot.revision + 1, status |-> l.status, values |-> l.values]
NextFile(parses, values) == [parses |-> parses, values |-> values,
                             version |-> s.file.version + 1]
Reloaded == IF s.file.parses THEN s.file.values ELSE s.current

Submit(c) ==
  /\ s.phase[c] = "idle"
  /\ \E v \in {0, 2}: s' = [s EXCEPT !.phase[c] = "waiting", !.desired[c] = v]
Admit(c) ==
  /\ s.phase[c] = "waiting" /\ s.holder = "none"
  /\ s' = [s EXCEPT !.phase[c] = "reading", !.holder = c]
\* The real edit waits a moment for the admission before this refusal.
Busy(c) ==
  /\ s.phase[c] = "waiting" /\ s.holder # "none"
  /\ s' = [s EXCEPT !.phase[c] = "done", !.receipt[c] = "busy"]
RefuseUnparsable(c) ==
  /\ s.phase[c] = "reading" /\ ~s.file.parses
  /\ s' = [s EXCEPT !.phase[c] = "done", !.holder = "none",
       !.receipt[c] = "syntax-error"]
\* A file that already holds the value is left unwritten; the edit still reloads.
Read(c) ==
  /\ s.phase[c] = "reading" /\ s.file.parses
  /\ s' = [s EXCEPT !.before[c] = Content(s.file),
       !.phase[c] = IF s.file.values[Key[c]] = s.desired[c]
                    THEN "committing" ELSE "checking"]
Candidate(c) == [s.before[c].values EXCEPT ![Key[c]] = s.desired[c]]
Replace(c) ==
  /\ s.phase[c] = "checking" /\ Content(s.file) = s.before[c]
  /\ s' = [s EXCEPT !.file = NextFile(TRUE, Candidate(c)),
       !.writes[c] = @ + 1, !.phase[c] = "committing",
       !.fileValues = @ \cup {Candidate(c)},
       !.otherKeysKept = @ /\ s.file.parses /\
         \A k \in Keys \ {Key[c]}: Candidate(c)[k] = s.file.values[k],
       !.brokenWritten = @ \/ ~s.file.parses]
Redo(c) ==
  /\ s.phase[c] = "checking" /\ Content(s.file) # s.before[c] /\ ~s.redone[c]
  /\ s' = [s EXCEPT !.phase[c] = "reading", !.redone[c] = TRUE]
RefuseChanged(c) ==
  /\ s.phase[c] = "checking" /\ Content(s.file) # s.before[c] /\ s.redone[c]
  /\ s' = [s EXCEPT !.phase[c] = "done", !.holder = "none", !.receipt[c] = "changed"]
\* The reload reads the file as it is now, after any external edit since the write.
Commit(c) ==
  /\ s.phase[c] = "committing"
  /\ s' = [s EXCEPT !.phase[c] = "done", !.holder = "none", !.receipt[c] = "edited",
       !.reply[c] = After(s.file), !.snapshot = After(s.file),
       !.published = @ \cup {After(s.file)}, !.current = Reloaded]

\* The sampler records the version it saw before the reload, so a write during the
\* reload is sampled again.
SampleBegin ==
  /\ s.holder = "none" /\ s.observed # s.file.version
  /\ s' = [s EXCEPT !.holder = "sampler", !.sampling = s.file.version]
SampleCommit ==
  /\ s.holder = "sampler"
  /\ s' = [s EXCEPT !.holder = "none", !.observed = s.sampling,
       !.snapshot = After(s.file), !.published = @ \cup {After(s.file)},
       !.current = Reloaded]

ExternalEdit ==
  /\ s.external < 2
  /\ \E v \in [Keys -> 0..1]:
       /\ ~s.file.parses \/ v # s.file.values
       /\ s' = [s EXCEPT !.file = NextFile(TRUE, v), !.external = @ + 1,
            !.fileValues = @ \cup {v}]
ExternalBreak ==
  /\ s.external < 2 /\ s.file.parses
  /\ s' = [s EXCEPT !.file = NextFile(FALSE, s.file.values), !.external = @ + 1]

Step(c) == Admit(c) \/ RefuseUnparsable(c) \/ Read(c) \/ Replace(c) \/ Redo(c) \/
  RefuseChanged(c) \/ Commit(c)
Sample == SampleBegin \/ SampleCommit
Next == ExternalEdit \/ ExternalBreak \/ Sample
  \/ (\E c \in Clients: Submit(c) \/ Busy(c) \/ Step(c))
Spec == Init /\ [][Next]_vars /\ WF_vars(Sample) /\ \A c \in Clients: WF_vars(Step(c))

TypeOK ==
  /\ s.file \in [parses : BOOLEAN, values : Valid, version : 0..MaxWrites]
  /\ s.holder \in Clients \cup {"none", "sampler"}
  /\ s.phase \in [Clients -> Phases]
  /\ s.receipt \in [Clients -> Refusals \cup {"none", "edited"}]
  /\ s.writes \in [Clients -> 0..1]
  /\ s.current \in Valid
  /\ s.snapshot.status \in {"ok", "syntax-error"}
\* An edit reads, writes and reloads only while it holds the reload admission; the
\* sampler and every other edit wait for it or are refused as busy.
OneWriter == \A c \in Clients: s.phase[c] \in Holding <=> s.holder = c
\* A write changes only its own key of the file it replaces, so an external edit that
\* reached the file before the recheck survives the write.
OtherKeysPreserved == s.otherKeysKept
\* A file that does not parse is never replaced.
UnparsableFileNeverWritten == ~s.brokenWritten
\* The current snapshot is the newest published one.
SnapshotIsNewest ==
  /\ s.snapshot \in s.published
  /\ \A p \in s.published: p.revision <= s.snapshot.revision
\* A revision names exactly one published snapshot.
PublicationIdentity == \A p, q \in s.published: p.revision = q.revision => p = q
\* Every published value set was once the whole file: no snapshot mixes two files.
PublishedValuesWereFileStates == \A p \in s.published: p.values \in s.fileValues
\* The published values are the last file that parsed, also while the file shows a
\* syntax error.
SnapshotShowsLastValid == s.snapshot.values = s.current
\* A refused edit writes nothing and publishes nothing.
RefusalChangesNothing ==
  [][\A c \in Clients:
       s.receipt[c] = "none" /\ s'.receipt[c] \in Refusals =>
         s'.file = s.file /\ s'.published = s.published /\ s'.writes[c] = 0]_vars
\* An edit replies with the snapshot its own reload made current, never one taken
\* before its write.
ReplyIsCurrent ==
  [][\A c \in Clients:
       s.receipt[c] = "none" /\ s'.receipt[c] = "edited" => s'.reply[c] = s'.snapshot]_vars
\* A publication advances the revision by exactly one.
RevisionAdvances ==
  [][s'.snapshot # s.snapshot => s'.snapshot.revision = s.snapshot.revision + 1]_vars
\* Once writes stop, the published snapshot shows the file: its values, or its syntax
\* error over the last valid values.
FileEventuallyPublished == <>[](
  IF s.file.parses THEN s.snapshot.status = "ok" /\ s.snapshot.values = s.file.values
  ELSE s.snapshot.status = "syntax-error")
=============================================================================
