-------------------- MODULE WindowsCheckpointStorage --------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS MaxPublication, MaxFrameGeneration
Slots == 1..3
Publications == 1..MaxPublication
Records == [publication : Publications, generation : 1..MaxFrameGeneration]
VARIABLES slotPublication, slotGeneration, invalid, damaged, archived,
          phase, chosen, pendingPublication, pendingGeneration,
          nextPublication, committed, protectedPublication, loadedAvailable,
          restoredPublication, restoredGeneration, loadHadDamage, notice,
          lastEvent
vars == <<slotPublication, slotGeneration, invalid, damaged, archived,
          phase, chosen, pendingPublication, pendingGeneration,
          nextPublication, committed, protectedPublication, loadedAvailable,
          restoredPublication, restoredGeneration, loadHadDamage, notice,
          lastEvent>>

Maximum(values) == IF values = {} THEN 0
                   ELSE CHOOSE n \in values : \A m \in values : n >= m
Minimum(values) == CHOOSE n \in values : \A m \in values : n <= m
Present == {s \in Slots : slotPublication[s] # 0}
Valid == Present \ invalid
AvailableGenerations == {slotGeneration[s] : s \in Valid}
AvailableRecords == {
  [publication |-> slotPublication[s], generation |-> slotGeneration[s]] : s \in Valid
}
ActiveDamage == {slotPublication[s] : s \in invalid}
NewestGeneration == Maximum(AvailableGenerations)
NewestPublication ==
  IF Valid = {} THEN 0
  ELSE CHOOSE p \in {slotPublication[s] :
                       s \in {candidate \in Valid :
                         slotGeneration[candidate] = NewestGeneration}} : TRUE
Oldest == {s \in Valid : slotGeneration[s] = Minimum(AvailableGenerations)}
Candidates == IF Slots \ Present # {} THEN Slots \ Present
              ELSE IF invalid # {} THEN invalid
              ELSE Oldest

Init ==
  /\ slotPublication = [s \in Slots |-> 0]
  /\ slotGeneration = [s \in Slots |-> 0]
  /\ invalid = {} /\ damaged = {} /\ archived = {}
  /\ phase = "Idle" /\ chosen = 0
  /\ pendingPublication = 0 /\ pendingGeneration = 0
  /\ nextPublication = 1 /\ committed = {} /\ protectedPublication = 0
  /\ loadedAvailable = {} /\ restoredPublication = 0 /\ restoredGeneration = 0
  /\ loadHadDamage = FALSE /\ notice = "None"
  /\ lastEvent = "Init"

Prepare ==
  /\ phase = "Idle" /\ nextPublication <= MaxPublication
  /\ NewestGeneration < MaxFrameGeneration
  /\ chosen' = Minimum(Candidates)
  /\ pendingPublication' = nextPublication
  /\ pendingGeneration' = NewestGeneration + 1
  /\ phase' = "Writing"
  /\ protectedPublication' = NewestPublication /\ lastEvent' = "Prepare"
  /\ UNCHANGED <<slotPublication, slotGeneration, invalid, damaged, archived,
                  nextPublication, committed, loadedAvailable, restoredPublication,
                  restoredGeneration, loadHadDamage, notice>>

Flush ==
  /\ phase = "Writing" /\ phase' = "Prepared" /\ lastEvent' = "Flush"
  /\ UNCHANGED <<slotPublication, slotGeneration, invalid, damaged, archived,
                  chosen, pendingPublication, pendingGeneration, nextPublication,
                  committed, protectedPublication, loadedAvailable,
                  restoredPublication, restoredGeneration, loadHadDamage, notice>>

PreserveInvalid ==
  /\ phase = "Prepared" /\ chosen \in invalid
  /\ archived' = archived \cup {slotPublication[chosen]}
  /\ slotPublication' = [slotPublication EXCEPT ![chosen] = 0]
  /\ slotGeneration' = [slotGeneration EXCEPT ![chosen] = 0]
  /\ invalid' = invalid \ {chosen}
  /\ phase' = "Preserved" /\ lastEvent' = "PreserveInvalid"
  /\ UNCHANGED <<damaged, chosen, pendingPublication, pendingGeneration,
                  nextPublication, committed, protectedPublication, loadedAvailable,
                  restoredPublication, restoredGeneration, loadHadDamage, notice>>

Commit ==
  /\ phase \in {"Prepared", "Preserved"} /\ chosen \notin invalid
  /\ slotPublication' = [slotPublication EXCEPT ![chosen] = pendingPublication]
  /\ slotGeneration' = [slotGeneration EXCEPT ![chosen] = pendingGeneration]
  /\ committed' = committed \cup {pendingPublication}
  /\ nextPublication' = pendingPublication + 1
  /\ phase' = "Idle" /\ chosen' = 0
  /\ pendingPublication' = 0 /\ pendingGeneration' = 0
  /\ lastEvent' = "Commit"
  /\ UNCHANGED <<invalid, damaged, archived, protectedPublication,
                  loadedAvailable, restoredPublication, restoredGeneration,
                  loadHadDamage, notice>>

WriteFailure ==
  /\ phase # "Idle"
  /\ phase' = "Idle" /\ chosen' = 0
  /\ pendingPublication' = 0 /\ pendingGeneration' = 0
  /\ lastEvent' = "WriteFailure"
  /\ UNCHANGED <<slotPublication, slotGeneration, invalid, damaged, archived,
                  nextPublication, committed, protectedPublication, loadedAvailable,
                  restoredPublication, restoredGeneration, loadHadDamage, notice>>

Interrupt ==
  /\ phase # "Idle"
  /\ phase' = "Idle" /\ chosen' = 0
  /\ pendingPublication' = 0 /\ pendingGeneration' = 0
  /\ lastEvent' = "Interrupt"
  /\ UNCHANGED <<slotPublication, slotGeneration, invalid, damaged, archived,
                  nextPublication, committed, protectedPublication, loadedAvailable,
                  restoredPublication, restoredGeneration, loadHadDamage, notice>>

CorruptSlot(s) ==
  /\ phase = "Idle" /\ s \in Valid
  /\ invalid' = invalid \cup {s}
  /\ damaged' = damaged \cup {slotPublication[s]}
  /\ protectedPublication' = 0 /\ lastEvent' = "CorruptSlot"
  /\ UNCHANGED <<slotPublication, slotGeneration, archived, phase, chosen,
                  pendingPublication, pendingGeneration, nextPublication, committed,
                  loadedAvailable, restoredPublication, restoredGeneration,
                  loadHadDamage, notice>>

Load ==
  /\ phase = "Idle"
  /\ loadedAvailable' = AvailableRecords
  /\ restoredPublication' = NewestPublication /\ restoredGeneration' = NewestGeneration
  /\ loadHadDamage' = (invalid # {} \/ (AvailableRecords = {} /\ archived # {}))
  /\ notice' = IF invalid # {} \/ (AvailableRecords = {} /\ archived # {})
               THEN IF AvailableRecords = {} THEN "Empty" ELSE "Recovered"
               ELSE "None"
  /\ lastEvent' = "Load"
  /\ UNCHANGED <<slotPublication, slotGeneration, invalid, damaged, archived,
                  phase, chosen, pendingPublication, pendingGeneration,
                  nextPublication, committed, protectedPublication>>

Next == Prepare \/ Flush \/ PreserveInvalid
        \/ Commit \/ WriteFailure \/ Interrupt \/ (\E s \in Slots : CorruptSlot(s)) \/ Load
SettleWrite == Flush \/ PreserveInvalid \/ Commit \/ WriteFailure \/ Interrupt
Spec == Init /\ [][Next]_vars /\ WF_vars(SettleWrite)

TypeOK ==
  /\ slotPublication \in [Slots -> 0..MaxPublication]
  /\ slotGeneration \in [Slots -> 0..MaxFrameGeneration]
  /\ invalid \subseteq Present
  /\ damaged \subseteq Publications /\ archived \subseteq damaged
  /\ phase \in {"Idle", "Writing", "Prepared", "Preserved"}
  /\ chosen \in 0..3
  /\ pendingPublication \in 0..MaxPublication
  /\ pendingGeneration \in 0..MaxFrameGeneration
  /\ nextPublication \in 1..(MaxPublication + 1)
  /\ committed \subseteq Publications /\ protectedPublication \in 0..MaxPublication
  /\ loadedAvailable \subseteq Records
  /\ restoredPublication \in 0..MaxPublication
  /\ restoredGeneration \in 0..MaxFrameGeneration
  /\ loadHadDamage \in BOOLEAN /\ notice \in {"None", "Empty", "Recovered"}
  /\ lastEvent \in {"Init", "Prepare",
                     "Flush", "PreserveInvalid", "Commit", "WriteFailure",
                     "Interrupt", "CorruptSlot", "Load"}

CompleteSlots == \A s \in Slots : (slotPublication[s] = 0) <=> (slotGeneration[s] = 0)
ThreeCopies == Cardinality(Valid) <= 3
CommittedOnly == \A s \in Present : slotPublication[s] \in committed
PublicationOrder == committed = 1..(nextPublication - 1)
PublicationIdentity ==
  \A first \in Present : \A second \in Present :
    first # second => slotPublication[first] # slotPublication[second]
OriginalsRetained == damaged \subseteq ActiveDamage \cup archived
DamagedWasCommitted == damaged \subseteq committed
FrameGenerationFromValid ==
  phase # "Idle" => pendingGeneration = NewestGeneration + 1
AllInvalidCanPrepare ==
  (phase = "Idle"
    /\ nextPublication <= MaxPublication /\ Valid = {}) => ENABLED Prepare
ValidFrameMaximumRefusesPrepare ==
  (phase = "Idle" /\ nextPublication <= MaxPublication
    /\ NewestGeneration = MaxFrameGeneration) => ~ENABLED Prepare
LatestPreserved == protectedPublication = 0 \/ protectedPublication \in {
  slotPublication[s] : s \in Valid
}
LatestValidRestore ==
  /\ restoredGeneration = Maximum({record.generation : record \in loadedAvailable})
  /\ (restoredGeneration = 0 => restoredPublication = 0)
  /\ (restoredGeneration # 0 =>
        [publication |-> restoredPublication, generation |-> restoredGeneration]
          \in loadedAvailable)
VisibleEmpty == (restoredPublication = 0 /\ loadHadDamage) => notice = "Empty"
VisibleFallback == (restoredPublication # 0 /\ loadHadDamage) => notice = "Recovered"
WriteSettles == (phase # "Idle") ~> (phase = "Idle")
=========================================================================
