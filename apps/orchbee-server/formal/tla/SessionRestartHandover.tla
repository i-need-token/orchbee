-------------------- MODULE SessionRestartHandover --------------------
\* Server and Worker restart by make before break: the old Server So and its Worker Ow, the
\* successor Server Sn and its Worker Wn, one Terminal Host session, the Session store lock, the
\* Session record with its successor entry, a lossy channel between Sn and So, So's lease, and the
\* next activation. Every process may crash or hang at any step. A hung or suspended process never
\* acts again and ends only through a handle opened on its verified identity, its process id and
\* creation time; a recorded identity that matches no live process is gone and nothing is ended.
\* A process Sn created but had not yet recorded is an orphan, as for every Session child: it never
\* runs and holds nothing, so the model leaves it out.
EXTENDS Naturals
CONSTANTS MaxFaults, MaxReplacements

Procs == {"So", "Ow", "Sn", "Wn"}
Alive == {"run", "hung", "suspended"}
Messages == {"prepare", "prepared", "commit", "abort"}

\* `record.server` names the recorded Server ("Cold" is the generation the next activation
\* publishes); `record.successor` is the successor entry naming Sn, and `record.worker` whether that
\* entry also names Wn. `committed` remembers that a commit publish named Sn. `cold` is the pair the
\* next activation starts. `exited` says that So ended by the protocol.
VARIABLES proc, record, lock, session, so, sn, served, msgs, replacements, faults, exited,
          committed, cold
vars == <<proc, record, lock, session, so, sn, served, msgs, replacements, faults, exited,
          committed, cold>>

Init ==
  /\ proc = [p \in Procs |-> IF p = "Wn" THEN "none" ELSE "run"]
  /\ record = [server |-> "So", successor |-> TRUE, worker |-> FALSE]
  /\ lock = "So" /\ session = "Ow" /\ so = "serving" /\ sn = "checking" /\ served = TRUE
  /\ msgs = {} /\ replacements = 0 /\ faults = 0 /\ exited = FALSE /\ committed = FALSE
  /\ cold = "none"

\* The processes in `P` end: through a handle, by a crash, or by their own exit. A process that is
\* already gone stays as it is, and whatever an ended process held is free.
Ends(P) ==
  /\ proc' = [p \in Procs |-> IF p \in P /\ proc[p] \in Alive THEN "dead" ELSE proc[p]]
  /\ lock' = IF lock \in P THEN "none" ELSE lock
  /\ session' = IF session \in P THEN "none" ELSE session

\* The successor processes the entry names.
Named == IF ~record.successor THEN {} ELSE IF record.worker THEN {"Sn", "Wn"} ELSE {"Sn"}
NoEntry == [record EXCEPT !.successor = FALSE, !.worker = FALSE]

\* Check: Sn creates Wn suspended and, under the record writer lock, adds Wn's identity to the
\* successor entry while the entry still names this ticket and So is the recorded Server; only
\* then does Wn run.
SnCreatesWorker ==
  /\ sn = "checking" /\ proc["Sn"] = "run" /\ proc["Wn"] = "none"
  /\ record.successor /\ record.server = "So"
  /\ proc' = [proc EXCEPT !["Wn"] = "suspended"]
  /\ record' = [record EXCEPT !.worker = TRUE]
  /\ sn' = "created"
  /\ UNCHANGED <<lock, session, so, served, msgs, replacements, faults, exited, committed, cold>>
SnResumesWorker ==
  /\ sn = "created" /\ proc["Sn"] = "run"
  /\ proc' = [proc EXCEPT !["Wn"] = IF @ = "suspended" THEN "run" ELSE @]
  /\ sn' = "ready"
  /\ UNCHANGED <<record, lock, session, so, served, msgs, replacements, faults, exited,
                  committed, cold>>
SnAsks ==
  /\ sn = "ready" /\ proc["Sn"] = "run"
  /\ msgs' = msgs \cup {"prepare"} /\ sn' = "asked"
  /\ UNCHANGED <<proc, record, lock, session, so, served, replacements, faults, exited,
                  committed, cold>>
