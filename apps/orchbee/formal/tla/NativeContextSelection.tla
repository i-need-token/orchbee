-------------------------- MODULE NativeContextSelection --------------------------
EXTENDS Integers, Sequences, FiniteSets

CONSTANT MaxEpoch
ASSUME MaxEpoch \in Nat \ {0}

\* One Site, one standalone context, two distinct Workspace identities, two
\* retained bindings, four operation epochs and two navigation revisions.
\* Labels are deliberately absent: identity and order are authoritative.
\* Detailed mailbox transport, cleanup reserves, first-frame/controller setup
\* and membership pruning refine to the existing native Client models.
Standalone == "standalone"
WorkspaceA == "workspace-a"
WorkspaceB == "workspace-b"
Contexts == {Standalone, WorkspaceA, WorkspaceB}
NoTab == "no-tab"
\* Keep absence in the same record domain as exact Viewer keys so TLC can
\* compare and normalize the finite set without mixed string/record equality.
NoViewer == [context |-> "no-context", tab |-> NoTab, epoch |-> 0]
Tabs == {"standalone-tab", "workspace-tab"}
KnownOutcomes == {"started", "pending", "failed", "unavailable", "unknown"}
Provenances == KnownOutcomes \cup {"binding-only", "empty"}
Members(context) == CASE context = Standalone -> <<"standalone-tab">>
                        [] context = WorkspaceA -> <<"workspace-tab">>
                        [] OTHER -> <<>>
Selected(context) == IF Members(context) = <<>> THEN NoTab ELSE Head(Members(context))
Navigation(revision) == IF revision = 1
  THEN <<Standalone, WorkspaceA, WorkspaceB>>
  ELSE <<Standalone, WorkspaceB, WorkspaceA>>
Index(order, context) == CHOOSE index \in 1..Len(order): order[index] = context
Neighbor(order, context, direction) ==
  LET index == Index(order, context) IN
    IF direction = "next" THEN order[IF index = Len(order) THEN 1 ELSE index + 1]
    ELSE order[IF index = 1 THEN Len(order) ELSE index - 1]
Viewer(context, epoch) == [context |-> context, tab |-> Selected(context), epoch |-> epoch]
Viewers == {NoViewer} \cup
  {Viewer(context, epoch): context \in {Standalone, WorkspaceA}, epoch \in 0..MaxEpoch}
Phases == {"stable", "read", "cleanup-ready", "cleanup-wait", "install",
           "attach-ready", "attach-wait", "retired"}
PendingPhases == Phases \ {"stable", "retired"}

VARIABLE s
vars == <<s>>

\* known is the retained Create authority, independent of the active projection.
\* Workspace A's Create predates this trace. Its outcome is not changed by a
\* navigation read. Workspace B is canonically Empty, with no invented binding.
Provenance(state, context) ==
  IF Selected(context) = NoTab THEN "empty"
  ELSE IF context = Standalone THEN "binding-only" ELSE state.known
CanAttach(state) == state.selected # NoTab
  /\ state.provenance \in {"binding-only", "started"}
Visible(state) == <<state.active, state.selected, state.membership,
  state.provenance, state.viewer, state.navRevision, state.membershipRevision>>
Retired(state) == [state EXCEPT !.phase = "retired", !.inputFenced = TRUE]

Init == \E outcome \in KnownOutcomes:
  s = [phase |-> "stable", active |-> Standalone, requested |-> Standalone,
       selected |-> Selected(Standalone), membership |-> Members(Standalone),
       provenance |-> "binding-only", known |-> outcome,
       viewer |-> Viewer(Standalone, 0), oldViewer |-> NoViewer,
       inputFenced |-> FALSE, epoch |-> 0, intent |-> "refresh",
       serverRevision |-> 1, navRevision |-> 1, membershipRevision |-> 1,
       candidateRevision |-> 1, validated |-> FALSE, cleanupSettled |-> FALSE,
       installed |-> FALSE, busy |-> "none", closeTarget |-> NoTab,
       visited |-> {Standalone}, staleEvent |-> -1, notice |-> "none",
       command |-> "none"]

Admit(target, intent) ==
  /\ s.phase = "stable" /\ s.busy = "none" /\ s.epoch < MaxEpoch
  /\ s' = [s EXCEPT !.phase = "read", !.requested = target, !.intent = intent,
       !.epoch = @ + 1, !.validated = FALSE, !.cleanupSettled = FALSE,
       !.installed = FALSE, !.oldViewer = s.viewer, !.notice = "none",
       !.command = "snapshot"]
