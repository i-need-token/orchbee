------------------------- MODULE NativeClientCloseHandoff -------------------------
EXTENDS Naturals, FiniteSets

Targets == {"A","B"}
NoTarget == "none"
MaxRank(a,b) == IF a >= b THEN a ELSE b
VARIABLE s
vars == <<s>>

Init ==
  \E fact \in {2,3}:
    s = [
      records |-> [t \in Targets |-> 0], present |-> {"A"},
      workerA |-> fact, resultA |-> 0, aStage |-> "known",
      aReference |-> TRUE, aReturned |-> FALSE, aJoined |-> FALSE,
      active |-> TRUE, retired |-> FALSE,
      pendingKind |-> "A", pendingTarget |-> "A", pendingKey |-> 1,
      cursor |-> 1, usedB |-> FALSE, bStage |-> "none",
      settled |-> {}, removed |-> {}, membershipConsumed |-> {},
      readIntents |-> {}, automatic |-> {}, readCount |-> 0,
      readResult |-> 0, readResultTarget |-> NoTarget, readResultKey |-> 0,
      nextTarget |-> "A", selected |-> "A", manualRequested |-> FALSE
    ]

UnsettledTargets == {t \in s.present: s.records[t] < 3}
EligibleReads == (s.readIntents \cup s.automatic) \cap UnsettledTargets
NextTarget ==
  IF s.nextTarget \in EligibleReads THEN s.nextTarget
  ELSE IF s.nextTarget = "A" THEN "B" ELSE "A"
NoPending == s.pendingKind = "none"
AReleased == ~s.aReference
ReadReference(t) == s.pendingKind = "read" /\ s.pendingTarget = t
Referenced(t) ==
  (t = "A" /\ s.aReference)
  \/ (s.pendingKind # "none" /\ s.pendingTarget = t)

DepositA ==
  /\ s.active /\ s.aStage = "known"
  /\ s' = [s EXCEPT !.aStage = "placed", !.resultA = s.workerA]
ConsumeA ==
  /\ s.active /\ s.aStage = "placed"
  /\ s.pendingKind = "A" /\ s.pendingTarget = "A" /\ s.pendingKey = 1
  /\ s' = [s EXCEPT !.aStage = "consumed", !.records["A"] = s.resultA,
       !.settled = @ \cup {"A"}, !.pendingKind = "none", !.pendingTarget = NoTarget,
       !.automatic = IF s.resultA < 3 THEN {"A"} ELSE {}]
AdmitB ==
  /\ s.active /\ NoPending /\ ~s.usedB /\ s.aStage = "consumed"
  /\ s' = [s EXCEPT !.usedB = TRUE, !.bStage = "queued",
       !.present = @ \cup {"B"}, !.pendingKind = "B", !.pendingTarget = "B",
       !.cursor = @ + 1, !.pendingKey = s.cursor+1]
ConsumeAWorkerReply ==
  /\ s.active /\ s.aStage = "consumed" /\ s.aReference
  /\ s' = [s EXCEPT !.aReference = FALSE]
ReceiveB ==
  /\ s.active /\ AReleased /\ s.bStage = "queued"
  /\ s.pendingKind = "B" /\ s.pendingTarget = "B"
  /\ \E fact \in 1..3:
       s' = [s EXCEPT !.bStage = "placed", !.records["B"] = fact]
ConsumeB ==
  /\ s.active /\ s.bStage = "placed" /\ s.pendingKind = "B"
  /\ s' = [s EXCEPT !.bStage = "consumed", !.settled = @ \cup {"B"},
       !.pendingKind = "none", !.pendingTarget = NoTarget,
       !.automatic = IF s.records["B"] < 3 THEN @ \cup {"B"} ELSE @]
RequestUiRetirement ==
  /\ s.active
  /\ s' = [s EXCEPT !.active = FALSE, !.retired = TRUE,
                    !.automatic = {}, !.readIntents = {}]
WorkerReturnA ==
  /\ s.retired /\ ~s.aReturned
  /\ s' = [s EXCEPT !.aReturned = TRUE]
