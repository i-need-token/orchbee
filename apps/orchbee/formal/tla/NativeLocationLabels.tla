-------------------------- MODULE NativeLocationLabels --------------------------
EXTENDS Naturals, FiniteSets

\* One Client/Site, Project p owning Workspace w, two labels, two request and
\* draft identities, and one peer revision write (label or membership).
\* Both attached Workspace and Standalone starts are admitted. Two operations
\* expose settled-executor evidence overlapping a later queued mailbox action.
\* No state constraints prune failures. Token/request exhaustion is a finite
\* abstraction of checked allocation. UTF-8 bytes, scalar limits, protocol
\* decoding, physical mailbox cells, Worker lifecycle and exact Viewer keys
\* refine through NativeClientRenameLifecycle, NativeClientMembershipRemoval,
\* NativeContextSelection and deterministic input/carrier tests. This model
\* checks their composition with location labels, not their full internals.
Requests == 1..2
Kinds == {"project", "workspace"}
Targets == {"p", "w"}
Labels == [Targets -> 0..1]
Target(kind) == IF kind = "project" THEN "p" ELSE "w"
KnownStatuses == {"Unchanged", "Renamed"}
Statuses == KnownStatuses \cup {"none", "NotApplied", "Unknown"}
NoEvidence == [request |-> 0, kind |-> "none", target |-> "none",
               status |-> "none", revision |-> 0]
NoCommand == [kind |-> "none", target |-> "none", site |-> "site",
              project |-> "p", workspace |-> "w", base |-> 0, token |-> 0,
              original |-> 0, label |-> 0, labels |-> [t \in Targets |-> 0], member |-> 0]
Transport == {"queued", "command", "committed", "known", "snapshot", "cleanup"}
Pending == Transport \cup {"ui", "refused", "install"}
Phases == Pending \cup {"idle", "draft", "done", "recovery", "retired"}

VARIABLE s
vars == <<s>>
Init ==
  \E location \in {"standalone", "w"}, prior \in BOOLEAN:
    s = [phase |-> "idle", location |-> location, busy |-> FALSE,
         revision |-> 0, labels |-> [t \in Targets |-> 0], member |-> 0,
         peerWrote |-> FALSE, cacheRevision |-> 0,
         cacheLabels |-> [t \in Targets |-> 0], cacheMember |-> 0,
         viewer |-> 0, detaches |-> 0, attaches |-> 0, launches |-> 0,
         token |-> 0, shown |-> 0, draftKind |-> "project", draftBase |-> 0,
         original |-> 0, text |-> 0, complete |-> TRUE, fault |-> FALSE,
         operation |-> 0, commands |-> [id \in Requests |-> NoCommand],
         sent |-> [id \in Requests |-> 0], receipts |-> [id \in Requests |-> NoEvidence],
         executor |-> [id \in Requests |-> NoEvidence], mailbox |-> [id \in Requests |-> NoEvidence],
         candidate |-> [revision |-> 0, labels |-> [t \in Targets |-> 0], member |-> 0],
         validated |-> FALSE, cleanupDone |-> FALSE, recovered |-> FALSE,
         budget |-> 0, priorAccepted |-> prior]

Command == s.commands[s.operation]
CurrentKnown == IF s.operation = 0 THEN FALSE ELSE s.executor[s.operation].status \in KnownStatuses
Evidence(status, revision) == [request |-> s.operation, kind |-> Command.kind,
  target |-> Command.target, status |-> status, revision |-> revision]
Draft == s.phase = "draft"
CanSubmit == Draft /\ s.shown = s.token /\ s.token > 0 /\ s.complete /\ ~s.fault
ExpectedLabels == [Command.labels EXCEPT ![Command.target] = Command.label]

Open(kind) ==
  /\ s.phase \in {"idle", "done"} /\ s.location = "w" /\ ~s.busy
  /\ s.token < 2 /\ s.operation < 2 /\ kind \in Kinds
  /\ s' = [s EXCEPT !.phase = "draft", !.token = @+1, !.shown = 0,
       !.draftKind = kind, !.draftBase = s.cacheRevision,
       !.original = s.cacheLabels[Target(kind)], !.text = s.cacheLabels[Target(kind)],
       !.complete = TRUE, !.fault = FALSE]