\* Keep each semantic branch as an action conjunction: TLC inlines a pure
\* operator alias and would report only Admit, losing direction/intent coverage.
SelectNext ==
  /\ s.phase = "stable"
  /\ Admit(Neighbor(Navigation(s.navRevision), s.active, "next"), "select")
SelectPrevious ==
  /\ s.phase = "stable"
  /\ Admit(Neighbor(Navigation(s.navRevision), s.active, "previous"), "select")
RefreshCurrent == /\ s.phase = "stable" /\ Admit(s.active, "refresh")
CoverageWrapNext == /\ s.active = Navigation(s.navRevision)[3] /\ SelectNext
CoverageWrapPrevious == /\ s.active = Standalone /\ SelectPrevious
CoverageLeaveEmpty == /\ s.selected = NoTab /\ SelectNext
CoverageLeaveEmptyPrevious == /\ s.selected = NoTab /\ SelectPrevious
CoverageRequestEmptyNext ==
  /\ Neighbor(Navigation(s.navRevision), s.active, "next") = WorkspaceB /\ SelectNext
CoverageRequestEmptyPrevious ==
  /\ Neighbor(Navigation(s.navRevision), s.active, "previous") = WorkspaceB /\ SelectPrevious
CoverageReturnKnown ==
  /\ s.active # WorkspaceA /\ WorkspaceA \in s.visited
  /\ Neighbor(Navigation(s.navRevision), s.active, "next") = WorkspaceA
  /\ SelectNext

BeginBusy(kind) ==
  /\ kind \in {"mutation", "close", "membership"}
  /\ s.phase = "stable" /\ s.busy = "none"
  /\ (kind = "close" => s.selected # NoTab)
  /\ s' = [s EXCEPT !.busy = kind,
       !.closeTarget = IF kind = "close" THEN s.selected ELSE NoTab]
SettleBusy ==
  /\ s.phase = "stable" /\ s.busy # "none"
  /\ s' = [s EXCEPT !.busy = "none", !.closeTarget = NoTab]
RejectBusySelection ==
  /\ s.phase # "retired"
  /\ (s.phase # "stable" \/ s.busy # "none" \/ s.epoch = MaxEpoch)
  /\ s' = [s EXCEPT !.notice = "busy"]
CoverageMutationBusy == /\ s.busy = "mutation" /\ RejectBusySelection
CoverageCloseBusy == /\ s.busy = "close" /\ RejectBusySelection
CoverageMembershipBusy == /\ s.busy = "membership" /\ RejectBusySelection

\* The Server alone changes navigation. A retained target survives a reorder
\* between admission and the read; candidate navigation and membership share
\* one validated revision. Missing owner/Site and unsupported target layouts
\* are definite refusal inputs, rather than fabricated Empty snapshots.
AdvanceServerRevision ==
  /\ s.phase # "retired" /\ s.serverRevision = 1
  /\ s' = [s EXCEPT !.serverRevision = 2]
ReadValidTarget ==
  /\ s.phase = "read"
  /\ s' = [s EXCEPT !.candidateRevision = s.serverRevision, !.validated = TRUE,
       !.phase = IF s.intent = "refresh" THEN "install"
                 ELSE IF s.viewer = NoViewer THEN "install" ELSE "cleanup-ready",
       !.cleanupSettled = (s.intent = "refresh" \/ s.viewer = NoViewer),
       !.inputFenced = IF s.intent = "select" THEN TRUE ELSE @]
RejectTarget(reason) ==
  /\ s.phase = "read" /\ reason \in {"unavailable", "unsupported"}
  /\ s' = [s EXCEPT !.phase = "stable", !.notice = reason]
RejectUnavailableTarget == /\ s.phase = "read" /\ RejectTarget("unavailable")
RejectUnsupportedTarget == /\ s.phase = "read" /\ RejectTarget("unsupported")
CoverageReorderedTarget ==
  /\ s.intent = "select" /\ s.serverRevision > s.navRevision /\ ReadValidTarget

RequestExactCleanup ==
  /\ s.phase = "cleanup-ready" /\ s.validated
  /\ s.viewer = s.oldViewer /\ s.viewer # NoViewer /\ s.inputFenced
  /\ s' = [s EXCEPT !.phase = "cleanup-wait", !.command = "detach"]
CompleteExactCleanup ==
  /\ s.phase = "cleanup-wait" /\ s.viewer = s.oldViewer
  /\ s' = [s EXCEPT !.viewer = NoViewer, !.cleanupSettled = TRUE, !.phase = "install"]
RetireAmbiguousCleanup == /\ s.phase = "cleanup-wait" /\ s' = Retired(s)