JoinAndMergeA ==
  /\ s.retired /\ s.aReturned /\ ~s.aJoined
  /\ s' = [s EXCEPT !.aJoined = TRUE,
       !.records["A"] = IF s.aReference THEN MaxRank(@,s.workerA) ELSE @,
       !.aReference = FALSE]
RepeatExactJoinedHandoff ==
  /\ s.aJoined /\ "A" \in s.present
  /\ UNCHANGED vars
RejectForeignHandoff ==
  /\ s.aReturned
  /\ UNCHANGED vars
DeliverStaleAUi ==
  /\ s.usedB \/ s.retired
  /\ UNCHANGED vars

SelectOtherTarget ==
  /\ s.active /\ s.selected = "A" /\ "B" \in s.present
  /\ s' = [s EXCEPT !.selected = "B"]
ObserveMembershipRemoval(t) ==
  /\ t \in s.present /\ s.records[t] > 0 /\ t \notin s.removed
  /\ s' = [s EXCEPT !.removed = @ \cup {t}]
ConsumeMembershipRemoval(t) ==
  /\ t \in s.removed /\ t \notin s.membershipConsumed
  /\ s' = [s EXCEPT !.membershipConsumed = @ \cup {t}]
PruneSettledUnreferenced(t) ==
  /\ t \in s.present /\ s.records[t] = 3
  /\ t \in s.settled \cap s.removed \cap s.membershipConsumed
  /\ ~Referenced(t)
  /\ s' = [s EXCEPT !.present = @ \ {t}, !.records[t] = 0,
       !.readIntents = @ \ {t}, !.automatic = @ \ {t}]

RequestBoundedManualPass ==
  /\ s.active /\ ~s.manualRequested
  /\ s' = [s EXCEPT !.manualRequested = TRUE,
                    !.readIntents = @ \cup UnsettledTargets]
CoalesceManualPass ==
  /\ s.active /\ s.manualRequested
  /\ UNCHANGED vars
ClassifyAutomaticEligibility(t) ==
  /\ s.active /\ t \in UnsettledTargets /\ s.records[t] > 0
  /\ t \notin s.automatic
  /\ s' = [s EXCEPT !.automatic = @ \cup {t}]
ParkAutomaticObservation(t) ==
  /\ t \in s.automatic
  /\ s' = [s EXCEPT !.automatic = @ \ {t}]
ChooseNextEligibleReadTarget ==
  /\ s.active /\ NoPending /\ s.readCount < 2 /\ EligibleReads # {}
  /\ LET t == NextTarget IN
     s' = [s EXCEPT !.pendingKind = "read", !.pendingTarget = t,
       !.cursor = @ + 1, !.pendingKey = s.cursor+1, !.readCount = @ + 1,
       !.readResult = 0, !.readResultTarget = NoTarget, !.readResultKey = 0,
       !.readIntents = @ \ {t}, !.nextTarget = IF t = "A" THEN "B" ELSE "A"]
ReceiveExactReadResult ==
  /\ s.active /\ AReleased /\ s.pendingKind = "read" /\ s.readResultKey = 0
  /\ \E fact \in s.records[s.pendingTarget]..3:
       s' = [s EXCEPT !.readResult = fact,
            !.readResultTarget = s.pendingTarget, !.readResultKey = s.pendingKey]
SettleExactReadSlot ==
  /\ s.active /\ s.pendingKind = "read" /\ s.readResultKey = s.pendingKey
  /\ s.readResultTarget = s.pendingTarget
  /\ LET t == s.pendingTarget IN
       s' = [s EXCEPT !.records[t] = MaxRank(@,s.readResult),
         !.settled = @ \cup {t}, !.pendingKind = "none", !.pendingTarget = NoTarget,
         !.automatic = IF s.readResult = 3 THEN @ \ {t} ELSE @]
RemovedRetainedTargetRead ==
  /\ s.pendingTarget \in s.removed /\ ReceiveExactReadResult