SnLearnsPrepared ==
  /\ sn = "asked" /\ proc["Sn"] = "run" /\ "prepared" \in msgs
  /\ msgs' = msgs \ {"prepared"} /\ sn' = "prepared"
  /\ UNCHANGED <<proc, record, lock, session, so, served, replacements, faults, exited,
                  committed, cold>>
SnTakesStore ==
  /\ sn = "prepared" /\ proc["Sn"] = "run" /\ lock = "none"
  /\ lock' = "Sn" /\ sn' = "stored"
  /\ UNCHANGED <<proc, record, session, so, served, msgs, replacements, faults, exited,
                  committed, cold>>
\* `AdoptHost`: the Host admits Wn only while its single Worker session is free.
SnAdopts ==
  /\ sn = "stored" /\ proc["Sn"] = "run" /\ proc["Wn"] = "run" /\ session = "none"
  /\ session' = "Wn" /\ sn' = "adopted"
  /\ UNCHANGED <<proc, record, lock, so, served, msgs, replacements, faults, exited, committed,
                  cold>>
\* The commit point: one publish under the record writer lock names Sn and Wn, only while the
\* successor entry still names this ticket and So is the recorded Server.
SnCommits ==
  /\ sn = "adopted" /\ proc["Sn"] = "run"
  /\ record.successor /\ record.server = "So"
  /\ record' = [server |-> "Sn", successor |-> FALSE, worker |-> FALSE]
  /\ committed' = TRUE /\ sn' = "committed"
  /\ UNCHANGED <<proc, lock, session, so, served, msgs, replacements, faults, exited, cold>>
SnReleasesOld ==
  /\ sn = "committed" /\ proc["Sn"] = "run"
  /\ msgs' = msgs \cup {"commit"} /\ sn' = "released"
  /\ UNCHANGED <<proc, record, lock, session, so, served, replacements, faults, exited,
                  committed, cold>>
\* So still runs after `commit`: Sn ends it through the handle it opened at Check.
SnEndsOld ==
  /\ sn = "released" /\ proc["Sn"] = "run" /\ proc["So"] \in Alive
  /\ Ends({"So"})
  /\ UNCHANGED <<record, so, sn, served, msgs, replacements, faults, exited, committed, cold>>
\* The lifetime lock is free once So is gone; Sn takes it and serves.
SnOpens ==
  /\ sn = "released" /\ proc["Sn"] = "run" /\ proc["So"] \notin Alive
  /\ sn' = "open"
  /\ UNCHANGED <<proc, record, lock, session, so, served, msgs, replacements, faults, exited,
                  committed, cold>>
\* Any failure before the commit (a refused or lost `prepare`, a store or Host taken back, its
\* budget): Sn sends `abort` once it has asked, ends Wn through its own handle and exits.
SnGivesUp ==
  /\ proc["Sn"] = "run"
  /\ sn \in {"checking", "created", "ready", "asked", "prepared", "stored", "adopted"}
  /\ msgs' = IF sn \in {"checking", "created", "ready"} THEN msgs ELSE msgs \cup {"abort"}
  /\ Ends({"Wn", "Sn"})
  /\ sn' = "gone"
  /\ UNCHANGED <<record, so, served, replacements, faults, exited, committed, cold>>

\* `prepare`: the entry names the ticket and Wn, both successor processes match live identities, on
\* which So opens its handles, and So's Worker serves. So holds intents, settles and captures.
Accepted ==
  record.successor /\ record.worker /\ served /\ proc["Sn"] \in Alive /\ proc["Wn"] \in Alive
SoPrepares ==
  /\ so = "serving" /\ proc["So"] = "run" /\ "prepare" \in msgs /\ Accepted
  /\ msgs' = msgs \ {"prepare"} /\ so' = "held"
  /\ UNCHANGED <<proc, record, lock, session, sn, served, replacements, faults, exited,
                  committed, cold>>