OpenProjectLabel == /\ s.location = "w" /\ Open("project")
OpenWorkspaceLabel == /\ s.location = "w" /\ Open("workspace")
RejectStandalone == /\ s.location = "standalone" /\ UNCHANGED vars
BeginCompetingWork == /\ s.phase = "idle" /\ ~s.busy /\ s' = [s EXCEPT !.busy = TRUE]
FinishCompetingWork == /\ s.busy /\ s' = [s EXCEPT !.busy = FALSE]
RejectBusyAdmission == /\ s.busy /\ UNCHANGED vars
RejectCompetingStructure == /\ s.phase \in Pending \cup {"draft", "recovery"} /\ UNCHANGED vars
DrawExact == /\ Draft /\ s.shown # s.token /\ s' = [s EXCEPT !.shown = s.token]
RevokeDraw == /\ Draft /\ s.shown = s.token /\ s' = [s EXCEPT !.shown = 0]
RejectStaleDraw == /\ Draft /\ s.token = 2 /\ UNCHANGED vars
EditLabel == /\ Draft /\ s' = [s EXCEPT !.text = 1-@]
InputFault == /\ Draft /\ ~s.fault /\ s' = [s EXCEPT !.fault = TRUE]
CorrectInput == /\ Draft /\ s.fault /\ s.complete /\ s' = [s EXCEPT !.fault = FALSE]
BeginIncompleteUtf8 == /\ Draft /\ s.complete /\ s' = [s EXCEPT !.complete = FALSE]
FinishUtf8 == /\ Draft /\ ~s.complete /\ s' = [s EXCEPT !.complete = TRUE]
RejectInvalidSubmit == /\ Draft /\ (~s.complete \/ s.fault \/ s.shown # s.token) /\ UNCHANGED vars
CancelDraft == /\ Draft /\ s' = [s EXCEPT !.phase = "done", !.shown = 0]
InvalidateContext ==
  /\ Draft /\ s' = [s EXCEPT !.phase = "done", !.shown = 0, !.location = "standalone"]
Submit ==
  /\ CanSubmit /\ s.operation < 2
  /\ s' = [s EXCEPT !.phase = "queued", !.operation = @+1, !.shown = 0,
       !.commands[s.operation+1] = [kind |-> s.draftKind, target |-> Target(s.draftKind),
         site |-> "site", project |-> "p", workspace |-> "w", base |-> s.draftBase,
         token |-> s.token, original |-> s.original, label |-> s.text,
         labels |-> s.cacheLabels, member |-> s.cacheMember],
       !.validated = FALSE, !.cleanupDone = FALSE, !.recovered = FALSE, !.budget = @+1]
SubmitProjectLabel == /\ s.draftKind = "project" /\ Submit
SubmitWorkspaceLabel == /\ s.draftKind = "workspace" /\ Submit
RejectPostSubmitInput == /\ s.phase \in Pending /\ UNCHANGED vars
DrainEarlierAccepted ==
  /\ s.phase = "queued" /\ s.priorAccepted /\ s' = [s EXCEPT !.priorAccepted = FALSE]
HandOffOnce ==
  /\ s.phase = "queued" /\ ~s.priorAccepted
  /\ s' = [s EXCEPT !.phase = "command", !.sent[s.operation] = @+1]
DefiniteNotSubmitted ==
  /\ s.phase = "queued"
  /\ s' = [s EXCEPT !.phase = "done", !.mailbox[s.operation] = Evidence("NotApplied", 0)]
PeerLabelWrite(target) ==
  /\ ~s.peerWrote /\ target \in Targets /\ s.phase # "retired"
  /\ s' = [s EXCEPT !.peerWrote = TRUE, !.revision = @+1, !.labels[target] = 1-@]
PeerMembershipWrite ==
  /\ ~s.peerWrote /\ s.phase # "retired"
  /\ s' = [s EXCEPT !.peerWrote = TRUE, !.revision = @+1, !.member = 1]
DefiniteRefusal ==
  /\ s.phase = "command"
  /\ s' = [s EXCEPT !.phase = "refused", !.executor[s.operation] = Evidence("NotApplied", 0)]
RefuseStaleRevision == /\ s.phase = "command" /\ Command.base # s.revision /\ DefiniteRefusal
ConsumeNotApplied ==
  /\ s.phase = "refused"
  /\ s' = [s EXCEPT !.phase = "done", !.mailbox[s.operation] = s.executor[s.operation]]
ServerUnchanged ==
  /\ s.phase = "command" /\ Command.base = s.revision /\ Command.label = Command.original
  /\ s' = [s EXCEPT !.phase = "committed", !.receipts[s.operation] = Evidence("Unchanged", s.revision)]