\* One state change abstracts the existing membership owner + mailbox commit.
\* Refresh keeps the current location and exact Viewer. Cross-context selection
\* installs the first canonical Tab or Empty, after old-Viewer cleanup.
InstallTarget ==
  /\ s.phase = "install" /\ s.validated /\ s.cleanupSettled
  /\ (s.intent = "select" => s.viewer = NoViewer)
  /\ s' = [s EXCEPT !.active = s.requested, !.selected = Selected(s.requested),
       !.membership = Members(s.requested), !.provenance = Provenance(s, s.requested),
       !.navRevision = s.candidateRevision, !.membershipRevision = s.candidateRevision,
       !.installed = TRUE, !.visited = @ \cup {s.requested},
       !.phase = IF s.intent = "refresh" THEN "stable" ELSE "attach-ready"]
CoverageEnterEmpty == /\ s.requested = WorkspaceB /\ InstallTarget
CoverageInstallKnownReturn ==
  /\ s.intent = "select" /\ s.requested = WorkspaceA
  /\ WorkspaceA \in s.visited /\ InstallTarget
CoverageRefreshWorkspace ==
  /\ s.intent = "refresh" /\ s.active # Standalone /\ InstallTarget

RequestFreshAttach ==
  /\ s.phase = "attach-ready" /\ CanAttach(s)
  /\ s.viewer = NoViewer /\ s.installed /\ s.cleanupSettled
  /\ s' = [s EXCEPT !.phase = "attach-wait", !.command = "attach"]
SettleWithoutAttach ==
  /\ s.phase = "attach-ready" /\ ~CanAttach(s)
  /\ s' = [s EXCEPT !.phase = "stable"]
CoverageKnownNonStarted ==
  /\ s.active = WorkspaceA /\ s.known # "started" /\ SettleWithoutAttach
CompleteFreshAttach ==
  /\ s.phase = "attach-wait"
  /\ s' = [s EXCEPT !.viewer = Viewer(s.active, s.epoch), !.inputFenced = FALSE,
       !.phase = "stable"]
FreshAttachUnavailable ==
  /\ s.phase = "attach-wait" /\ s' = [s EXCEPT !.phase = "stable"]
RetireAmbiguousAttach == /\ s.phase = "attach-wait" /\ s' = Retired(s)

\* A defensive replay of an earlier operation result is independently delivered
\* during a later generation. This explicit slot makes rejection reachable and
\* observable without giving a stale result permission to settle the new flight.
InjectStaleEpoch ==
  /\ s.phase \in PendingPhases /\ s.epoch >= 2 /\ s.staleEvent = -1
  /\ s' = [s EXCEPT !.staleEvent = s.epoch - 1]
RejectStaleEpoch ==
  /\ s.staleEvent >= 0 /\ s.staleEvent < s.epoch
  /\ s' = [s EXCEPT !.staleEvent = -1, !.notice = "stale"]
RejectFencedOldFrame ==
  /\ s.phase \in {"cleanup-ready", "cleanup-wait", "install", "attach-ready", "attach-wait"}
  /\ s.intent = "select" /\ s.inputFenced /\ s.oldViewer # NoViewer
  /\ s' = [s EXCEPT !.notice = "old-frame"]

\* Weak fairness abstracts the existing monotonic deadline: an unresolved
\* operation eventually expires. No fairness of remote success is assumed.
\* Retirement fences access and preserves any unresolved exact Viewer fact;
\* it never synthesizes a successful Detach or retries an ambiguous effect.
ExpireOperation == /\ s.phase \in PendingPhases /\ s' = Retired(s)

Next ==
  \/ SelectNext \/ SelectPrevious \/ RefreshCurrent
  \/ CoverageWrapNext \/ CoverageWrapPrevious \/ CoverageLeaveEmpty \/ CoverageReturnKnown
  \/ CoverageLeaveEmptyPrevious \/ CoverageRequestEmptyNext \/ CoverageRequestEmptyPrevious
  \/ \E kind \in {"mutation", "close", "membership"}: BeginBusy(kind)
  \/ SettleBusy \/ RejectBusySelection
  \/ CoverageMutationBusy \/ CoverageCloseBusy \/ CoverageMembershipBusy
  \/ AdvanceServerRevision \/ ReadValidTarget \/ CoverageReorderedTarget
  \/ RejectUnavailableTarget \/ RejectUnsupportedTarget
  \/ RequestExactCleanup \/ CompleteExactCleanup \/ RetireAmbiguousCleanup
  \/ InstallTarget \/ CoverageEnterEmpty \/ CoverageInstallKnownReturn \/ CoverageRefreshWorkspace
  \/ RequestFreshAttach \/ SettleWithoutAttach \/ CoverageKnownNonStarted
  \/ CompleteFreshAttach \/ FreshAttachUnavailable \/ RetireAmbiguousAttach
  \/ InjectStaleEpoch \/ RejectStaleEpoch \/ RejectFencedOldFrame \/ ExpireOperation

