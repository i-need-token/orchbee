---------------------- MODULE TlaVerificationExecution ----------------------
EXTENDS Naturals, FiniteSets
CONSTANTS Resume, HistoricalFailure, Heavy, SameInputs, CachedLogIntact,
          CompleteLogs, CompleteCoverage
Jobs == 1..3
Models == {"A", "B"}
Model(j) == IF j = 3 THEN "B" ELSE "A"
Weight(j) == IF j = 2 THEN Heavy ELSE 1
Witness(j) == IF j = 2 THEN {"b"} ELSE {"a"}
Required(m) == IF m = "A" \/ ~CompleteCoverage THEN {"a", "b"} ELSE {"a"}
InitialRecord(j) == IF Resume /\ j = 1 THEN "passed" ELSE
    IF Resume /\ HistoricalFailure /\ j = 2 THEN "failed" ELSE "absent"
RunValid == SameInputs /\ (~Resume \/ CachedLogIntact)
LogComplete(j) == CompleteLogs \/ j # 3
VARIABLES phase, failed, stop, cancelled, validation, reused, records,
          recordCoverage, evidence, temporary, modelResult
vars == <<phase, failed, stop, cancelled, validation, reused, records,
          recordCoverage, evidence, temporary, modelResult>>
Active == {j \in Jobs : phase[j] \in {"active", "ready", "writing"}}
Used == (IF 1 \in Active THEN Weight(1) ELSE 0)
      + (IF 2 \in Active THEN Weight(2) ELSE 0)
      + (IF 3 \in Active THEN Weight(3) ELSE 0)
Fits(j) == Cardinality(Active) < 2 /\
    ((Used + Weight(j) <= 2) \/ (Active = {} /\ Weight(j) > 2))
FirstPending(j) == \A k \in Jobs : k < j => phase[k] # "pending"
Terminal(j) == phase[j] \in {"passed", "failed", "skipped"}
Init == /\ records = [j \in Jobs |-> InitialRecord(j)]
        /\ phase = [j \in Jobs |-> IF InitialRecord(j) = "failed" THEN "failed" ELSE "pending"]
        /\ failed = {j \in Jobs : InitialRecord(j) = "failed"}
        /\ stop = FALSE /\ cancelled = FALSE /\ validation = "unvalidated" /\ reused = {}
        /\ recordCoverage = [j \in Jobs |-> IF InitialRecord(j) = "passed" THEN Witness(j) ELSE {}]
        /\ evidence = [j \in Jobs |-> {}] /\ temporary = [j \in Jobs |-> FALSE]
        /\ modelResult = [m \in Models |-> "pending"]
ValidateRun == /\ validation = "unvalidated"
               /\ validation' = IF RunValid THEN "ready" ELSE "rejected"
               /\ stop' = (stop \/ ~RunValid)
               /\ UNCHANGED <<phase, failed, cancelled, reused, records,
                   recordCoverage, evidence, temporary, modelResult>>
AcceptContinuation == ValidateRun /\ RunValid
RejectContinuation == ValidateRun /\ ~RunValid
Reuse(j) == /\ validation = "ready" /\ ~stop /\ phase[j] = "pending"
            /\ records[j] = "passed" /\ FirstPending(j)
            /\ phase' = [phase EXCEPT ![j] = "passed"]
            /\ reused' = reused \cup {j}
            /\ evidence' = [evidence EXCEPT ![j] = recordCoverage[j]]
            /\ UNCHANGED <<failed, stop, cancelled, validation, records,
                recordCoverage, temporary, modelResult>>
Start(j) == /\ validation = "ready" /\ ~stop /\ phase[j] = "pending"
            /\ records[j] = "absent" /\ FirstPending(j) /\ Fits(j)
            /\ phase' = [phase EXCEPT ![j] = "active"]
            /\ UNCHANGED <<failed, stop, cancelled, validation, reused, records,
                recordCoverage, evidence, temporary, modelResult>>