ServerRenamed ==
  /\ s.phase = "command" /\ Command.base = s.revision /\ Command.label # Command.original
  /\ s' = [s EXCEPT !.phase = "committed", !.revision = @+1,
       !.labels[Command.target] = Command.label, !.receipts[s.operation] = Evidence("Renamed", s.revision+1)]
ReceiveKnownResult ==
  /\ s.phase = "committed"
  /\ s' = [s EXCEPT !.phase = "known", !.executor[s.operation] = s.receipts[s.operation]]
OutcomeUnknown ==
  /\ s.phase \in {"command", "committed"}
  /\ s' = [s EXCEPT !.phase = "retired", !.executor[s.operation] = Evidence("Unknown", 0)]
LoseChangedReceipt == /\ s.phase = "committed" /\ s.receipts[s.operation].status = "Renamed" /\ OutcomeUnknown
RejectIncompatibleResult == /\ s.phase = "command" /\ OutcomeUnknown
RequestSnapshot == /\ s.phase = "known" /\ CurrentKnown /\ s' = [s EXCEPT !.phase = "snapshot"]
SnapshotUnavailable ==
  /\ s.phase = "snapshot"
  /\ s' = [s EXCEPT !.phase = "recovery", !.mailbox[s.operation] = s.executor[s.operation]]
RejectWrongSnapshotOwner == /\ s.phase = "snapshot" /\ SnapshotUnavailable
RejectUnexpectedExactDelta ==
  /\ s.phase = "snapshot" /\ s.revision = s.executor[s.operation].revision
  /\ SnapshotUnavailable
SnapshotAvailable ==
  /\ s.phase = "snapshot" /\ CurrentKnown
  /\ s.revision >= s.executor[s.operation].revision
  /\ (s.revision = s.executor[s.operation].revision =>
       s.labels = ExpectedLabels /\ s.member = Command.member)
  /\ s' = [s EXCEPT !.phase = "ui", !.validated = TRUE,
       !.candidate = [revision |-> s.revision, labels |-> s.labels, member |-> s.member],
       !.mailbox[s.operation] = s.executor[s.operation]]
AcceptExactSnapshot == /\ s.phase = "snapshot" /\ s.revision = s.executor[s.operation].revision /\ SnapshotAvailable
AcceptLaterSnapshot == /\ s.phase = "snapshot" /\ s.revision > s.executor[s.operation].revision /\ SnapshotAvailable
ConsumeKnownSnapshot ==
  /\ s.phase = "ui" /\ s.validated
  /\ s' = [s EXCEPT !.phase = IF s.candidate.member = s.cacheMember THEN "done" ELSE "cleanup",
       !.cacheRevision = IF s.candidate.member = s.cacheMember THEN s.candidate.revision ELSE @,
       !.cacheLabels = IF s.candidate.member = s.cacheMember THEN s.candidate.labels ELSE @]
InstallLabelOnly == /\ s.phase = "ui" /\ s.candidate.member = s.cacheMember /\ ConsumeKnownSnapshot
RoutePeerMembership == /\ s.phase = "ui" /\ s.candidate.member # s.cacheMember /\ ConsumeKnownSnapshot
CompleteExactCleanup ==
  /\ s.phase = "cleanup" /\ s.validated /\ s.viewer = s.cacheMember
  /\ s' = [s EXCEPT !.phase = "install", !.viewer = 2, !.detaches = @+1, !.cleanupDone = TRUE]
InstallPeerMembership ==
  /\ s.phase = "install" /\ s.cleanupDone /\ s.viewer = 2
  /\ s' = [s EXCEPT !.phase = "done", !.cacheRevision = s.candidate.revision,
       !.cacheLabels = s.candidate.labels, !.cacheMember = s.candidate.member,
       !.viewer = s.candidate.member, !.attaches = @+1]
ExplicitRefresh ==
  /\ s.phase = "recovery" /\ ~s.recovered
  /\ s' = [s EXCEPT !.phase = "snapshot", !.recovered = TRUE, !.budget = @+1]
RecoverKnownNoop == /\ s.phase = "recovery" /\ s.executor[s.operation].status = "Unchanged" /\ ExplicitRefresh
RecoverKnownRename == /\ s.phase = "recovery" /\ s.executor[s.operation].status = "Renamed" /\ ExplicitRefresh
RejectMutationReplay == /\ s.phase \in {"recovery", "retired"} /\ UNCHANGED vars
DuplicateExactEvidence ==
  /\ \E id \in Requests: s.executor[id] # NoEvidence \/ s.mailbox[id] # NoEvidence
  /\ UNCHANGED vars
