-------------------------- MODULE NativeWorkspaceSetup --------------------------
EXTENDS Naturals, FiniteSets

\* One Client, one Site, one existing and one new Project/Workspace, three
\* mutation identities, one peer revision write, two modal openings and one
\* text edit. Six tokens bound all openings, step advances and picker target
\* changes together, including repeated selection changes. At exhaustion,
\* these actions preserve state, like the implementation's checked token
\* allocation; six is an exploration bound, not a product interaction limit.
\* No state constraint removes reachable failures. Labels/paths
\* project to validity and decoder completeness; exact bytes, scalar limits,
\* paste framing, Worker validation and detailed Viewer cleanup are refined by
\* their existing owners and deterministic tests, not proved here.
Requests == 1..3
Kinds == {"project", "workspace", "directory"}
Projects == {"p0", "p1"}
Workspaces == {"w0", "w1"}
Targets == Projects \cup Workspaces \cup {"none"}
NoReceipt == [kind |-> "none", target |-> "none", revision |-> 0, site |-> "site"]
NoCommand == [kind |-> "none", target |-> "none", base |-> 0,
              token |-> 0, site |-> "site"]
Transport == {"queued", "command", "committed", "known", "snapshot", "cleanup", "install"}
Phases == Transport \cup {"idle", "draft", "done", "recovery", "retired"}

VARIABLE s
vars == <<s>>
Init ==
  \E viewer \in BOOLEAN, priorAccepted \in BOOLEAN:
    s = [phase |-> "idle", step |-> "none", token |-> 0, shown |-> 0,
         text |-> "valid", complete |-> TRUE, edited |-> FALSE,
         picker |-> "p0", draftTarget |-> "none", draftBase |-> 0,
         opens |-> 0, revision |-> 0, cache |-> 0, peerWrote |-> FALSE,
         projects |-> {"p0"}, workspaces |-> {"w0"}, directories |-> {},
         workspaceProject |-> "none", selected |-> "w0", viewer |-> viewer,
         operation |-> 0, commands |-> [id \in Requests |-> NoCommand],
         sent |-> [id \in Requests |-> 0], committed |-> [id \in Requests |-> NoReceipt],
         known |-> [id \in Requests |-> NoReceipt], outcome |-> "none",
         outcomeKind |-> "none", cleanupDone |-> FALSE, validated |-> FALSE,
         recovered |-> FALSE, budget |-> 0, busy |-> FALSE, launches |-> 0,
         priorAccepted |-> priorAccepted]

Command == s.commands[s.operation]
CurrentKnown == IF s.operation = 0 THEN FALSE ELSE s.known[s.operation] # NoReceipt
Draft == s.phase = "draft"
DrawAuthorized == s.shown = s.token /\ s.token # 0
CanEnter == Draft /\ DrawAuthorized /\ s.complete /\ s.text = "valid"
CanOpen == s.phase \in {"idle", "done"} /\ ~s.busy /\ s.opens < 2 /\ s.token < 6
Receipt == [kind |-> Command.kind,
            target |-> IF Command.kind = "workspace" THEN "w1" ELSE Command.target,
            revision |-> s.revision+1, site |-> Command.site]
TargetAvailable == CASE Command.kind = "project" -> "p1" \notin s.projects
  [] Command.kind = "workspace" -> Command.target \in s.projects /\ "w1" \notin s.workspaces
  [] OTHER -> Command.target \in s.workspaces /\ Command.target \notin s.directories

OpenWorkspaceWizard ==
  /\ CanOpen /\ "w1" \notin s.workspaces
  /\ s' = [s EXCEPT !.phase = "draft", !.step = "picker", !.token = @+1,
       !.shown = 0, !.text = "valid", !.complete = TRUE, !.picker = "p0",
       !.draftBase = s.cache, !.draftTarget = "none", !.opens = @+1]
OpenDirectory ==
  /\ CanOpen /\ ~s.viewer /\ s.selected \notin s.directories
  /\ s' = [s EXCEPT !.phase = "draft", !.step = "directory", !.token = @+1,
       !.shown = 0, !.text = "valid", !.complete = TRUE,
       !.draftBase = s.cache, !.draftTarget = s.selected, !.opens = @+1]
RejectDirectoryAdmission ==
  /\ s.phase \in {"idle", "done"} /\ (s.viewer \/ s.selected \in s.directories)
  /\ UNCHANGED vars
