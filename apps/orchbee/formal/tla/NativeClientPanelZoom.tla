--------------------------- MODULE NativeClientPanelZoom ----------------------------
EXTENDS Integers, Sequences, FiniteSets

\* Abstracts the Client-local per-Tab zoom flag lifecycle owned by
\* NativeSessionState: two Workbench contexts of two Tabs each, latest-known
\* membership split between the current cache and retained context snapshots,
\* zoom flag setting/following/GC, and the drawn-vs-observed set split.
\* Panel counts collapse to {0,1,2} — the contract only distinguishes
\* deleted, single-leaf, and multi-leaf. Focus is a leaf index inside the
\* active Tab. Real rectangles, viewer/controller lifecycle, transport,
\* hit-map incarnation identity, and the Minimal/TooSmall single-leaf
\* degradation refine to the existing native Client models; this module owns
\* only the volatile zoom state machine and its GC invariant.
CONSTANT MaxRetainedContexts
ASSUME MaxRetainedContexts \in Nat \ {0}

Contexts == {"context-a", "context-b"}
TabsOf(context) ==
  IF context = "context-a" THEN {"a1", "a2"} ELSE {"b1", "b2"}
AllTabs == UNION {TabsOf(context) : context \in Contexts}
ContextOf(tab) == CHOOSE context \in Contexts : tab \in TabsOf(context)
\* 0 = deleted, 1 = single leaf, 2 = multi leaf.
Counts == {0, 1, 2}
LeavesOf(count) == 1..count
CandidatesFor(context) == [TabsOf(context) -> Counts]
Notices == {"none", "busy", "zoom-noop"}

VARIABLE s
vars == <<s>>

\* A retained entry snapshots one departed context's last known membership.
\* The set is bounded by MaxRetainedContexts — the ceiling the implementation
\* derives from MAXIMUM_TERMINAL_CONTEXTS on its retained context list. With
\* two modeled contexts at most one entry can exist, so RecycleRetained is
\* the abstraction of both navigation removal and capacity eviction.
IsRetainedEntry(entry) ==
  /\ entry.context \in Contexts
  /\ DOMAIN entry.counts = TabsOf(entry.context)

\* Latest-known membership: the current cache answers first for its own
\* tabs; a departed context's tabs fall back to its retained snapshot;
\* anything else is unknown (0) and must not keep a flag alive.
LatestKnown(cache, retained, tab) ==
  IF tab \in DOMAIN cache THEN cache[tab]
  ELSE IF \E entry \in retained:
            /\ entry.context = ContextOf(tab) /\ tab \in DOMAIN entry.counts
       THEN (CHOOSE entry \in retained:
               /\ entry.context = ContextOf(tab)
               /\ tab \in DOMAIN entry.counts).counts[tab]
       ELSE 0

PruneAgainst(cache, retained, flags) ==
  {tab \in flags : LatestKnown(cache, retained, tab) > 1}

ZoomActive(state) ==
  /\ state.activeTab \in state.zoomed /\ state.cache[state.activeTab] > 1
Observed(state) == LeavesOf(state.cache[state.activeTab])
Drawn(state) ==
  IF ZoomActive(state) THEN {state.focus} ELSE Observed(state)

\* The abstraction presupposes a live active Tab: an all-deleted candidate
\* has no Tab to select and refines to the session's empty/retire lifecycle,
\* which this module does not model. Every install therefore only admits a
\* candidate that keeps at least one live Tab.
HasLiveTab(cache) == \E tab \in DOMAIN cache : cache[tab] >= 1

\* The whole record an install produces — same-context refresh merge and
\* context switch both end here, so the prune always runs on the freshest
\* union of cache and retained truth.
InstallRecord(state, current, cache, retained) ==
  LET nextActive == CHOOSE tab \in DOMAIN cache : cache[tab] >= 1
  IN [state EXCEPT
       !.current = current, !.cache = cache, !.retained = retained,
       !.activeTab =
         IF state.activeTab \in DOMAIN cache /\ cache[state.activeTab] >= 1
         THEN state.activeTab ELSE nextActive,
       !.focus = 1,
       !.zoomed = PruneAgainst(cache, retained, @),
       !.notice = "none"]

Init ==
  \E current \in Contexts:
    \E candidate \in CandidatesFor(current):
      \E active \in DOMAIN candidate:
        \E focus \in {1, 2}:
          /\ candidate[active] >= 1 /\ focus <= candidate[active]
          /\ s = [current |-> current, activeTab |-> active, cache |-> candidate,
                  retained |-> {}, zoomed |-> {}, focus |-> focus,
                  busy |-> FALSE, notice |-> "none"]

\* z admits the same guard the local focus transition owns: an unguarded
\* phase. A guarded phase keeps the flag untouched — the local no-op the
\* focus bindings already perform.
ToggleZoom ==
  /\ ~s.busy /\ s.cache[s.activeTab] > 1
  /\ s' = [s EXCEPT
       !.zoomed = IF s.activeTab \in @ THEN @ \ {s.activeTab}
                  ELSE @ \cup {s.activeTab},
       !.notice = "none"]
