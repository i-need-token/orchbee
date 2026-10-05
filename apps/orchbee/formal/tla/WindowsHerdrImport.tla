---------------- MODULE WindowsHerdrImport ----------------
EXTENDS Naturals, FiniteSets, TLC

\* apps/orchbee/docs/windows-cli-configuration.md#file-workflow-and-recovery
\* Exclusive CREATE_NEW, pinned ancestors and handle sharing are adapter assumptions.
Calls == 1..2
Modes == {"preview", "write"}
Phases == {"new", "read", "prepared", "created", "partial", "written",
           "synced", "preview", "success", "failed", "refused", "crashed", "unreported"}
VARIABLE s
vars == <<s>>

Init ==
  \E existing \in BOOLEAN, modes \in [Calls -> Modes]:
    s = [existing |-> existing, mode |-> modes,
         phase |-> [i \in Calls |-> "new"], reads |-> [i \in Calls |-> 0],
         prepared |-> [i \in Calls |-> FALSE], attempts |-> [i \in Calls |-> 0],
         created |-> [i \in Calls |-> FALSE], flushes |-> [i \in Calls |-> 0],
         fileOwner |-> IF existing THEN 3 ELSE 0,
         fileState |-> IF existing THEN "original" ELSE "absent",
         open |-> 0, sourceWrites |-> 0, runtimeEffects |-> 0, coverage |-> "Init"]

ReadSource(i) ==
  /\ s.phase[i] = "new"
  /\ s' = [s EXCEPT !.phase[i] = "read", !.reads[i] = @ + 1,
                    !.coverage = "ReadSource"]
RejectSource(i) ==
  /\ s.phase[i] = "new"
  /\ s' = [s EXCEPT !.phase[i] = "failed", !.reads[i] = @ + 1,
                    !.coverage = "RejectSource"]
PrepareCandidate(i) ==
  /\ s.phase[i] = "read"
  /\ s' = [s EXCEPT !.phase[i] = "prepared", !.prepared[i] = TRUE,
                    !.coverage = "PrepareCandidate"]
RejectCandidate(i) ==
  /\ s.phase[i] = "read"
  /\ s' = [s EXCEPT !.phase[i] = "failed", !.coverage = "RejectCandidate"]
PreviewCandidate(i) ==
  /\ s.phase[i] = "prepared" /\ s.mode[i] = "preview"
  /\ s' = [s EXCEPT !.phase[i] = "preview", !.coverage = "PreviewCandidate"]
CreateCandidate(i) ==
  /\ s.phase[i] = "prepared" /\ s.mode[i] = "write" /\ s.fileOwner = 0
  /\ s' = [s EXCEPT !.phase[i] = "created", !.attempts[i] = @ + 1,
                    !.created[i] = TRUE, !.fileOwner = i, !.fileState = "partial",
                    !.open = i, !.coverage = "CreateCandidate"]
RefuseExisting(i) ==
  /\ s.phase[i] = "prepared" /\ s.mode[i] = "write" /\ s.fileOwner # 0
  /\ s' = [s EXCEPT !.phase[i] = "refused", !.attempts[i] = @ + 1,
                    !.coverage = "RefuseExisting"]
FailCreate(i) ==
  /\ s.phase[i] = "prepared" /\ s.mode[i] = "write" /\ s.fileOwner = 0
  /\ s' = [s EXCEPT !.phase[i] = "failed", !.attempts[i] = @ + 1,
                    !.coverage = "FailCreate"]
WritePartial(i) ==
  /\ s.phase[i] = "created" /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "partial", !.coverage = "WritePartial"]
WriteComplete(i) ==
  /\ s.phase[i] \in {"created", "partial"} /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "written", !.fileState = "written",
                    !.coverage = "WriteComplete"]
FailWrite(i) ==
  /\ s.phase[i] \in {"created", "partial"} /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "failed", !.open = 0,
                    !.coverage = "FailWrite"]
FlushCandidate(i) ==
  /\ s.phase[i] = "written" /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "synced", !.flushes[i] = 1,
                    !.fileState = "synced", !.coverage = "FlushCandidate"]
FailFlush(i) ==
  /\ s.phase[i] = "written" /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "failed", !.open = 0,
                    !.coverage = "FailFlush"]
FinishImport(i) ==
  /\ s.phase[i] = "synced" /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "success", !.open = 0,
                    !.coverage = "FinishImport"]
LoseReceipt(i) ==
  /\ s.phase[i] = "synced" /\ s.open = i
  /\ s' = [s EXCEPT !.phase[i] = "unreported", !.open = 0,
                    !.coverage = "LoseReceipt"]
CrashInvocation(i) ==
  /\ s.phase[i] \in {"new", "read", "prepared", "created", "partial", "written", "synced"}
  /\ s' = [s EXCEPT !.phase[i] = "crashed",
                    !.open = IF @ = i THEN 0 ELSE @, !.coverage = "CrashInvocation"]

Next == \E i \in Calls:
  ReadSource(i) \/ RejectSource(i) \/ PrepareCandidate(i) \/ RejectCandidate(i)
  \/ PreviewCandidate(i) \/ CreateCandidate(i) \/ RefuseExisting(i) \/ FailCreate(i)
  \/ WritePartial(i) \/ WriteComplete(i) \/ FailWrite(i) \/ FlushCandidate(i)
  \/ FailFlush(i) \/ FinishImport(i) \/ LoseReceipt(i) \/ CrashInvocation(i)

Spec == Init /\ [][Next]_vars
TypeOK ==
  /\ s.mode \in [Calls -> Modes] /\ s.phase \in [Calls -> Phases]
  /\ s.reads \in [Calls -> 0..1] /\ s.attempts \in [Calls -> 0..1]
  /\ s.flushes \in [Calls -> 0..1] /\ s.created \in [Calls -> BOOLEAN]
  /\ s.prepared \in [Calls -> BOOLEAN] /\ s.open \in Calls \cup {0}
  /\ s.fileOwner \in Calls \cup {0, 3}
  /\ s.fileState \in {"absent", "original", "partial", "written", "synced"}
SourceAndRuntimeUnchanged == s.sourceWrites = 0 /\ s.runtimeEffects = 0
PreviewCreatesNothing == \A i \in Calls:
  s.mode[i] = "preview" => ~s.created[i] /\ s.attempts[i] = 0
PreparedBeforeCreation == \A i \in Calls: s.created[i] => s.prepared[i]
OneCreator == Cardinality({i \in Calls: s.created[i]}) <= 1
ExistingTargetUnchanged == s.existing =>
  s.fileOwner = 3 /\ s.fileState = "original" /\ s.open = 0
NoReplay == \A i \in Calls: s.attempts[i] <= 1
SuccessRequiresFlush == \A i \in Calls: s.phase[i] = "success" =>
  s.created[i] /\ s.fileOwner = i /\ s.fileState = "synced" /\ s.flushes[i] = 1
ResidueKeepsItsCreator == \A i \in Calls:
  s.created[i] => s.fileOwner = i /\ s.fileState # "absent"
OpenHandleKeepsOwnership == s.open # 0 =>
  s.fileOwner = s.open /\ s.phase[s.open] \in {"created", "partial", "written", "synced"}
=============================================================================