StartSingle(j) == Start(j) /\ Active = {} /\ Weight(j) <= 2
StartConcurrent(j) == Start(j) /\ Active # {}
StartOversize(j) == Start(j) /\ Weight(j) > 2
RecordFailure(j) == /\ j \in Active /\ j \notin failed
                    /\ failed' = failed \cup {j} /\ stop' = TRUE
                    /\ UNCHANGED <<phase, cancelled, validation, reused, records,
                        recordCoverage, evidence, temporary, modelResult>>
CloseChild(j) == /\ phase[j] = "active" /\ j \notin failed /\ ~cancelled
                 /\ phase' = [phase EXCEPT ![j] = "ready"]
                 /\ UNCHANGED <<failed, stop, cancelled, validation, reused, records,
                     recordCoverage, evidence, temporary, modelResult>>
PrepareRecord(j) == /\ phase[j] = "ready" /\ j \notin failed /\ ~cancelled /\ LogComplete(j)
                    /\ phase' = [phase EXCEPT ![j] = "writing"]
                    /\ temporary' = [temporary EXCEPT ![j] = TRUE]
                    /\ UNCHANGED <<failed, stop, cancelled, validation, reused,
                        records, recordCoverage, evidence, modelResult>>
RejectLog(j) == /\ phase[j] = "ready" /\ ~LogComplete(j) /\ j \notin failed
                /\ failed' = failed \cup {j} /\ stop' = TRUE
                /\ UNCHANGED <<phase, cancelled, validation, reused, records,
                    recordCoverage, evidence, temporary, modelResult>>
CommitRecord(j) == /\ phase[j] = "writing" /\ temporary[j] /\ records[j] = "absent"
                   /\ j \notin failed /\ ~cancelled
                   /\ phase' = [phase EXCEPT ![j] = "passed"]
                   /\ records' = [records EXCEPT ![j] = "passed"]
                   /\ recordCoverage' = [recordCoverage EXCEPT ![j] = Witness(j)]
                   /\ evidence' = [evidence EXCEPT ![j] = Witness(j)]
                   /\ temporary' = [temporary EXCEPT ![j] = FALSE]
                   /\ UNCHANGED <<failed, stop, cancelled, validation, reused, modelResult>>
DrainFailure(j) == /\ j \in Active /\ (j \in failed \/ cancelled) /\ records[j] = "absent"
                   /\ phase' = [phase EXCEPT ![j] = "failed"]
                   /\ failed' = failed \cup {j}
                   /\ records' = [records EXCEPT ![j] = "failed"]
                   /\ temporary' = [temporary EXCEPT ![j] = FALSE]
                   /\ UNCHANGED <<stop, cancelled, validation, reused,
                       recordCoverage, evidence, modelResult>>
Skip(j) == /\ stop /\ phase[j] = "pending"
           /\ phase' = [phase EXCEPT ![j] = "skipped"]
           /\ UNCHANGED <<failed, stop, cancelled, validation, reused, records,
               recordCoverage, evidence, temporary, modelResult>>
Cancel == /\ ~cancelled /\ ~ (\A j \in Jobs : Terminal(j))
          /\ cancelled' = TRUE /\ stop' = TRUE
          /\ UNCHANGED <<phase, failed, validation, reused, records,
              recordCoverage, evidence, temporary, modelResult>>
ModelJobs(m) == {j \in Jobs : Model(j) = m}
AllPassed(m) == \A j \in ModelJobs(m) : phase[j] = "passed"
Covered(m) == Required(m) \subseteq UNION {evidence[j] : j \in ModelJobs(m)}
Mixed(m) == ModelJobs(m) \cap reused # {} /\ ModelJobs(m) \ reused # {}
Adjudicate(m) == /\ modelResult[m] = "pending"
                 /\ \A j \in ModelJobs(m) : Terminal(j)
                 /\ modelResult' = [modelResult EXCEPT ![m] =
                     IF AllPassed(m) /\ Covered(m) THEN "passed" ELSE "failed"]
                 /\ stop' = (stop \/ (AllPassed(m) /\ ~Covered(m)))
                 /\ UNCHANGED <<phase, failed, cancelled, validation, reused,
                     records, recordCoverage, evidence, temporary>>