ToggleZoomOn == /\ s.activeTab \notin s.zoomed /\ ToggleZoom
ToggleZoomOff == /\ s.activeTab \in s.zoomed /\ ToggleZoom
\* A single-leaf Tab is an honest no-op: no flag, no badge, state unchanged
\* except the localized notice slot.
ToggleZoomSingleLeafNoOp ==
  /\ ~s.busy /\ s.cache[s.activeTab] = 1
  /\ s' = [s EXCEPT !.notice = "zoom-noop"]
RejectBusyToggle ==
  /\ s.busy
  /\ s' = [s EXCEPT !.notice = "busy"]

\* hjkl direction navigation refines to a leaf index change inside the
\* active Tab's canonical tree. The zoom flag is untouched; when zoomed the
\* drawn set follows the new focus.
FocusMoveTo(index) ==
  /\ ~s.busy /\ index \in LeavesOf(s.cache[s.activeTab]) /\ index # s.focus
  /\ s' = [s EXCEPT !.focus = index, !.notice = "none"]
FocusMoveZoomed ==
  /\ ZoomActive(s)
  /\ \E index \in LeavesOf(s.cache[s.activeTab]): FocusMoveTo(index)
FocusMoveUnzoomed ==
  /\ ~ZoomActive(s)
  /\ \E index \in LeavesOf(s.cache[s.activeTab]): FocusMoveTo(index)

\* Selecting a sibling Tab inside the same context keeps every zoom flag —
\* the memory is per-Tab, so returning to the flagged Tab restores the view.
SelectTabWithin(target) ==
  /\ ~s.busy /\ target \in DOMAIN s.cache /\ target # s.activeTab
  /\ s.cache[target] >= 1
  /\ \E focus \in LeavesOf(s.cache[target]):
     s' = [s EXCEPT !.activeTab = target, !.focus = focus, !.notice = "none"]
CoverageSelectSiblingLeavingFlag ==
  /\ s.activeTab \in s.zoomed
  /\ \E target \in DOMAIN s.cache: SelectTabWithin(target)

\* Context switch: the commit retains the OLD context's snapshot before
\* installing the NEW candidate (the implementation's commit order),
\* consumes any stale retained entry for the target, then prunes against
\* the union of the fresh cache and the updated retained set. The candidate
\* is the fresh membership read — remote mutations the Client could not
\* have seen arrive exactly here.
SwitchContext(target) ==
  /\ ~s.busy /\ target # s.current
  /\ \E candidate \in CandidatesFor(target):
    LET retainedAfter ==
          (s.retained \ {entry \in s.retained :
              entry.context = target \/ entry.context = s.current})
          \cup {[context |-> s.current, counts |-> s.cache]}
    IN /\ HasLiveTab(candidate)
       /\ Cardinality(retainedAfter) <= MaxRetainedContexts
       /\ s' = InstallRecord(s, target, candidate, retainedAfter)
\* Departing while the active Tab is zoomed: the flag leaves the cache and
\* survives on the retained snapshot — the lazy-but-live entry the GC keeps.
CoverageDepartZoomed ==
  /\ ZoomActive(s) /\ \E target \in Contexts: SwitchContext(target)
\* Returning to a context whose retained snapshot still proves the flag: the
\* fresh install keeps it when the candidate agrees the Tab is multi-leaf.
CoverageReturnRestores ==
  /\ \E entry \in s.retained:
    /\ entry.context # s.current
    /\ \E tab \in DOMAIN entry.counts:
      /\ tab \in s.zoomed /\ entry.counts[tab] > 1
    /\ SwitchContext(entry.context)

\* Same-context refresh merge: the candidate replaces the cache; shrinking
\* to a single leaf or deleting prunes the affected flags immediately.
RefreshMerge ==
  /\ ~s.busy
  /\ \E candidate \in CandidatesFor(s.current):
    /\ HasLiveTab(candidate)
    /\ s' = InstallRecord(s, s.current, candidate, s.retained)
CoverageShrinkActiveToSingle ==
  /\ s.activeTab \in s.zoomed
  /\ \E candidate \in CandidatesFor(s.current):
    /\ HasLiveTab(candidate)
    /\ candidate[s.activeTab] = 1
    /\ s' = InstallRecord(s, s.current, candidate, s.retained)
CoverageDeleteZoomedTab ==
  /\ \E tab \in DOMAIN s.cache: tab \in s.zoomed
  /\ \E candidate \in CandidatesFor(s.current):
    /\ HasLiveTab(candidate)
    /\ \E tab \in DOMAIN candidate:
      /\ tab \in s.zoomed /\ candidate[tab] = 0
    /\ s' = InstallRecord(s, s.current, candidate, s.retained)

\* A retained entry is recycled — navigation removal or the bounded
\* retention's capacity eviction. Flags that only survived on that snapshot
\* lose their last known membership and are pruned.
RecycleRetained ==
  /\ ~s.busy /\ s.retained # {}
  /\ \E entry \in s.retained:
    LET retainedAfter == s.retained \ {entry}
    IN s' = [s EXCEPT !.retained = retainedAfter,
         !.zoomed = PruneAgainst(s.cache, retainedAfter, @),
         !.notice = "none"]