Spec == Init /\ [][Next]_vars /\ WF_vars(ExpireOperation)

TypeOK ==
  /\ s.phase \in Phases /\ s.active \in Contexts /\ s.requested \in Contexts
  /\ s.selected \in Tabs \cup {NoTab}
  /\ s.membership \in {Members(context): context \in Contexts}
  /\ s.known \in KnownOutcomes /\ s.provenance \in Provenances
  /\ s.viewer \in Viewers /\ s.oldViewer \in Viewers
  /\ s.epoch \in 0..MaxEpoch /\ s.intent \in {"select", "refresh"}
  /\ s.serverRevision \in 1..2 /\ s.navRevision \in 1..2
  /\ s.membershipRevision \in 1..2 /\ s.candidateRevision \in 1..2
  /\ s.busy \in {"none", "mutation", "close", "membership"}
  /\ s.closeTarget \in Tabs \cup {NoTab} /\ s.visited \subseteq Contexts
  /\ s.staleEvent \in {-1} \cup 0..MaxEpoch
  /\ s.notice \in {"none", "busy", "unavailable", "unsupported", "stale", "old-frame"}
  /\ s.command \in {"none", "snapshot", "detach", "attach", "create", "start", "dispose",
                     "navigation-mutation"}
  /\ \A flag \in {s.inputFenced, s.validated, s.cleanupSettled, s.installed}: flag \in BOOLEAN
AtomicLocationProjection ==
  /\ s.membership = Members(s.active) /\ s.selected = Selected(s.active)
  /\ s.navRevision = s.membershipRevision /\ s.navRevision <= s.serverRevision
KnownCreateCannotBecomeBindingOnly == s.provenance = Provenance(s, s.active)
EmptyHasNoViewer == s.selected = NoTab => s.viewer = NoViewer
ExactCurrentViewer == s.viewer # NoViewer =>
  /\ s.viewer.context = s.active /\ s.viewer.tab = s.selected
  /\ s.provenance \in {"binding-only", "started"}
ValidatedBeforeCleanup == s.phase \in {"cleanup-ready", "cleanup-wait"} =>
  /\ s.validated /\ s.inputFenced /\ s.viewer = s.oldViewer
InstallationRequiresCleanup == s.installed => s.validated /\ s.cleanupSettled
FreshViewerUsesCurrentEpoch ==
  s.intent = "select" /\ s.installed /\ s.viewer # NoViewer => s.viewer.epoch = s.epoch
CloseKeepsExactTarget == s.busy = "close" => s.closeTarget = s.selected /\ s.phase = "stable"
RefreshRetainsLocation == s.intent = "refresh" => s.requested = s.active
\* This observable command projection is refined by carrier command traces;
\* it does not establish the implementation's transport behavior by itself.
NoImplicitCreateOrDispose == s.command \notin {"create", "start", "dispose", "navigation-mutation"}

LocationChangesOnlyAtInstall == [][s.active' # s.active => InstallTarget]_vars
KnownCreateNeverForgotten == [][s.known' = s.known]_vars
BusyPreservesOperation == [][RejectBusySelection =>
  /\ Visible(s') = Visible(s) /\ s.requested' = s.requested /\ s.epoch' = s.epoch
  /\ s.closeTarget' = s.closeTarget]_vars
DefiniteRefusalPreservesLocation ==
  [][(RejectUnavailableTarget \/ RejectUnsupportedTarget) => Visible(s') = Visible(s)]_vars
StaleEpochCannotSettle == [][RejectStaleEpoch =>
  /\ Visible(s') = Visible(s) /\ s.phase' = s.phase /\ s.requested' = s.requested
  /\ s.epoch' = s.epoch /\ s.validated' = s.validated /\ s.command' = s.command]_vars
FencedOldFrameCannotPublish == [][RejectFencedOldFrame => Visible(s') = Visible(s)]_vars
NavigationIsServerOwned == [][s.serverRevision' # s.serverRevision => AdvanceServerRevision]_vars
RefreshPreservesExactViewer == [][InstallTarget /\ s.intent = "refresh" => s.viewer' = s.viewer]_vars
OperationEventuallySettles == (s.phase \in PendingPhases) ~> (s.phase \in {"stable", "retired"})
=============================================================================