RejectLateResult == /\ s.operation = 2 /\ UNCHANGED vars
ExpirePending ==
  /\ s.phase \in Transport
  /\ s' = [s EXCEPT !.phase = "retired",
       !.mailbox[s.operation] = IF s.phase = "queued" THEN Evidence("NotApplied", 0) ELSE @,
       !.executor[s.operation] = IF s.phase \in {"command", "committed"}
         THEN Evidence("Unknown", 0) ELSE @]
QueuedRetirementKeepsPriorKnown ==
  /\ s.phase = "queued" /\ s.operation = 2 /\ s.executor[1].status \in KnownStatuses
  /\ ExpirePending

Next == OpenProjectLabel \/ OpenWorkspaceLabel \/ RejectStandalone
  \/ BeginCompetingWork \/ FinishCompetingWork \/ RejectBusyAdmission \/ RejectCompetingStructure
  \/ DrawExact \/ RevokeDraw \/ RejectStaleDraw \/ EditLabel \/ InputFault \/ CorrectInput
  \/ BeginIncompleteUtf8 \/ FinishUtf8 \/ RejectInvalidSubmit \/ CancelDraft \/ InvalidateContext
  \/ SubmitProjectLabel \/ SubmitWorkspaceLabel \/ RejectPostSubmitInput \/ DrainEarlierAccepted
  \/ HandOffOnce \/ DefiniteNotSubmitted \/ (\E target \in Targets: PeerLabelWrite(target))
  \/ PeerMembershipWrite \/ DefiniteRefusal \/ RefuseStaleRevision \/ ConsumeNotApplied
  \/ ServerUnchanged \/ ServerRenamed \/ ReceiveKnownResult \/ OutcomeUnknown
  \/ LoseChangedReceipt \/ RejectIncompatibleResult \/ RequestSnapshot \/ SnapshotUnavailable
  \/ RejectWrongSnapshotOwner \/ RejectUnexpectedExactDelta \/ AcceptExactSnapshot \/ AcceptLaterSnapshot
  \/ InstallLabelOnly \/ RoutePeerMembership \/ CompleteExactCleanup \/ InstallPeerMembership
  \/ RecoverKnownNoop \/ RecoverKnownRename \/ RejectMutationReplay \/ DuplicateExactEvidence
  \/ RejectLateResult \/ ExpirePending \/ QueuedRetirementKeepsPriorKnown

\* Only existing finite transport/cleanup cutoffs and available local result
\* consumption are fair. No fairness is assumed for typing or explicit w.
Spec == Init /\ [][Next]_vars /\ WF_vars(ExpirePending)
  /\ WF_vars(ConsumeNotApplied) /\ WF_vars(ConsumeKnownSnapshot) /\ WF_vars(InstallPeerMembership)

EvidenceType == [request : 0..2, kind : Kinds \cup {"none"}, target : Targets \cup {"none"},
                 status : Statuses, revision : 0..3]
TypeOK ==
  /\ s.phase \in Phases /\ s.location \in {"standalone", "w"} /\ s.busy \in BOOLEAN
  /\ s.revision \in 0..3 /\ s.labels \in Labels /\ s.member \in 0..1 /\ s.peerWrote \in BOOLEAN
  /\ s.cacheRevision \in 0..3 /\ s.cacheLabels \in Labels /\ s.cacheMember \in 0..1
  /\ s.viewer \in 0..2 /\ s.detaches \in 0..1 /\ s.attaches \in 0..1 /\ s.launches = 0
  /\ s.token \in 0..2 /\ s.shown \in 0..2 /\ s.draftKind \in Kinds /\ s.draftBase \in 0..3
  /\ s.original \in 0..1 /\ s.text \in 0..1 /\ s.complete \in BOOLEAN /\ s.fault \in BOOLEAN
  /\ s.operation \in 0..2 /\ s.sent \in [Requests -> 0..1]
  /\ s.commands \in [Requests -> [kind : Kinds \cup {"none"}, target : Targets \cup {"none"},
       site : {"site"}, project : {"p"}, workspace : {"w"}, base : 0..3, token : 0..2,
       original : 0..1, label : 0..1, labels : Labels, member : 0..1]]
  /\ s.receipts \in [Requests -> EvidenceType] /\ s.executor \in [Requests -> EvidenceType]
  /\ s.mailbox \in [Requests -> EvidenceType]
  /\ s.candidate \in [revision : 0..3, labels : Labels, member : 0..1]
  /\ s.validated \in BOOLEAN /\ s.cleanupDone \in BOOLEAN /\ s.recovered \in BOOLEAN
  /\ s.budget \in 0..4 /\ s.priorAccepted \in BOOLEAN