BeginCompetingWork ==
  /\ s.phase = "idle" /\ ~s.busy /\ s' = [s EXCEPT !.busy = TRUE]
FinishCompetingWork ==
  /\ s.busy /\ s' = [s EXCEPT !.busy = FALSE]
RejectBusyOpen == /\ s.busy /\ UNCHANGED vars
RejectCompetingStructure == /\ s.phase \in Transport \cup {"draft", "recovery"} /\ UNCHANGED vars

DrawExact == /\ Draft /\ s.shown # s.token /\ s' = [s EXCEPT !.shown = s.token]
RevokeDraw == /\ Draft /\ DrawAuthorized /\ s' = [s EXCEPT !.shown = 0]
RejectStaleDraw == /\ Draft /\ s.token > 1 /\ UNCHANGED vars
ChooseNewProject ==
  /\ Draft /\ s.step = "picker" /\ s.picker = "p0" /\ s.token < 6
  /\ s' = [s EXCEPT !.picker = "new", !.token = @+1, !.shown = 0]
ChooseExistingProject ==
  /\ Draft /\ s.step = "picker" /\ s.picker = "new" /\ s.token < 6
  /\ s' = [s EXCEPT !.picker = "p0", !.token = @+1, !.shown = 0]
AcceptProjectChoice ==
  /\ CanEnter /\ s.step = "picker" /\ s.token < 6
  /\ s' = [s EXCEPT !.step = IF s.picker = "new" THEN "project" ELSE "workspace",
       !.draftTarget = IF s.picker = "new" THEN "p1" ELSE s.picker,
       !.token = @+1, !.shown = 0]
EditText(value) ==
  /\ Draft /\ s.step \in Kinds /\ ~s.edited
  /\ value \in {"valid", "invalid"}
  \* Like Rename, text keeps this step's first-draw authority. Changing the
  \* selected Project or advancing a step allocates a fresh token and revokes it.
  /\ s' = [s EXCEPT !.text = value, !.edited = TRUE]
BeginIncompleteUtf8 ==
  /\ Draft /\ s.step \in Kinds /\ s.complete
  /\ s' = [s EXCEPT !.complete = FALSE]
FinishUtf8 ==
  /\ Draft /\ ~s.complete /\ s' = [s EXCEPT !.complete = TRUE]
RejectUndrawnEnter == /\ Draft /\ ~DrawAuthorized /\ UNCHANGED vars
RejectNextStepSamePacket == /\ Draft /\ s.token > 1 /\ s.shown = 0 /\ RejectUndrawnEnter
RejectIncompleteEnter == /\ Draft /\ ~s.complete /\ UNCHANGED vars
RejectInvalidEnter == /\ Draft /\ s.text = "invalid" /\ UNCHANGED vars
CancelDraft ==
  /\ Draft /\ s' = [s EXCEPT !.phase = "done", !.shown = 0]
CancelAfterKnownProject ==
  /\ Draft /\ s.step = "workspace" /\ CurrentKnown /\ Command.kind = "project"
  /\ CancelDraft
InvalidateDraftContext ==
  /\ Draft /\ s' = [s EXCEPT !.phase = "done", !.shown = 0]

Submit ==
  /\ CanEnter /\ s.step \in Kinds /\ s.operation < 3
  /\ s' = [s EXCEPT !.phase = "queued", !.operation = @+1, !.shown = 0,
       !.commands[s.operation+1] = [kind |-> s.step, target |-> s.draftTarget,
          base |-> s.draftBase, token |-> s.token, site |-> "site"],
       !.outcome = "none", !.outcomeKind = s.step, !.validated = FALSE,
       !.cleanupDone = FALSE, !.recovered = FALSE, !.budget = @+1]
SubmitProject == /\ s.step = "project" /\ Submit
SubmitWorkspace == /\ s.step = "workspace" /\ Submit
SubmitDirectory == /\ s.step = "directory" /\ Submit
RejectPostSubmitInput == /\ s.phase \in Transport /\ UNCHANGED vars
HandOffOnce ==
  /\ s.phase = "queued" /\ ~s.priorAccepted
  /\ s' = [s EXCEPT !.phase = "command", !.sent[s.operation] = @+1]
DrainEarlierAccepted ==
  /\ s.phase = "queued" /\ s.priorAccepted
  /\ s' = [s EXCEPT !.priorAccepted = FALSE]