Next ==
  \/ DepositA \/ ConsumeA \/ AdmitB \/ ConsumeAWorkerReply
  \/ ReceiveB \/ ConsumeB \/ RequestUiRetirement \/ WorkerReturnA
  \/ JoinAndMergeA \/ RepeatExactJoinedHandoff \/ RejectForeignHandoff
  \/ DeliverStaleAUi \/ SelectOtherTarget
  \/ \E t \in Targets: ObserveMembershipRemoval(t)
  \/ \E t \in Targets: ConsumeMembershipRemoval(t)
  \/ \E t \in Targets: PruneSettledUnreferenced(t)
  \/ RequestBoundedManualPass \/ CoalesceManualPass
  \/ \E t \in Targets: ClassifyAutomaticEligibility(t)
  \/ \E t \in Targets: ParkAutomaticObservation(t)
  \/ ChooseNextEligibleReadTarget \/ ReceiveExactReadResult
  \/ SettleExactReadSlot \/ RemovedRetainedTargetRead

Spec == Init /\ [][Next]_vars
  /\ WF_vars(ConsumeA) /\ WF_vars(ConsumeB)
  /\ WF_vars(SettleExactReadSlot) /\ WF_vars(JoinAndMergeA)

TypeOK ==
  /\ s.records \in [Targets -> 0..3] /\ s.present \subseteq Targets
  /\ s.settled \subseteq Targets /\ s.removed \subseteq Targets
  /\ s.membershipConsumed \subseteq s.removed
  /\ s.readIntents \subseteq s.present /\ s.automatic \subseteq s.present
  /\ s.pendingKind \in {"none","A","B","read"}
  /\ s.pendingTarget \in Targets \cup {NoTarget}
  /\ s.readCount \in 0..2 /\ s.cursor \in 1..4
  /\ s.pendingKey \in 1..4 /\ s.readResultKey \in 0..4
OneExactPendingTarget ==
  /\ (NoPending <=> s.pendingTarget = NoTarget)
  /\ s.pendingKind = "A" => s.pendingTarget = "A"
  /\ s.pendingKind = "B" => s.pendingTarget = "B"
  /\ s.pendingKind # "none" => s.pendingTarget \in s.present
ExactReadResultNamesSlot ==
  s.pendingKind = "read" /\ s.readResultKey # 0 =>
    s.readResultKey = s.pendingKey /\ s.readResultTarget = s.pendingTarget
ReferencedAIsAddressable ==
  s.aReference => "A" \in s.present
RetainedKnownRecordsHaveIdentity ==
  \A t \in Targets: s.records[t] > 0 => t \in s.present
JoinPreservesAddressableKnownA ==
  s.aJoined /\ "A" \in s.present => s.records["A"] >= s.workerA
RetirementCannotRestartReads ==
  s.retired => ~s.active /\ ~ENABLED ChooseNextEligibleReadTarget

RetainedFactsNeverRegressWithoutPrune == [][
  \A t \in s.present \cap s'.present: s'.records[t] >= s.records[t]
]_vars
PruningRequiresAllFences == [][
  \A t \in s.present \ s'.present:
    s.records[t] = 3 /\ t \in s.settled \cap s.removed \cap s.membershipConsumed
    /\ ~Referenced(t)
]_vars
ACompletionCannotReplaceB == [][
  JoinAndMergeA \/ ConsumeAWorkerReply =>
    UNCHANGED <<s.records["B"],s.bStage,s.pendingKind,s.pendingTarget,s.pendingKey>>
]_vars
NewFlightRequiresPreviousSemanticSettlement == [][
  s'.cursor > s.cursor => NoPending
]_vars
SelectionKeepsOtherAddresses == [][
  SelectOtherTarget => UNCHANGED <<s.records,s.present,s.readIntents>>
]_vars
ManualPassIncludesRemovedUnsettledTargets == [][
  RequestBoundedManualPass => UnsettledTargets \subseteq s'.readIntents
]_vars
RoundRobinAdvancesAfterAdmission == [][
  ChooseNextEligibleReadTarget =>
    s'.pendingTarget = NextTarget /\ s'.nextTarget # s'.pendingTarget
]_vars
ReadCompletionPreservesOtherTarget == [][
  SettleExactReadSlot =>
    \A t \in Targets \ {s.pendingTarget}: s'.records[t] = s.records[t]
]_vars
CompletedWorkerEventuallyJoined == s.aReturned ~> s.aJoined
PlacedUiEventuallyConsumedOrRetired ==
  (s.aStage = "placed") ~> (s.aStage = "consumed" \/ s.retired)

=============================================================================