AtMostOnceHandoff == \A id \in Requests: s.sent[id] <= 1
HandoffDrainsAccepted == (\E id \in Requests: s.sent[id] = 1) => ~s.priorAccepted
ExactEvidenceIdentity == \A id \in Requests:
  /\ (s.executor[id] # NoEvidence => s.executor[id].request = id
       /\ s.executor[id].kind = s.commands[id].kind /\ s.executor[id].target = s.commands[id].target)
  /\ (s.mailbox[id] # NoEvidence => s.mailbox[id].request = id
       /\ s.mailbox[id].kind = s.commands[id].kind /\ s.mailbox[id].target = s.commands[id].target)
KnownEvidenceMatchesReceipt == \A id \in Requests:
  s.executor[id].status \in KnownStatuses => s.executor[id] = s.receipts[id] /\ s.sent[id] = 1
KnownRevisionAndLabel == \A id \in Requests:
  /\ (s.executor[id].status = "Unchanged" => s.executor[id].revision = s.commands[id].base
       /\ s.commands[id].label = s.commands[id].original)
  /\ (s.executor[id].status = "Renamed" => s.executor[id].revision = s.commands[id].base+1
       /\ s.commands[id].label # s.commands[id].original)
NotAppliedHasNoServerResult == \A id \in Requests:
  s.executor[id].status = "NotApplied" \/ s.mailbox[id].status = "NotApplied" => s.receipts[id] = NoEvidence
UnknownRetires == (\E id \in Requests: s.executor[id].status = "Unknown") => s.phase = "retired"
SnapshotNeedsKnownEvidence == s.phase \in {"snapshot", "ui", "recovery", "cleanup", "install"} => CurrentKnown
AvailableUiResultCannotExpire == s.phase \in {"ui", "refused", "install"} => ~ENABLED ExpirePending
CleanupNeedsActualMembershipChange == s.phase \in {"cleanup", "install"} =>
  s.validated /\ s.candidate.member # s.cacheMember
ViewerTracksInstalledMembership == s.viewer # 2 => s.viewer = s.cacheMember
NoImplicitLaunch == s.launches = 0

SubmissionCapturesExactDraft == [][Submit => CanSubmit /\ s.location = "w"
  /\ s'.commands[s.operation+1].base = s.draftBase
  /\ s'.commands[s.operation+1].target = Target(s.draftKind)]_vars
CapturedOwnerNeverRetargets == [][\A id \in 1..s.operation: s'.commands[id] = s.commands[id]]_vars
EvidenceNeverRegresses == [][\A id \in Requests:
  /\ (s.executor[id] # NoEvidence => s'.executor[id] = s.executor[id])
  /\ (s.mailbox[id] # NoEvidence => s'.mailbox[id] = s.mailbox[id])]_vars
QueuedRetirementKeepsBothOwners == [][QueuedRetirementKeepsPriorKnown =>
  s'.executor[1] = s.executor[1] /\ s'.mailbox[2] = Evidence("NotApplied", 0)]_vars
OnlySubmitAllocates == [][s'.operation # s.operation => Submit]_vars
ExplicitRefreshNeverReplays == [][ExplicitRefresh =>
  UNCHANGED <<s.operation, s.sent, s.commands, s.executor, s.mailbox>>]_vars
BudgetNeverRenewsImplicitly == [][s'.operation = s.operation /\ ~ExplicitRefresh => s'.budget = s.budget]_vars
LabelOnlyPreservesShell == [][InstallLabelOnly => UNCHANGED <<s.viewer, s.detaches, s.attaches, s.cacheMember>>]_vars
MembershipChangesOnlyThroughOwner == [][s'.cacheMember # s.cacheMember =>
  InstallPeerMembership /\ s.cleanupDone /\ s.viewer = 2]_vars
RenamePreservesCanonicalMembership == [][ServerUnchanged \/ ServerRenamed => UNCHANGED s.member]_vars
CancelDoesNotUndoResult == [][CancelDraft => UNCHANGED <<s.labels, s.member, s.executor, s.mailbox, s.viewer>>]_vars
DefiniteRefusalPreservesShell == [][DefiniteRefusal \/ DefiniteNotSubmitted => UNCHANGED <<s.viewer, s.cacheMember>>]_vars
RetiredCannotReplay == [][s.phase = "retired" => s'.sent = s.sent]_vars
PendingEventuallySettles == (s.phase \in Pending) ~> (s.phase \notin Pending)
=============================================================================