SoRefuses ==
  /\ so = "serving" /\ proc["So"] = "run" /\ "prepare" \in msgs /\ ~Accepted
  /\ msgs' = msgs \ {"prepare"}
  /\ UNCHANGED <<proc, record, lock, session, so, sn, served, replacements, faults, exited,
                  committed, cold>>
SoReleasesStore ==
  /\ so = "held" /\ proc["So"] = "run"
  /\ lock' = "none" /\ so' = "released"
  /\ UNCHANGED <<proc, record, session, sn, served, msgs, replacements, faults, exited,
                  committed, cold>>
\* A failure before its Worker's shutdown (a background program, the settle bound, the capture):
\* So retakes the store it released, lifts the hold and refuses; Sn never learned `prepared`.
SoUndoes ==
  /\ so \in {"held", "released"} /\ proc["So"] = "run"
  /\ so = "released" => lock = "none"
  /\ lock' = "So" /\ so' = "serving"
  /\ UNCHANGED <<proc, record, session, sn, served, msgs, replacements, faults, exited,
                  committed, cold>>
\* The Keep shutdown ends So's Worker, or So ends it through its handle when it hangs; the Host
\* keeps its terminals. So answers `prepared` and its lease starts.
SoReleasesWorker ==
  /\ so = "released" /\ proc["So"] = "run"
  /\ Ends({"Ow"}) /\ served' = FALSE
  /\ msgs' = msgs \cup {"prepared"} /\ so' = "prepared"
  /\ UNCHANGED <<record, sn, replacements, faults, exited, committed, cold>>
\* `commit`: So reads the record under the writer lock and exits only once it names Sn.
SoCommitted ==
  /\ so = "prepared" /\ proc["So"] = "run" /\ "commit" \in msgs
  /\ msgs' = msgs \ {"commit"}
  /\ IF record.server = "Sn"
       THEN /\ Ends({"So"}) /\ so' = "exited" /\ exited' = TRUE
       ELSE UNCHANGED <<proc, lock, session, so, exited>>
  /\ UNCHANGED <<record, sn, served, replacements, faults, committed, cold>>
\* `abort`, the lease, or the end of a successor process: the fence. So holds the record writer
\* lock throughout and publishes last, so a crash inside it leaves the entry naming processes
\* already gone or still live, which the model reaches by a crash before the fence. While the
\* record still names So, So ends Wn, then Sn, through the handles it opened at `prepare` and then
\* removes the entry; once the record names another Server, So exits.
SoFences ==
  /\ so = "prepared" /\ proc["So"] = "run"
  /\ msgs' = msgs \ {"abort"}
  /\ IF record.server = "So"
       THEN /\ record' = NoEntry
            /\ Ends({"Wn", "Sn"}) /\ so' = "retaking" /\ UNCHANGED exited
       ELSE /\ Ends({"So"}) /\ so' = "exited" /\ exited' = TRUE /\ UNCHANGED record
  /\ UNCHANGED <<sn, served, replacements, faults, committed, cold>>
SoRetakesStore ==
  /\ so = "retaking" /\ proc["So"] = "run" /\ lock = "none"
  /\ lock' = "So" /\ so' = "replacing"
  /\ UNCHANGED <<proc, record, session, sn, served, msgs, replacements, faults, exited,
                  committed, cold>>
\* The take-back: a P1a replacement adopts the Host, outside P1a's cap.
SoReplaces ==
  /\ so = "replacing" /\ proc["So"] = "run" /\ session = "none"
  /\ proc' = [proc EXCEPT !["Ow"] = "run"] /\ session' = "Ow" /\ served' = TRUE
  /\ so' = "serving"
  /\ UNCHANGED <<record, lock, sn, msgs, replacements, faults, exited, committed, cold>>
\* P1a: a lost Worker is replaced under the cap.
SoReplacesLost ==
  /\ so = "serving" /\ proc["So"] = "run" /\ ~served /\ proc["Ow"] \notin Alive
  /\ replacements < MaxReplacements /\ session = "none"
  /\ proc' = [proc EXCEPT !["Ow"] = "run"] /\ session' = "Ow" /\ served' = TRUE
  /\ replacements' = replacements + 1
  /\ UNCHANGED <<record, lock, so, sn, msgs, faults, exited, committed, cold>>