DefiniteNotSubmitted ==
  /\ s.phase = "queued"
  /\ s' = [s EXCEPT !.phase = "done", !.outcome = "not-submitted"]
PeerRevisionWrite ==
  /\ ~s.peerWrote /\ s.phase # "retired"
  /\ s' = [s EXCEPT !.revision = @+1, !.peerWrote = TRUE]
RefuseStaleRevision ==
  /\ s.phase = "command" /\ Command.base # s.revision
  /\ s' = [s EXCEPT !.phase = "done", !.outcome = "precommit"]
RefuseBeforeCommit ==
  /\ s.phase = "command" /\ Command.base = s.revision
  /\ s' = [s EXCEPT !.phase = "done", !.outcome = "precommit"]
ServerCommit ==
  /\ s.phase = "command" /\ Command.base = s.revision /\ TargetAvailable
  /\ s' = [s EXCEPT !.phase = "committed", !.revision = @+1,
       !.committed[s.operation] = Receipt,
       !.projects = IF Command.kind = "project" THEN @ \cup {Command.target} ELSE @,
       !.workspaces = IF Command.kind = "workspace" THEN @ \cup {"w1"} ELSE @,
       !.workspaceProject = IF Command.kind = "workspace" THEN Command.target ELSE @,
       !.directories = IF Command.kind = "directory" THEN @ \cup {Command.target} ELSE @]
ReceiveKnownCommit ==
  /\ s.phase = "committed"
  /\ s' = [s EXCEPT !.phase = "known", !.known[s.operation] = s.committed[s.operation],
       !.outcome = "known"]
OutcomeUnknown ==
  /\ s.phase \in {"command", "committed"}
  /\ s' = [s EXCEPT !.phase = "retired", !.outcome = "unknown"]
LoseCommittedReceipt == /\ s.phase = "committed" /\ OutcomeUnknown
RejectIncompatibleTypedResult == /\ s.phase = "command" /\ OutcomeUnknown
RequestSnapshot ==
  /\ s.phase = "known" /\ CurrentKnown
  /\ s' = [s EXCEPT !.phase = "snapshot"]
SnapshotUnavailable ==
  /\ s.phase = "snapshot"
  /\ s' = [s EXCEPT !.phase = "recovery"]
RejectWrongSnapshotIdentity ==
  /\ s.phase = "snapshot"
  /\ s' = [s EXCEPT !.phase = "recovery"]
ExplicitRefresh ==
  /\ s.phase = "recovery" /\ ~s.recovered
  /\ s' = [s EXCEPT !.phase = "snapshot", !.recovered = TRUE, !.budget = @+1]
RejectMutationReplay == /\ s.phase \in {"recovery", "retired"} /\ UNCHANGED vars
AcceptProjectSnapshot ==
  /\ s.phase = "snapshot" /\ Command.kind = "project" /\ s.token < 6
  /\ s.known[s.operation].target \in s.projects
  /\ s.revision >= s.known[s.operation].revision
  /\ s' = [s EXCEPT !.phase = "draft", !.step = "workspace", !.token = @+1,
       !.shown = 0, !.text = "valid", !.complete = TRUE,
       !.draftTarget = s.known[s.operation].target, !.draftBase = s.revision,
       !.cache = s.revision, !.validated = TRUE]
AcceptWorkspaceSnapshot ==
  /\ s.phase = "snapshot" /\ Command.kind = "workspace"
  /\ s.known[s.operation].target = "w1" /\ "w1" \in s.workspaces
  /\ s.workspaceProject = Command.target /\ s.revision >= s.known[s.operation].revision
  /\ s' = [s EXCEPT !.phase = "cleanup", !.validated = TRUE, !.cache = s.revision]
CompleteExactCleanup ==
  /\ s.phase = "cleanup" /\ s.validated
  /\ s' = [s EXCEPT !.phase = "install", !.viewer = FALSE, !.cleanupDone = TRUE]
InstallCreatedWorkspace ==
  /\ s.phase = "install" /\ s.cleanupDone
  /\ s' = [s EXCEPT !.phase = "done", !.selected = s.known[s.operation].target]