AdjudicateVerified(m) == Adjudicate(m) /\ AllPassed(m) /\ Covered(m) /\ ~Mixed(m)
AdjudicateReusedAndFresh(m) == Adjudicate(m) /\ AllPassed(m) /\ Covered(m) /\ Mixed(m)
AdjudicateCoverageFailure(m) == Adjudicate(m) /\ AllPassed(m) /\ ~Covered(m)
AdjudicateIncomplete(m) == Adjudicate(m) /\ ~AllPassed(m)
Next == AcceptContinuation \/ RejectContinuation \/ Cancel \/
    (\E j \in Jobs : Reuse(j) \/ StartSingle(j) \/ StartConcurrent(j) \/ StartOversize(j) \/
      RecordFailure(j) \/ CloseChild(j) \/ PrepareRecord(j) \/ RejectLog(j) \/
      CommitRecord(j) \/ DrainFailure(j) \/ Skip(j)) \/
    (\E m \in Models : AdjudicateVerified(m) \/ AdjudicateReusedAndFresh(m) \/
      AdjudicateCoverageFailure(m) \/ AdjudicateIncomplete(m))
TypeOK == /\ phase \in [Jobs -> {"pending", "active", "ready", "writing", "passed", "failed", "skipped"}]
          /\ failed \subseteq Jobs /\ stop \in BOOLEAN /\ cancelled \in BOOLEAN
          /\ validation \in {"unvalidated", "ready", "rejected"} /\ reused \subseteq Jobs
          /\ records \in [Jobs -> {"absent", "failed", "passed"}]
          /\ recordCoverage \in [Jobs -> SUBSET {"a", "b"}]
          /\ evidence \in [Jobs -> SUBSET {"a", "b"}] /\ temporary \in [Jobs -> BOOLEAN]
          /\ modelResult \in [Models -> {"pending", "passed", "failed"}]
ResourceSafe == Cardinality(Active) <= 2 /\
    (Used <= 2 \/ (Cardinality(Active) = 1 /\ \E j \in Active : Weight(j) > 2))
CacheSafe == \A j \in reused : RunValid /\ records[j] = "passed" /\ evidence[j] = recordCoverage[j]
CommitSafe == \A j \in Jobs : phase[j] = "passed" =>
    records[j] = "passed" /\ ~temporary[j] /\ evidence[j] = recordCoverage[j]
FailureSafe == \A j \in failed : phase[j] # "passed"
ModelSafe == \A m \in Models : modelResult[m] = "passed" => AllPassed(m) /\ Covered(m)
AdmissionSafe == stop => ~ (\E j \in Jobs : ENABLED Start(j) \/ ENABLED Reuse(j))
RejectedSafe == validation = "rejected" => Active = {} /\ reused = {}
NoOverwrite == \A j \in Jobs : records[j] # "absent" => records'[j] = records[j]
RecordStability == [][NoOverwrite]_vars
Done == (\A j \in Jobs : Terminal(j)) /\ (\A m \in Models : modelResult[m] # "pending")
Spec == Init /\ [][Next]_vars /\ WF_vars(ValidateRun) /\
    (\A j \in Jobs : WF_vars(Reuse(j)) /\ WF_vars(Start(j)) /\ WF_vars(CloseChild(j)) /\
      WF_vars(PrepareRecord(j)) /\ WF_vars(RejectLog(j)) /\ WF_vars(CommitRecord(j)) /\
      WF_vars(DrainFailure(j)) /\ WF_vars(Skip(j))) /\
    (\A m \in Models : WF_vars(Adjudicate(m)))
Completion == <>Done
=============================================================================