\* While no hand-over is held, So treats an entry whose Sn is gone, or that reached no `prepare`
\* within its bound, as abandoned: under the writer lock it ends the processes the entry names by
\* verified identity, Wn first, and then removes the entry. Its stop does the same.
SoClearsAbandoned ==
  /\ so = "serving" /\ proc["So"] = "run" /\ record.successor
  /\ record' = NoEntry
  /\ Ends(Named)
  /\ UNCHANGED <<so, sn, served, msgs, replacements, faults, exited, committed, cold>>
\* A running Worker whose Server is gone exits by itself; a hung or suspended one does not.
WorkerLeaves(w) ==
  /\ proc[w] = "run" /\ proc[IF w = "Ow" THEN "So" ELSE "Sn"] \notin Alive
  /\ Ends({w}) /\ served' = IF w = "Ow" THEN FALSE ELSE served
  /\ UNCHANGED <<record, so, sn, msgs, replacements, faults, exited, committed, cold>>

\* The next start, under the activation lock and the record writer lock, finds the recorded
\* Server and Worker gone: it ends the processes the successor entry names by verified identity,
\* Wn first, and only then publishes a new generation without the entry, adopting the Host. A live
\* successor process never refuses the start.
RecordedWorker == IF record.server = "So" THEN "Ow" ELSE "Wn"
Activate ==
  /\ cold = "none" /\ record.server \in {"So", "Sn"}
  /\ proc[record.server] \notin Alive /\ proc[RecordedWorker] \notin Alive
  /\ LET live == {p \in Named: proc[p] \in Alive}
     IN /\ (IF lock \in live THEN "none" ELSE lock) = "none"
        /\ (IF session \in live THEN "none" ELSE session) = "none"
        /\ proc' = [p \in Procs |-> IF p \in live THEN "dead" ELSE proc[p]]
  /\ lock' = "Cold" /\ session' = "Cold"
  /\ record' = [server |-> "Cold", successor |-> FALSE, worker |-> FALSE]
  /\ cold' = "serving"
  /\ UNCHANGED <<so, sn, served, msgs, replacements, faults, exited, committed>>

Crash(p) ==
  /\ faults < MaxFaults /\ proc[p] \in Alive
  /\ Ends({p}) /\ faults' = faults + 1
  /\ served' = IF p = "Ow" THEN FALSE ELSE served
  /\ UNCHANGED <<record, so, sn, msgs, replacements, exited, committed, cold>>
Hang(p) ==
  /\ faults < MaxFaults /\ proc[p] = "run"
  /\ proc' = [proc EXCEPT ![p] = "hung"] /\ faults' = faults + 1
  /\ UNCHANGED <<record, lock, session, so, sn, served, msgs, replacements, exited, committed,
                  cold>>
Lose(m) ==
  /\ msgs' = msgs \ {m}
  /\ UNCHANGED <<proc, record, lock, session, so, sn, served, replacements, faults, exited,
                  committed, cold>>
\* A late or stray `commit` or `abort`, such as one a requester that retried sends.
Stray(m) ==
  /\ faults < MaxFaults /\ m \notin msgs
  /\ msgs' = msgs \cup {m} /\ faults' = faults + 1
  /\ UNCHANGED <<proc, record, lock, session, so, sn, served, replacements, exited, committed,
                  cold>>

Progress ==
  \/ SnCreatesWorker \/ SnResumesWorker \/ SnAsks \/ SnLearnsPrepared \/ SnTakesStore
  \/ SnAdopts \/ SnCommits \/ SnReleasesOld \/ SnEndsOld \/ SnOpens \/ SnGivesUp
  \/ SoPrepares \/ SoRefuses \/ SoReleasesStore \/ SoUndoes \/ SoReleasesWorker \/ SoCommitted
  \/ SoFences \/ SoRetakesStore \/ SoReplaces \/ SoReplacesLost \/ SoClearsAbandoned
  \/ \E w \in {"Ow", "Wn"}: WorkerLeaves(w)
  \/ Activate