AcceptDirectorySnapshot ==
  /\ s.phase = "snapshot" /\ Command.kind = "directory"
  /\ s.known[s.operation].target = s.selected /\ s.selected \in s.directories
  /\ s.revision >= s.known[s.operation].revision
  /\ s' = [s EXCEPT !.phase = "done", !.validated = TRUE, !.cache = s.revision]
RecoverProjectThenAdvance == /\ s.recovered /\ AcceptProjectSnapshot
RecoverWorkspaceThenCleanup == /\ s.recovered /\ AcceptWorkspaceSnapshot
RecoverDirectoryWithoutLaunch == /\ s.recovered /\ AcceptDirectorySnapshot
ExplicitLaunch ==
  /\ s.phase = "done" /\ ~s.viewer /\ s.selected \in s.directories /\ s.launches = 0
  /\ s' = [s EXCEPT !.viewer = TRUE, !.launches = 1]
RejectLateResult == /\ s.phase \in {"done", "retired"} /\ s.operation > 0 /\ UNCHANGED vars
ExpirePending ==
  /\ s.phase \in Transport
  /\ s' = [s EXCEPT !.phase = "retired",
       !.outcome = IF CurrentKnown THEN "known"
         ELSE IF s.phase = "queued" THEN "not-submitted" ELSE "unknown"]

Next == OpenWorkspaceWizard \/ OpenDirectory \/ RejectDirectoryAdmission
  \/ BeginCompetingWork \/ FinishCompetingWork \/ RejectBusyOpen \/ RejectCompetingStructure
  \/ DrawExact \/ RevokeDraw \/ RejectStaleDraw \/ ChooseNewProject \/ ChooseExistingProject
  \/ AcceptProjectChoice \/ (\E value \in {"valid", "invalid"}: EditText(value))
  \/ BeginIncompleteUtf8 \/ FinishUtf8 \/ RejectUndrawnEnter \/ RejectNextStepSamePacket \/ RejectIncompleteEnter
  \/ RejectInvalidEnter \/ CancelDraft \/ CancelAfterKnownProject \/ InvalidateDraftContext
  \/ SubmitProject \/ SubmitWorkspace \/ SubmitDirectory \/ RejectPostSubmitInput
  \/ HandOffOnce \/ DrainEarlierAccepted \/ DefiniteNotSubmitted \/ PeerRevisionWrite \/ RefuseStaleRevision
  \/ RefuseBeforeCommit \/ ServerCommit \/ ReceiveKnownCommit \/ OutcomeUnknown
  \/ LoseCommittedReceipt \/ RejectIncompatibleTypedResult \/ RequestSnapshot
  \/ SnapshotUnavailable \/ RejectWrongSnapshotIdentity \/ ExplicitRefresh \/ RejectMutationReplay
  \/ AcceptProjectSnapshot \/ AcceptWorkspaceSnapshot \/ CompleteExactCleanup
  \/ RecoverProjectThenAdvance \/ RecoverWorkspaceThenCleanup \/ RecoverDirectoryWithoutLaunch
  \/ InstallCreatedWorkspace \/ AcceptDirectorySnapshot \/ ExplicitLaunch \/ RejectLateResult \/ ExpirePending

\* The existing carrier/cleanup owner's finite operation cutoff guarantees
\* settlement or retirement. It does not guarantee user input or refresh.
Spec == Init /\ [][Next]_vars /\ WF_vars(ExpirePending)

TypeOK ==
  /\ s.phase \in Phases /\ s.step \in Kinds \cup {"none", "picker"}
  /\ s.token \in 0..6 /\ s.shown \in 0..6 /\ s.text \in {"valid", "invalid"}
  /\ s.complete \in BOOLEAN /\ s.edited \in BOOLEAN /\ s.picker \in {"p0", "new"}
  /\ s.draftTarget \in Targets /\ s.draftBase \in 0..4 /\ s.opens \in 0..2
  /\ s.revision \in 0..4 /\ s.cache \in 0..4 /\ s.peerWrote \in BOOLEAN
  /\ s.projects \subseteq Projects /\ s.workspaces \subseteq Workspaces
  /\ s.directories \subseteq Workspaces /\ s.workspaceProject \in Projects \cup {"none"}
  /\ s.selected \in Workspaces /\ s.viewer \in BOOLEAN /\ s.operation \in 0..3
  /\ s.commands \in [Requests -> [kind : Kinds \cup {"none"}, target : Targets,
       base : 0..4, token : 0..6, site : {"site"}]]
  /\ s.sent \in [Requests -> 0..1]
  /\ s.committed \in [Requests -> [kind : Kinds \cup {"none"}, target : Targets,
       revision : 0..4, site : {"site"}]]
  /\ s.known \in [Requests -> [kind : Kinds \cup {"none"}, target : Targets,
       revision : 0..4, site : {"site"}]]
  /\ s.outcome \in {"none", "not-submitted", "precommit", "known", "unknown"}
  /\ s.outcomeKind \in Kinds \cup {"none"}
  /\ s.cleanupDone \in BOOLEAN /\ s.validated \in BOOLEAN /\ s.recovered \in BOOLEAN
  /\ s.budget \in 0..6 /\ s.busy \in BOOLEAN /\ s.launches \in 0..1
  /\ s.priorAccepted \in BOOLEAN