CoverageRecycleDropsFlag ==
  /\ \E entry \in s.retained:
    /\ \E tab \in DOMAIN entry.counts: tab \in s.zoomed
    /\ RecycleRetained

BeginBusy == /\ ~s.busy /\ s' = [s EXCEPT !.busy = TRUE, !.notice = "none"]
SettleBusy == /\ s.busy /\ s' = [s EXCEPT !.busy = FALSE, !.notice = "none"]

SelectSiblingTab == \E target \in DOMAIN s.cache: SelectTabWithin(target)

Next ==
  \/ ToggleZoomOn \/ ToggleZoomOff \/ ToggleZoomSingleLeafNoOp
  \/ RejectBusyToggle
  \/ FocusMoveZoomed \/ FocusMoveUnzoomed
  \/ SelectSiblingTab \/ CoverageSelectSiblingLeavingFlag
  \/ SwitchContext("context-a") \/ SwitchContext("context-b")
  \/ CoverageDepartZoomed \/ CoverageReturnRestores
  \/ RefreshMerge \/ CoverageShrinkActiveToSingle \/ CoverageDeleteZoomedTab
  \/ RecycleRetained \/ CoverageRecycleDropsFlag
  \/ BeginBusy \/ SettleBusy

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.current \in Contexts /\ s.activeTab \in AllTabs
  /\ s.activeTab \in TabsOf(s.current)
  /\ DOMAIN s.cache = TabsOf(s.current)
  /\ s.cache \in [TabsOf(s.current) -> Counts]
  /\ \A entry \in s.retained: IsRetainedEntry(entry)
  /\ s.zoomed \subseteq AllTabs
  /\ s.focus \in {1, 2} /\ s.busy \in BOOLEAN /\ s.notice \in Notices
\* Every flag rides a latest-known multi-leaf membership — current cache
\* first, then the departed context's retained snapshot. Flags whose last
\* known truth is deleted or single-leaf are collected at the install that
\* learned it, never lazily at read time.
ZoomFlagsRideLatestKnown ==
  \A tab \in s.zoomed: LatestKnown(s.cache, s.retained, tab) > 1
\* Read-side adjudication is current-cache only: the active Tab renders
\* zoomed exactly while its flag survives AND its cache membership stays
\* multi-leaf — a stale retained flag can never paint a zoom.
ZoomAdjudicatesCurrentCache ==
  s.activeTab \in s.zoomed => s.cache[s.activeTab] > 1
\* The three-set contract: zoomed drawing is exactly the focused leaf while
\* observation keeps every canonical leaf — hidden leaves stay attached.
ZoomedDrawnIsFocusedLeaf == ZoomActive(s) => Drawn(s) = {s.focus}
UnzoomedDrawsEveryLeaf == ~ZoomActive(s) => Drawn(s) = Observed(s)
ObservedKeepsEveryLeaf ==
  Observed(s) = LeavesOf(s.cache[s.activeTab])
FocusInsideActiveLeaves == s.focus \in LeavesOf(s.cache[s.activeTab])
ActiveTabIsAlive == s.cache[s.activeTab] >= 1
\* Retained snapshots belong to departed contexts and stay inside the
\* bounded retention the real MAXIMUM_TERMINAL_CONTEXTS ceiling motivates.
RetainedSnapshotsAreDeparted ==
  \A entry \in s.retained: entry.context # s.current
RetainedBound == Cardinality(s.retained) <= MaxRetainedContexts

\* A flag is only ever minted by the toggle on the current active Tab;
\* every other transition can only keep or prune.
ZoomFlagsMintedOnlyByToggle ==
  [][s.zoomed' \ s.zoomed # {} =>
      /\ ToggleZoom
      /\ s.zoomed' \ s.zoomed = {s.activeTab}]_vars
\* Toggling the flag never shrinks observation — hidden leaves keep their
\* canonical geometry and stay alive either way.
ToggleKeepsObservation ==
  [][ToggleZoom => Observed(s') = Observed(s)]_vars
\* A successful zoomed focus move redraws exactly the new focus — the zoom
\* follows, never widens.
ZoomedFocusMoveRedrawsFocus ==
  [][FocusMoveZoomed => Drawn(s') = {s.focus'}]_vars
\* Cache truth only changes through an install — the merge or switch points,
\* never lazily inside a read path.
CacheChangesOnlyAtInstall ==
  [][s.cache' # s.cache =>
      \/ SwitchContext("context-a") \/ SwitchContext("context-b")
      \/ RefreshMerge \/ CoverageShrinkActiveToSingle \/ CoverageDeleteZoomedTab]_vars
=============================================================================