Next ==
  \/ Progress
  \/ \E p \in Procs: Crash(p) \/ Hang(p)
  \/ \E m \in msgs: Lose(m)
  \/ \E m \in {"commit", "abort"}: Stray(m)
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ proc \in [Procs -> {"none", "run", "hung", "suspended", "dead"}]
  /\ record \in [server: {"So", "Sn", "Cold"}, successor: BOOLEAN, worker: BOOLEAN]
  /\ lock \in {"none", "So", "Sn", "Cold"} /\ session \in {"none", "Ow", "Wn", "Cold"}
  /\ so \in {"serving", "held", "released", "prepared", "retaking", "replacing", "exited"}
  /\ sn \in {"checking", "created", "ready", "asked", "prepared", "stored", "adopted",
             "committed", "released", "open", "gone"}
  /\ served \in BOOLEAN /\ msgs \subseteq Messages
  /\ replacements \in 0..MaxReplacements /\ faults \in 0..MaxFaults
  /\ exited \in BOOLEAN /\ committed \in BOOLEAN /\ cold \in {"none", "serving"}

SoAdmits == so = "serving" /\ proc["So"] = "run" /\ served
SnAdmits == sn = "open" /\ proc["Sn"] = "run" /\ proc["Wn"] \in Alive
\* A Server admits intents only while it is the recorded Server, owns the store and its Worker
\* holds the Host session. A Worker accepts input only for a Server that admits it, and no action
\* moves an accepted input to another Worker, so an input one Worker accepted is never delivered
\* by another.
AdmitsOnlyRecordedOwner ==
  /\ SoAdmits => record.server = "So" /\ lock = "So" /\ session = "Ow"
  /\ SnAdmits => record.server = "Sn" /\ lock = "Sn" /\ session = "Wn"
SingleOwner ==
  /\ ~(SoAdmits /\ SnAdmits)
  /\ cold = "serving" => ~SoAdmits /\ ~SnAdmits
\* So exits by protocol only after the record named Sn.
ExitOnlyAfterCommit == exited => committed
\* A live successor process is always one the record names, in the entry or as the recorded pair,
\* so whoever finds the entry can end it: nobody loses track of a Wn that never ran or that hangs.
SuccessorTracked ==
  \A p \in {"Sn", "Wn"}: proc[p] \in Alive => p \in Named \/ record.server = "Sn"
\* A start never adopts beside a process the successor entry named.
AdoptionAlone == cold = "serving" => proc["Sn"] \notin Alive /\ proc["Wn"] \notin Alive

\* How a behavior may end: the old pair serving, the new pair serving, or the pair the next
\* activation started. Besides these, only failures the restart does not add: a hung process the
\* record names as its Server or Worker, which an activation refuses as an uncertain owner and the
\* person's stop ends as for any hung Session process today; a Server that gave up at P1a's cap;
\* and a new Worker lost after the commit, which the new Server replaces through P1a. A process
\* only the successor entry names is never such a failure: someone must end it.
OldServing == SoAdmits /\ proc["Sn"] \notin Alive /\ proc["Wn"] \notin Alive
NewServing == SnAdmits /\ proc["So"] \notin Alive
RecordedPair == IF record.server = "So" THEN {"So", "Ow"}
                ELSE IF record.server = "Sn" THEN {"Sn", "Wn"} ELSE {}
Unresolved == \E p \in RecordedPair: proc[p] = "hung"
GivenUp ==
  /\ so = "serving" /\ proc["So"] = "run" /\ ~served /\ replacements = MaxReplacements
  /\ proc["Sn"] \notin Alive /\ proc["Wn"] \notin Alive
LostNewWorker == sn = "open" /\ proc["Sn"] = "run" /\ proc["Wn"] = "dead"
GoodEnd ==
  OldServing \/ NewServing \/ cold = "serving" \/ Unresolved \/ GivenUp \/ LostNewWorker
\* Every behavior ends well: wherever no protocol step can follow, whatever faults remain.
EndsWell == ~ENABLED Progress => GoodEnd
=============================================================================