CanonicalMembership ==
  /\ s.selected \in s.workspaces /\ s.directories \subseteq s.workspaces
  /\ ("w1" \in s.workspaces => s.workspaceProject \in s.projects)
AtMostOnceHandoff == \A id \in Requests: s.sent[id] <= 1
HandoffDrainsEarlierAccepted == (\E id \in Requests: s.sent[id] = 1) => ~s.priorAccepted
KnownReceiptIsExact == \A id \in Requests:
  s.known[id] # NoReceipt => s.known[id] = s.committed[id] /\ s.sent[id] = 1
TypedOutcomeNamesCommand == s.operation > 0 => s.outcomeKind = Command.kind
SnapshotRequiresKnownReceipt == s.phase \in {"snapshot", "recovery", "cleanup", "install"} => CurrentKnown
NotSubmittedHasNoHandoff == s.outcome = "not-submitted" => s.sent[s.operation] = 0
RefusalHasNoCommit == s.outcome \in {"not-submitted", "precommit"} => s.committed[s.operation] = NoReceipt
UnknownRetiresCarrier == s.outcome = "unknown" => s.phase = "retired" /\ ~CurrentKnown
NewWorkspaceViewerNeedsExplicitLaunch == s.selected = "w1" /\ s.viewer => s.launches = 1
BusyExcludesDraft == Draft => ~s.busy

AdmissionRequiresDrawAndCompleteInput == [][Submit =>
  /\ CanEnter /\ s'.commands[s.operation+1].target = s.draftTarget
  /\ s'.commands[s.operation+1].base = s.draftBase]_vars
EachStepNeedsNewDraw == [][s'.step # s.step /\ s'.phase = "draft" =>
  s'.shown = 0 /\ s'.token > s.token]_vars
KnownCommitNeverRegresses == [][\A id \in Requests:
  s.known[id] # NoReceipt => s'.known[id] = s.known[id]]_vars
CommittedFactsNeverRollBack == [][
  s.projects \subseteq s'.projects /\ s.workspaces \subseteq s'.workspaces
  /\ s.directories \subseteq s'.directories]_vars
CancellationPreservesCanonicalFacts == [][CancelDraft =>
  UNCHANGED <<s.projects, s.workspaces, s.directories, s.known, s.selected, s.viewer>>]_vars
FrozenCommandNeverRetargets == [][\A id \in 1..s.operation:
  s'.commands[id] = s.commands[id]]_vars
OnlySubmitAllocatesMutation == [][s'.operation # s.operation => Submit]_vars
RecoveryNeverReplays == [][ExplicitRefresh =>
  UNCHANGED <<s.sent, s.operation, s.commands, s.known, s.committed>>]_vars
SameOperationKeepsBudget == [][s'.operation = s.operation /\ ~ExplicitRefresh =>
  s'.budget = s.budget]_vars
SelectionChangesOnlyAfterCleanup == [][s'.selected # s.selected =>
  InstallCreatedWorkspace /\ s.validated /\ s.cleanupDone /\ CurrentKnown]_vars
RefusalPreservesTerminal == [][s'.outcome \in {"not-submitted", "precommit"} /\ ~ExplicitLaunch =>
  UNCHANGED <<s.selected, s.viewer>>]_vars
SetupNeverLaunches == [][s'.launches # s.launches => ExplicitLaunch]_vars
RetiredCarrierCannotReplay == [][s.phase = "retired" => s'.sent = s.sent]_vars
PendingEventuallySettles == (s.phase \in Transport) ~> (s.phase \notin Transport)
=============================================================================
