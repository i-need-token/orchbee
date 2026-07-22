import {
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from 'react';
import {
	HoverCardUpdateType,
	TabHoverCardController,
	TabHoverCardThumbnailRuntime,
} from '../../hovercard';
import { TabActive, TabOpen, TabPinned, type TabData, type TabGroupData } from '../../model';
import {
	applyHorizontalGroupDragCommands,
	assertHorizontalGroupDragRefinement,
	buildDraggedTabsLayout,
	calculateGroupForDraggedTabs,
	calculateHorizontalMoveThreshold,
	createArmedHorizontalGroupDrag,
	getDraggingViewsBoundsAtPoint,
	getHorizontalDragTarget,
	getTargetBeforeDraggedTabs,
	shouldEnterVerticalTabGroup,
	shouldRemainInVerticalTabGroup,
	transitionHorizontalGroupDrag,
} from '../../dragging';
import {
	TAB_CLOSE_TOUCH_RESIZE_DELAY_MS,
	TabLayoutState,
	TabWidthConstraints,
	calculateTabBounds,
	calculateTabClosingModeWidth,
	getPinnedTabContainerDragAxes,
	getTabClosingModeWatchBounds,
	shouldExitTabClosingMode,
	tabCollectionEaseInOut,
	tabClosingModeContainsPoint,
	type AnimatedLayoutFrame,
	type TabCloseSource,
} from '../../layout';
import { requireDefined, TabStripOrientation, VerticalTabStripCollapseState } from '../../common';
import { DEFAULT_TAB_STYLE_METRICS, LAYOUT_CONSTANTS } from '../../style';
import { TabGroupEditorBubbleTracker } from '../../groups';
import { ExpandOnHoverLockType, VerticalTabStripExpandOnHoverController } from '../../vertical';
import { TabCollectionAnimationRuntime } from '../utils/TabCollectionAnimationRuntime';
import { TabStripComboButtonController } from '../../shared';
import { getTabGroupMembershipAnnouncement, getTabMoveAnnouncement } from '../../accessibility';
import { useElementSize } from './useElementSize';
import type { TabStripViewModel } from '../types/tab-strip-view-model';
import {
	TAB_SIZE,
	TAB_BOUNDS_ANIMATION_DURATION_MS,
	requestManagedAnimationFrame,
	cancelManagedAnimationFrame,
	GROUP_COLORS,
	packHorizontalDragViews,
	getSourceVisualId,
	getVerticalDragVisualViews,
	captureDragVisuals,
	buildDragSessionData,
	areDraggedTabIdsConsecutive,
	buildVisualItems,
	groupHeaderWidth,
	sizeInfoForItem,
	groupForDragInsertion,
	type DragState,
	type MenuState,
	type HoverState,
	type TabStripProps,
} from '../utils/tab-strip-internals';

export function useTabStrip({
	model,
	orientation = TabStripOrientation.Horizontal,
	verticalCollapsed = false,
	verticalExpandOnHover = true,
	verticalExpandOnHoverController,
	hoverCardController: externalHoverCardController,
	getHoverCardThumbnailSource,
	showHoverCardPreviews = true,
	showHoverCardMemoryUsage = true,
	showGroupHoverCards = true,
	onVerticalExpandedOnHoverChange,
	showVerticalTopControls = true,
	showVerticalTabSearchButton = false,
	verticalGroupFocused = false,
	showPinnedTabsInFocusedGroups = false,
	verticalTabSearchOpen = false,
	verticalComboButtonController: externalComboButtonController,
	verticalToolbarHeight = 0,
	verticalCaptionButtonWidth = 0,
	verticalIsExitingExpandOnHover = false,
	onVerticalCollapsedChange,
	onVerticalUnfocusGroup,
	onVerticalTabSearchClick,
	onVerticalExpandOnHoverChange,
	showNewTabButton = true,
	horizontalFrameCondensed = false,
	touchUi = false,
	tabStripDeclutter = true,
	createCrossWindowDragRuntime,
	onDraggingChange,
	onTabKeyboardFocusChanged,
	onMoveTabsToNewWindow,
	onMoveGroupToNewWindow,
	enableTabGroupFocusing = false,
	ariaLabel = 'Tabs',
	onNewTab,
	onNewTabButtonPressed,
}: TabStripProps): TabStripViewModel {
	const snapshot = useSyncExternalStore(model.subscribe, model.getSnapshot, model.getSnapshot);
	const horizontal = orientation === TabStripOrientation.Horizontal;
	const focusedGroup =
		snapshot.focusedGroupId === null
			? null
			: (snapshot.groups.find((group) => group.id === snapshot.focusedGroupId) ?? null);
	const groupFocusActive = focusedGroup !== null;
	const tabVisibleWhileFocused = useCallback(
		(tab: Readonly<TabData>) =>
			!groupFocusActive ||
			tab.group === focusedGroup?.id ||
			(showPinnedTabsInFocusedGroups && tab.pinned),
		[focusedGroup?.id, groupFocusActive, showPinnedTabsInFocusedGroups],
	);
	const [stripRef, size] = useElementSize<HTMLDivElement>();
	const items = useMemo(() => buildVisualItems(snapshot), [snapshot]);
	const horizontalAnimationRuntimeRef = useRef<TabCollectionAnimationRuntime | null>(null);
	if (horizontalAnimationRuntimeRef.current === null) {
		horizontalAnimationRuntimeRef.current = new TabCollectionAnimationRuntime();
	}
	const [animatedHorizontalFrame, setAnimatedHorizontalFrame] =
		useState<AnimatedLayoutFrame | null>(null);
	const horizontalAnimationRequestRef = useRef<number | null>(null);
	const [menu, setMenu] = useState<MenuState | null>(null);
	const [hover, setHover] = useState<HoverState | null>(null);
	// Kept as a compatibility hook so a pre-controller Fast Refresh instance
	// can still run its legacy cleanup closure without referencing a removed hook.
	const hoverTimer = useRef<number | null>(null);
	const [drag, setDrag] = useState<DragState | null>(null);
	const [closingTabIds, setClosingTabIds] = useState<ReadonlySet<string>>(() => new Set());
	const initialFullyCollapsedGroups = snapshot.groups
		.filter((group) => group.collapsed)
		.map((group) => group.id);
	const [fullyCollapsedGroupIds, setFullyCollapsedGroupIds] = useState<ReadonlySet<string>>(
		() => new Set(initialFullyCollapsedGroups),
	);
	const [tabClosingMode, setTabClosingMode] = useState<{
		width: number;
		source: Exclude<TabCloseSource, 'non-ui'>;
	} | null>(null);
	const [announcement, setAnnouncement] = useState({ text: '', sequence: 0 });
	const closingTimers = useRef(new Map<string, number>());
	const collapsedGroupTimers = useRef(new Map<string, number>());
	const fullyCollapsedGroupIdsRef = useRef(new Set(initialFullyCollapsedGroups));
	const middlePressRef = useRef<{ tabId: string; pointerId: number } | null>(null);
	const touchDragTimer = useRef<number | null>(null);
	const dragSettleTimer = useRef<number | null>(null);
	const dragRef = useRef<DragState | null>(null);
	const previousTabCountRef = useRef(snapshot.tabs.length);
	const suppressGroupToggleRef = useRef<string | null>(null);
	const previousDraggingRef = useRef(false);
	const onDraggingChangeRef = useRef(onDraggingChange);
	const groupEditorTrackerRef = useRef<TabGroupEditorBubbleTracker | null>(null);
	const previousOrientationRef = useRef(orientation);
	if (groupEditorTrackerRef.current === null) {
		groupEditorTrackerRef.current = new TabGroupEditorBubbleTracker();
	}
	const groupEditorTracker = groupEditorTrackerRef.current;
	const openGroupEditor = useCallback(
		(id: string, x: number, y: number) => {
			groupEditorTracker.open();
			setMenu({ kind: 'group', id, x, y });
		},
		[groupEditorTracker],
	);
	dragRef.current = drag;
	onDraggingChangeRef.current = onDraggingChange;

	useEffect(() => {
		const started = drag?.started === true && !drag.settling;
		if (started === previousDraggingRef.current) return;
		previousDraggingRef.current = started;
		onDraggingChangeRef.current?.(started);
	}, [drag?.settling, drag?.started]);

	useEffect(
		() => () => {
			if (touchDragTimer.current !== null) {
				window.clearTimeout(touchDragTimer.current);
				touchDragTimer.current = null;
			}
			if (dragSettleTimer.current !== null) {
				window.clearTimeout(dragSettleTimer.current);
				dragSettleTimer.current = null;
			}
			if (!previousDraggingRef.current) return;
			previousDraggingRef.current = false;
			onDraggingChangeRef.current?.(false);
		},
		[],
	);

	useEffect(() => {
		if (menu?.kind === 'group') return;
		groupEditorTracker.close('explicit');
	}, [groupEditorTracker, menu?.kind]);

	useEffect(() => {
		if (!menu) return;
		let targetStillExists = true;
		if (menu.kind === 'group')
			targetStillExists = snapshot.groups.some((group) => group.id === menu.id);
		else if (menu.kind === 'tab')
			targetStillExists = snapshot.tabs.some((tab) => tab.id === menu.id);
		if (targetStillExists) return;
		setMenu(null);
		groupEditorTracker.close('explicit');
	}, [groupEditorTracker, menu, snapshot.groups, snapshot.tabs]);

	useEffect(() => {
		if (!menu) return;
		const dismissOnOutsidePointer = (event: PointerEvent) => {
			const target = event.target;
			if (target instanceof Element && target.closest('[data-slot="context-menu-content"]')) return;
			const header =
				target instanceof Element ? target.closest<HTMLElement>('[data-group-id]') : null;
			if (menu.kind === 'group' && header?.dataset['groupId'] === menu.id) {
				suppressGroupToggleRef.current = menu.id;
			}
			setMenu(null);
		};
		window.addEventListener('pointerdown', dismissOnOutsidePointer, true);
		return () => window.removeEventListener('pointerdown', dismissOnOutsidePointer, true);
	}, [menu]);

	useEffect(() => {
		if (previousOrientationRef.current === orientation) return;
		previousOrientationRef.current = orientation;
		if (!groupEditorTracker.onVerticalTabStripModeWillChange()) return;
		setMenu((current) => (current?.kind === 'group' ? null : current));
	}, [groupEditorTracker, orientation]);

	useEffect(
		() => () => {
			const current = dragRef.current;
			if (!current?.started || current.settling) return;
			current.crossWindowRuntime?.end('cancel');
			if (horizontal && current.horizontalGroupDrag !== null) {
				const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
					type: 'cancel',
				});
				applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
				assertHorizontalGroupDragRefinement(model, canceled.state);
			} else {
				model.restoreCanceledDrag(current.restoreState);
			}
		},
		[horizontal, model],
	);

	useEffect(() => {
		const current = dragRef.current;
		if (!current?.started || current.settling) return;
		const liveIds = new Set(snapshot.tabs.map((tab) => tab.id));
		if (current.tabIds.every((id) => liveIds.has(id))) return;
		const runtimeSnapshot = current.crossWindowRuntime?.getSnapshot?.();
		const transferredToAnotherContext =
			runtimeSnapshot?.sourceContextId !== undefined &&
			runtimeSnapshot.attachedContextId !== undefined &&
			runtimeSnapshot.attachedContextId !== runtimeSnapshot.sourceContextId;
		if (transferredToAnotherContext) return;
		current.crossWindowRuntime?.end('tab-destroyed');
		announce('Tab move canceled');
		setDrag(null);
	}, [snapshot.revision, snapshot.tabs]);

	const announce = (text: string) => {
		setAnnouncement((current) => ({ text, sequence: current.sequence + 1 }));
	};

	useEffect(
		() =>
			model.subscribeBeforeTabAdded(() => {
				const current = dragRef.current;
				if (!current?.started || current.settling || current.groupHeaderId === null) return;
				const runtimeSnapshot = current.crossWindowRuntime?.getSnapshot?.();
				const transferredToAnotherContext =
					runtimeSnapshot?.sourceContextId !== undefined &&
					runtimeSnapshot.attachedContextId !== undefined &&
					runtimeSnapshot.attachedContextId !== runtimeSnapshot.sourceContextId;
				current.crossWindowRuntime?.end(transferredToAnotherContext ? 'complete' : 'cancel');
				if (!transferredToAnotherContext) {
					if (horizontal && current.horizontalGroupDrag !== null) {
						const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
							type: 'cancel',
						});
						applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
						assertHorizontalGroupDragRefinement(model, canceled.state);
					} else {
						model.restoreCanceledDrag(current.restoreState);
					}
					announce('Group move canceled');
				}
				dragRef.current = null;
				setDrag(null);
			}),
		[horizontal, model],
	);

	const internalHoverCardController = useRef<TabHoverCardController | null>(null);
	if (internalHoverCardController.current === null) {
		internalHoverCardController.current = new TabHoverCardController();
	}
	const hoverCardController = externalHoverCardController ?? internalHoverCardController.current;
	const hoverCardSnapshot = useSyncExternalStore(
		hoverCardController.subscribe,
		hoverCardController.getSnapshot,
		hoverCardController.getSnapshot,
	);
	const thumbnailRuntimeRef = useRef<TabHoverCardThumbnailRuntime<string> | null>(null);
	if (thumbnailRuntimeRef.current === null)
		thumbnailRuntimeRef.current = new TabHoverCardThumbnailRuntime<string>();
	const thumbnailRuntime = thumbnailRuntimeRef.current;
	const thumbnailSnapshot = useSyncExternalStore(
		thumbnailRuntime.subscribe,
		thumbnailRuntime.getSnapshot,
		thumbnailRuntime.getSnapshot,
	);
	const internalComboButtonController = useRef<TabStripComboButtonController | null>(null);
	if (internalComboButtonController.current === null) {
		internalComboButtonController.current = new TabStripComboButtonController({
			tabSearchPinned: showVerticalTabSearchButton,
		});
	}
	const comboButtonController =
		externalComboButtonController ?? internalComboButtonController.current;
	const comboButtonSnapshot = useSyncExternalStore(
		comboButtonController.subscribe,
		comboButtonController.getSnapshot,
		comboButtonController.getSnapshot,
	);
	const previousTabSearchOpen = useRef(false);

	const internalExpandOnHoverController = useRef<VerticalTabStripExpandOnHoverController | null>(
		null,
	);
	if (internalExpandOnHoverController.current === null) {
		internalExpandOnHoverController.current = new VerticalTabStripExpandOnHoverController();
	}
	const expandOnHoverController =
		verticalExpandOnHoverController ?? internalExpandOnHoverController.current;
	const expandOnHoverSnapshot = useSyncExternalStore(
		expandOnHoverController.subscribe,
		expandOnHoverController.getSnapshot,
		expandOnHoverController.getSnapshot,
	);

	const expandedOnHover = !horizontal && verticalCollapsed && expandOnHoverSnapshot.expanded;
	const effectiveVerticalCollapsed = verticalCollapsed && !expandedOnHover;

	useEffect(() => () => internalExpandOnHoverController.current?.destroy(), []);
	useEffect(() => () => internalHoverCardController.current?.destroy(), []);
	// React StrictMode replays effects without recreating refs. Reset observation
	// here instead of permanently destroying the reusable runtime instance.
	useEffect(() => () => thumbnailRuntime.updateTarget(null), [thumbnailRuntime]);

	useEffect(() => {
		if (externalComboButtonController) return;
		comboButtonController.setPinned('tab-search', showVerticalTabSearchButton);
	}, [comboButtonController, externalComboButtonController, showVerticalTabSearchButton]);

	useEffect(() => {
		comboButtonController.setOrientation(orientation);
	}, [comboButtonController, orientation]);

	useEffect(() => {
		if (previousTabSearchOpen.current === verticalTabSearchOpen) return;
		previousTabSearchOpen.current = verticalTabSearchOpen;
		if (verticalTabSearchOpen) comboButtonController.onBubbleInitializing();
		else comboButtonController.onBubbleDestroying();
	}, [comboButtonController, verticalTabSearchOpen]);

	useEffect(() => {
		const targetTab =
			hover?.kind === 'tab' ? (snapshot.tabs.find((tab) => tab.id === hover.id) ?? null) : null;
		const targetMatchesVisibleCard =
			targetTab !== null &&
			hoverCardSnapshot.visible &&
			hoverCardSnapshot.targetId === `tab:${targetTab.id}`;
		if (!getHoverCardThumbnailSource || !targetMatchesVisibleCard) {
			thumbnailRuntime.updateTarget(null);
			return;
		}
		thumbnailRuntime.updateTarget({
			id: targetTab.id,
			source: getHoverCardThumbnailSource(targetTab),
			previewsEnabled: showHoverCardPreviews,
			needsThumbnail: targetTab.id !== snapshot.activeTabId,
			isInitialShow: hoverCardSnapshot.cardsSeen <= 1,
			useAnimations: true,
			...(targetTab.discarded === undefined ? {} : { discarded: targetTab.discarded }),
			...(targetTab.crashed === undefined ? {} : { crashed: targetTab.crashed }),
		});
	}, [
		getHoverCardThumbnailSource,
		hover?.id,
		hover?.kind,
		hoverCardSnapshot.cardsSeen,
		hoverCardSnapshot.targetId,
		hoverCardSnapshot.visible,
		showHoverCardPreviews,
		snapshot.activeTabId,
		snapshot.revision,
		snapshot.tabs,
		thumbnailRuntime,
	]);

	useEffect(
		() => () => {
			new Set(closingTimers.current.values()).forEach((timer) => window.clearTimeout(timer));
			closingTimers.current.clear();
		},
		[],
	);

	const collapsedGroupStateKey = snapshot.groups
		.map((group) => `${group.id}:${group.collapsed ? 1 : 0}`)
		.join('|');
	useEffect(() => {
		const groups = model.getSnapshot().groups;
		const liveGroups = new Set(groups.map((group) => group.id));
		groups.forEach((group) => {
			const pending = collapsedGroupTimers.current.get(group.id);
			if (!group.collapsed) {
				if (pending !== undefined) window.clearTimeout(pending);
				collapsedGroupTimers.current.delete(group.id);
				if (!fullyCollapsedGroupIdsRef.current.delete(group.id)) return;
				setFullyCollapsedGroupIds(new Set(fullyCollapsedGroupIdsRef.current));
				return;
			}
			if (fullyCollapsedGroupIdsRef.current.has(group.id) || pending !== undefined) return;
			const timer = window.setTimeout(() => {
				collapsedGroupTimers.current.delete(group.id);
				const stillCollapsed = model
					.getSnapshot()
					.groups.some((candidate) => candidate.id === group.id && candidate.collapsed);
				if (!stillCollapsed) return;
				fullyCollapsedGroupIdsRef.current.add(group.id);
				setFullyCollapsedGroupIds(new Set(fullyCollapsedGroupIdsRef.current));
			}, TAB_BOUNDS_ANIMATION_DURATION_MS);
			collapsedGroupTimers.current.set(group.id, timer);
		});
		[...collapsedGroupTimers.current].forEach(([groupId, timer]) => {
			if (liveGroups.has(groupId)) return;
			window.clearTimeout(timer);
			collapsedGroupTimers.current.delete(groupId);
			if (fullyCollapsedGroupIdsRef.current.delete(groupId)) {
				setFullyCollapsedGroupIds(new Set(fullyCollapsedGroupIdsRef.current));
			}
		});
		// The serialized key intentionally limits this effect to collapse/removal
		// transitions; unrelated model revisions must not restart the 200ms timer.
	}, [collapsedGroupStateKey, model]);

	useEffect(
		() => () => {
			collapsedGroupTimers.current.forEach((timer) => window.clearTimeout(timer));
			collapsedGroupTimers.current.clear();
		},
		[],
	);

	const requestCloseTabs = (tabIds: readonly string[], source: TabCloseSource = 'non-ui') => {
		const beforeClose = model.getSnapshot();
		const existingIds = new Set(beforeClose.tabs.map((tab) => tab.id));
		const ids = [...new Set(tabIds)].filter(
			(id) => existingIds.has(id) && !closingTimers.current.has(id),
		);
		if (ids.length === 0) return;
		const closingTitles = beforeClose.tabs
			.filter((tab) => ids.includes(tab.id))
			.map((tab) => tab.title || 'New Tab');
		if (horizontal && source !== 'non-ui' && ids.length === 1 && horizontalLayout) {
			const removedId = requireDefined(ids[0]);
			const removedModelIndex = beforeClose.tabs.findIndex((tab) => tab.id === removedId);
			const removedTab = beforeClose.tabs[removedModelIndex];
			const removedVisualIndex = items.findIndex(
				(item) => item.type === 'tab' && item.tab.id === removedId,
			);
			const removedBounds = horizontalLayout.bounds[removedVisualIndex];
			const remainingTabs = beforeClose.tabs.filter((tab) => tab.id !== removedId);
			const nextActive = remainingTabs[Math.min(removedModelIndex, remainingTabs.length - 1)];
			const nextVisualIndex = nextActive
				? items.findIndex((item) => item.type === 'tab' && item.tab.id === nextActive.id)
				: -1;
			const nextActiveBounds = horizontalLayout.bounds[nextVisualIndex];
			const trailing = horizontalLayout.bounds.at(-1);
			if (removedTab && removedBounds && trailing) {
				const width = calculateTabClosingModeWidth({
					currentOverride: tabClosingMode?.width ?? null,
					containerWidth: layoutWidth,
					modelCountAfterRemoval: remainingTabs.length,
					removedModelIndex,
					removedWidth: removedBounds.width,
					removedWasActive: beforeClose.activeTabId === removedId,
					removedWasPinned: removedTab.pinned,
					layoutDomain: horizontalLayout.domain,
					trailingIdealRight: trailing.x + trailing.width,
					tabOverlap: DEFAULT_TAB_STYLE_METRICS.tabOverlap,
					...(nextActiveBounds === undefined ? {} : { nextActiveWidth: nextActiveBounds.width }),
					...(nextActive === undefined ? {} : { nextActivePinned: nextActive.pinned }),
				});
				setTabClosingMode(width === null ? null : { width, source });
			}
		}
		setClosingTabIds((current) => new Set([...current, ...ids]));
		const timer = window.setTimeout(() => {
			const ready = ids.filter((id) => closingTimers.current.get(id) === timer);
			ready.forEach((id) => closingTimers.current.delete(id));
			model.closeTabs(ready);
			if (ready.length === 1) announce(`Closed ${closingTitles[0]}`);
			else if (ready.length > 1) announce(`Closed ${ready.length} tabs`);
			setClosingTabIds((current) => {
				const next = new Set(current);
				ready.forEach((id) => next.delete(id));
				return next;
			});
		}, TAB_BOUNDS_ANIMATION_DURATION_MS);
		ids.forEach((id) => closingTimers.current.set(id, timer));
	};

	useEffect(() => {
		if (tabClosingMode?.source !== 'touch') return;
		let timer = 0;
		const finishWhenIdle = () => {
			timer = window.setTimeout(() => {
				if (dragRef.current?.started) finishWhenIdle();
				else setTabClosingMode(null);
			}, TAB_CLOSE_TOUCH_RESIZE_DELAY_MS);
		};
		finishWhenIdle();
		return () => window.clearTimeout(timer);
	}, [tabClosingMode?.source, tabClosingMode?.width]);

	useEffect(() => {
		if (tabClosingMode?.source !== 'mouse') return;
		const handlePointerMove = (event: PointerEvent) => {
			const element = stripRef.current;
			if (!element) return;
			const rect = element.getBoundingClientRect();
			const watchBounds = getTabClosingModeWatchBounds(
				{
					x: rect.left,
					y: rect.top,
					width: rect.width,
					height: rect.height,
				},
				getComputedStyle(element).direction === 'rtl',
			);
			if (!tabClosingModeContainsPoint(watchBounds, { x: event.clientX, y: event.clientY })) {
				setTabClosingMode(null);
			}
		};
		window.addEventListener('pointermove', handlePointerMove);
		return () => window.removeEventListener('pointermove', handlePointerMove);
	}, [stripRef, tabClosingMode?.source]);

	useEffect(() => {
		expandOnHoverController.setEnvironment({
			shouldDisplayVerticalTabs: !horizontal,
			collapsed: verticalCollapsed,
			enabled: verticalExpandOnHover,
			frameActive: document.visibilityState !== 'hidden',
		});
	}, [expandOnHoverController, horizontal, verticalCollapsed, verticalExpandOnHover]);

	useEffect(() => {
		onVerticalExpandedOnHoverChange?.(expandedOnHover);
	}, [expandedOnHover, onVerticalExpandedOnHoverChange]);

	useEffect(() => {
		if (!menu || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepCurrentState);
		return () => lock.dispose();
	}, [expandOnHoverController, horizontal, menu, verticalCollapsed]);

	useEffect(() => {
		if (!comboButtonSnapshot.keepCurrentStateLock || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepCurrentState);
		return () => lock.dispose();
	}, [
		comboButtonSnapshot.keepCurrentStateLock,
		expandOnHoverController,
		horizontal,
		verticalCollapsed,
	]);

	useEffect(() => {
		if (!drag?.started || horizontal || !verticalCollapsed) return;
		const lock = expandOnHoverController.getLock(ExpandOnHoverLockType.KeepExpanded);
		return () => lock.dispose();
	}, [drag?.started, expandOnHoverController, horizontal, verticalCollapsed]);

	useEffect(() => {
		if (!menu && !drag?.started) return;
		const lock = hoverCardController.getHideLock();
		setHover(null);
		return () => lock.dispose();
	}, [drag?.started, hoverCardController, menu]);

	const layoutWidth = Math.max(0, size.width - (showNewTabButton ? 42 : 0));
	const effectiveLayoutWidth = Math.min(layoutWidth, tabClosingMode?.width ?? layoutWidth);
	const expandedHeaderDragGroupId =
		horizontal && drag?.started && drag.groupHeaderId !== null && !drag.groupWasCollapsedBeforeDrag
			? drag.groupHeaderId
			: null;
	const horizontalLayout = useMemo(() => {
		if (!horizontal) return null;
		const constraints = items.map((item) => {
			const group =
				item.type === 'tab' && item.tab.group !== null
					? snapshot.groups.find((candidate) => candidate.id === item.tab.group)
					: null;
			const hiddenByGroupFocus =
				groupFocusActive &&
				(item.type === 'group'
					? item.group.id !== focusedGroup.id
					: !tabVisibleWhileFocused(item.tab));
			const hiddenByCollapsedGroup =
				!groupFocusActive &&
				item.type === 'tab' &&
				Boolean(group?.collapsed) &&
				group?.id !== expandedHeaderDragGroupId;
			const state = new TabLayoutState(
				hiddenByGroupFocus ||
					hiddenByCollapsedGroup ||
					(item.type === 'tab' && closingTabIds.has(item.tab.id))
					? TabOpen.Closed
					: TabOpen.Open,
				item.type === 'tab' && item.tab.pinned ? TabPinned.Pinned : TabPinned.Unpinned,
				item.type === 'tab' && item.tab.id === snapshot.activeTabId
					? TabActive.Active
					: TabActive.Inactive,
			);
			const sizeItem =
				item.type === 'group' && item.group.id === expandedHeaderDragGroupId
					? { ...item, group: { ...item.group, collapsed: false } }
					: item;
			return new TabWidthConstraints(state, sizeInfoForItem(sizeItem));
		});
		return calculateTabBounds(constraints, effectiveLayoutWidth);
	}, [
		closingTabIds,
		effectiveLayoutWidth,
		expandedHeaderDragGroupId,
		focusedGroup?.id,
		groupFocusActive,
		horizontal,
		items,
		snapshot.groups,
		snapshot.activeTabId,
		tabVisibleWhileFocused,
	]);

	useEffect(() => {
		const runtime = horizontalAnimationRuntimeRef.current;
		if (!runtime || !horizontal || !horizontalLayout) {
			setAnimatedHorizontalFrame(null);
			return;
		}
		if (horizontalAnimationRequestRef.current !== null) {
			cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
			horizontalAnimationRequestRef.current = null;
		}
		const target = {
			// Tab-closing mode constrains child target bounds without resizing the
			// collection host. Passing effectiveLayoutWidth here would trigger
			// Chromium's real host-resize snap branch and skip close animation.
			hostSize: { width: layoutWidth, height: LAYOUT_CONSTANTS.tabStripHeight },
			children: items.map((item, index) => {
				const bounds = horizontalLayout.bounds[index] ?? {
					x: 0,
					y: 0,
					width: 0,
					height: LAYOUT_CONSTANTS.tabStripHeight,
				};
				return {
					id: item.id,
					visible: bounds.width > 0,
					bounds: { ...bounds },
				};
			}),
		};
		const synced = runtime.sync(
			target,
			items.map((item) => ({
				id: item.id,
				group: item.type === 'group' ? item.group.id : item.tab.group,
				dragging:
					item.type === 'group'
						? Boolean(
								drag?.started &&
								drag.visualViews.some((view) => view.id === `group:${item.group.id}`),
							)
						: Boolean(drag?.started && drag.tabIds.includes(item.tab.id)),
			})),
		);
		setAnimatedHorizontalFrame(synced.frame);
		// A render with an unchanged target can still arrive while a previous
		// timeline is active (selection/collapse and drag-settle updates commonly
		// do this together). We canceled that timeline's RAF above, so it must be
		// resumed from its current progress instead of leaving a permanent
		// half-interpolated frame.
		if (!runtime.isAnimating) return;
		if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			setAnimatedHorizontalFrame(runtime.complete());
			return;
		}

		const startedAt = performance.now();
		const startProgress = synced.startProgress;
		const tick = (now: number) => {
			const elapsed = Math.max(0, now - startedAt);
			const timeline = Math.min(1, elapsed / TAB_BOUNDS_ANIMATION_DURATION_MS);
			const progress = startProgress + (1 - startProgress) * tabCollectionEaseInOut(timeline);
			setAnimatedHorizontalFrame(runtime.setProgress(progress));
			if (timeline < 1) {
				horizontalAnimationRequestRef.current = requestManagedAnimationFrame(tick);
			} else {
				horizontalAnimationRequestRef.current = null;
				setAnimatedHorizontalFrame(runtime.complete());
			}
		};
		horizontalAnimationRequestRef.current = requestManagedAnimationFrame(tick);
	}, [
		drag?.started,
		drag?.tabIds,
		drag?.visualViews,
		horizontal,
		horizontalLayout,
		items,
		layoutWidth,
	]);

	useEffect(
		() => () => {
			if (horizontalAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
				horizontalAnimationRequestRef.current = null;
			}
			horizontalAnimationRuntimeRef.current?.dispose();
		},
		[],
	);

	useEffect(() => {
		const previousCount = previousTabCountRef.current;
		previousTabCountRef.current = snapshot.tabs.length;
		if (snapshot.tabs.length > previousCount) setTabClosingMode(null);
	}, [snapshot.tabs.length]);

	useEffect(() => {
		if (!tabClosingMode || closingTabIds.size > 0) return;
		const trailing = horizontalLayout?.bounds.at(-1);
		if (trailing && shouldExitTabClosingMode(tabClosingMode.width, trailing.x + trailing.width)) {
			setTabClosingMode(null);
		}
	}, [closingTabIds, horizontalLayout, tabClosingMode]);

	const clearHover = useCallback(
		(updateType: HoverCardUpdateType = HoverCardUpdateType.Hover) => {
			if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
			hoverTimer.current = null;
			hoverCardController.updateHoverCard(null, updateType);
			setHover(null);
		},
		[hoverCardController],
	);

	useEffect(() => {
		if (!horizontalAnimationRuntimeRef.current?.isAnimating) return;
		// TabContainerImpl suppresses hover cards while any tab-slot bounds are
		// animating so cards never remain anchored to a moving view.
		clearHover(HoverCardUpdateType.Animating);
	}, [animatedHorizontalFrame, clearHover]);

	useEffect(() => () => clearHover(HoverCardUpdateType.Event), [clearHover]);

	const updateHoverForElement = (
		element: HTMLElement,
		id: string,
		updateType: HoverCardUpdateType,
		kind: HoverState['kind'] = 'tab',
	) => {
		const targetTab = kind === 'tab' ? (snapshot.tabs.find((tab) => tab.id === id) ?? null) : null;
		const pinnedBelowAnchor =
			!horizontal && targetTab?.pinned === true && !effectiveVerticalCollapsed;
		const rawRect = element.getBoundingClientRect();
		const slotId = kind === 'group' ? `group-${id}` : id;
		const slotBounds = horizontalAnimationRuntimeRef.current
			?.getSlot(slotId)
			?.getAnchorBoundsInScreen({
				x: rawRect.left,
				y: rawRect.top,
				width: rawRect.width,
				height: rawRect.height,
			});
		const rect = slotBounds
			? {
					left: slotBounds.x,
					top: slotBounds.y,
					right: slotBounds.x + slotBounds.width,
					bottom: slotBounds.y + slotBounds.height,
					width: slotBounds.width,
					height: slotBounds.height,
				}
			: rawRect;
		const targetWidth = horizontal
			? Math.max(
					rect.width,
					...[
						...(element.closest('[data-ct-root]')?.querySelectorAll<HTMLElement>('[data-tab-id]') ??
							[]),
					].map((tab) => tab.getBoundingClientRect().width),
				)
			: (element.closest<HTMLElement>('[data-ct-root]')?.getBoundingClientRect().width ??
				rect.width);
		setHover({
			kind,
			id,
			x:
				horizontal || pinnedBelowAnchor
					? Math.max(8, Math.min(rect.left, window.innerWidth - 340))
					: rect.right + 8,
			y: horizontal || pinnedBelowAnchor ? rect.bottom + 6 : Math.max(8, rect.top),
		});
		hoverCardController.updateHoverCard(
			{
				id: `${kind}:${id}`,
				width: targetWidth,
				minimumWidth: horizontal
					? TAB_SIZE.pinnedTabWidth
					: LAYOUT_CONSTANTS.verticalTabStripCollapsedWidth,
				standardWidth: horizontal ? TAB_SIZE.standardWidth : 240,
				focused: updateType === HoverCardUpdateType.Focus,
			},
			updateType,
		);
	};

	const onTabMouseEnter = (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => {
		updateHoverForElement(event.currentTarget, tabId, HoverCardUpdateType.Hover);
	};

	const onGroupMouseEnter = (event: ReactMouseEvent<HTMLDivElement>, groupId: string) => {
		if (!showGroupHoverCards) return;
		updateHoverForElement(event.currentTarget, groupId, HoverCardUpdateType.Hover, 'group');
	};

	useEffect(() => {
		if (!hoverCardSnapshot.visible) return;
		const dismissForPointer = () => {
			hoverCardController.updateHoverCard(null, HoverCardUpdateType.Event);
			setHover(null);
		};
		const dismissForKey = (event: KeyboardEvent) => {
			const focusInStrip = event.target instanceof Node && stripRef.current?.contains(event.target);
			if (focusInStrip && event.key !== 'Enter' && event.key !== 'Escape') return;
			dismissForPointer();
		};
		window.addEventListener('pointerdown', dismissForPointer, true);
		window.addEventListener('keydown', dismissForKey, true);
		return () => {
			window.removeEventListener('pointerdown', dismissForPointer, true);
			window.removeEventListener('keydown', dismissForKey, true);
		};
	}, [hoverCardController, hoverCardSnapshot.visible, stripRef]);

	const selectFromPointer = (event: ReactPointerEvent, tab: Readonly<TabData>) => {
		const modifier = event.metaKey || event.ctrlKey;
		if (event.shiftKey && modifier) model.addSelectionFromAnchorTo(tab.id);
		else if (event.shiftKey) model.extendSelectionTo(tab.id);
		else if (modifier) model.toggleSelected(tab.id);
		else if (!model.getSnapshot().selectedTabIds.has(tab.id)) model.selectTab(tab.id);
	};

	const onTabPointerDown = (event: ReactPointerEvent<HTMLDivElement>, tab: Readonly<TabData>) => {
		if (event.button === 1) {
			middlePressRef.current = { tabId: tab.id, pointerId: event.pointerId };
			event.preventDefault();
			return;
		}
		if (event.button !== 0) return;
		clearHover();
		const touchPointer = event.pointerType === 'touch';
		// Tab::OnGestureEvent handles kGestureTapDown exactly like the unmodified
		// selection portion of OnMousePressed: selection happens before the drag
		// controller is asked to start.
		const restoreState = model.captureDragRestoreState();
		selectFromPointer(event, tab);
		const pointerSnapshot = model.getSnapshot();
		if (
			(event.metaKey || event.ctrlKey) &&
			!event.shiftKey &&
			!pointerSnapshot.selectedTabIds.has(tab.id)
		) {
			return;
		}
		const initiallySelected = pointerSnapshot.selectedTabIds.has(tab.id)
			? pointerSnapshot.tabs
					.filter(
						(candidate) =>
							pointerSnapshot.selectedTabIds.has(candidate.id) && candidate.pinned === tab.pinned,
					)
					.map((candidate) => candidate.id)
			: [tab.id];
		const selected = initiallySelected;
		const draggedTabIds = selected.length ? selected : [tab.id];
		const draggedIdSet = new Set(draggedTabIds);
		const fullySelectedGroupIds = pointerSnapshot.groups
			.filter((group) => {
				const members = pointerSnapshot.tabs.filter((candidate) => candidate.group === group.id);
				return members.length > 0 && members.every((candidate) => draggedIdSet.has(candidate.id));
			})
			.map((group) => group.id);
		const visuals = captureDragVisuals({
			sourceElement: event.currentTarget,
			sourceId: tab.id,
			tabIds: draggedTabIds,
			sourceGroupId: null,
			groupIds: fullySelectedGroupIds,
			underlineGroupIds: [
				...new Set(
					pointerSnapshot.tabs
						.filter((candidate) => draggedIdSet.has(candidate.id) && candidate.group !== null)
						.map((candidate) => requireDefined(candidate.group)),
				),
			],
			orientation,
			pointer: { x: event.clientX, y: event.clientY },
		});
		const next: DragState = {
			sourceId: tab.id,
			tabIds: draggedTabIds,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			touchDragReady: !touchPointer,
			startX: event.clientX,
			startY: event.clientY,
			shiftPressedOnPointerDown: event.shiftKey,
			x: event.clientX,
			y: event.clientY,
			started: false,
			targetIndex: null,
			targetGroup: null,
			targetIdealLeft: null,
			lastReorderCoordinate: horizontal ? event.clientX : event.clientY,
			restoreState,
			groupHeaderId: null,
			groupWasCollapsedBeforeDrag: false,
			horizontalGroupDrag: null,
			label: tab.title,
			...visuals,
			crossWindowRuntime:
				createCrossWindowDragRuntime?.(
					buildDragSessionData({
						snapshot: pointerSnapshot,
						tabIds: draggedTabIds,
						sourceId: tab.id,
						groupHeaderId: null,
						groupWasCollapsedBeforeDrag: false,
						pointerOffsetRatios: visuals.pointerOffsetRatios,
					}),
				) ?? null,
			settling: false,
		};
		setDrag(next);
		if (touchPointer) {
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = window.setTimeout(() => {
				touchDragTimer.current = null;
				const current = dragRef.current;
				if (!current || current.pointerId !== event.pointerId || current.pointerType !== 'touch')
					return;
				const liveSnapshot = model.getSnapshot();
				if (!liveSnapshot.selectedTabIds.has(current.sourceId)) model.selectTab(current.sourceId);
				setDrag((candidate) =>
					candidate?.pointerId === event.pointerId
						? { ...candidate, touchDragReady: true }
						: candidate,
				);
			}, 500);
		} else {
			event.currentTarget.setPointerCapture?.(event.pointerId);
		}
	};

	useEffect(() => {
		const finishMiddlePress = (event: PointerEvent) => {
			const press = middlePressRef.current;
			if (!press || event.pointerId !== press.pointerId || event.button !== 1) return;
			middlePressRef.current = null;
			const hit =
				document
					.elementFromPoint?.(event.clientX, event.clientY)
					?.closest<HTMLElement>('[data-tab-id]') ?? null;
			if (hit?.dataset['tabId'] === press.tabId) {
				requestCloseTabs([press.tabId], 'mouse');
				return;
			}
			// Port of Tab::OnMouseReleased's rapid-close fallback: if the pressed
			// view moved away because it is already closing, close the tab that has
			// animated under the pointer instead.
			if (closingTimers.current.has(press.tabId) && hit?.dataset['tabId']) {
				requestCloseTabs([hit.dataset['tabId']], 'mouse');
			}
		};
		const cancelMiddlePress = (event: PointerEvent) => {
			if (middlePressRef.current?.pointerId === event.pointerId) middlePressRef.current = null;
		};
		window.addEventListener('pointerup', finishMiddlePress);
		window.addEventListener('pointercancel', cancelMiddlePress);
		return () => {
			window.removeEventListener('pointerup', finishMiddlePress);
			window.removeEventListener('pointercancel', cancelMiddlePress);
		};
	});

	const onGroupPointerDown = (
		event: ReactPointerEvent<HTMLDivElement>,
		group: Readonly<TabGroupData>,
	) => {
		if (event.button !== 0) return;
		if (menu?.kind === 'group') return;
		clearHover();
		const touchPointer = event.pointerType === 'touch';
		const pointerSnapshot = model.getSnapshot();
		const groupTabs = pointerSnapshot.tabs.filter((tab) => tab.group === group.id);
		if (groupTabs.length === 0) return;
		const restoreState = model.captureDragRestoreState();
		const active =
			groupTabs.find((tab) => tab.id === pointerSnapshot.activeTabId) ??
			requireDefined(groupTabs[0]);
		const draggedTabIds = groupTabs.map((tab) => tab.id);
		const visuals = captureDragVisuals({
			sourceElement: event.currentTarget,
			sourceId: active.id,
			tabIds: draggedTabIds,
			sourceGroupId: group.id,
			groupIds: [group.id],
			underlineGroupIds: [group.id],
			orientation,
			pointer: { x: event.clientX, y: event.clientY },
		});
		const next: DragState = {
			sourceId: active.id,
			tabIds: draggedTabIds,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			touchDragReady: !touchPointer,
			startX: event.clientX,
			startY: event.clientY,
			shiftPressedOnPointerDown: event.shiftKey,
			x: event.clientX,
			y: event.clientY,
			started: false,
			targetIndex: null,
			targetGroup: group.id,
			targetIdealLeft: null,
			lastReorderCoordinate: horizontal ? event.clientX : event.clientY,
			restoreState,
			groupHeaderId: group.id,
			groupWasCollapsedBeforeDrag: group.collapsed,
			horizontalGroupDrag: horizontal
				? createArmedHorizontalGroupDrag(pointerSnapshot, group.id)
				: null,
			label: group.title || 'Unnamed group',
			...visuals,
			crossWindowRuntime:
				createCrossWindowDragRuntime?.(
					buildDragSessionData({
						snapshot: pointerSnapshot,
						tabIds: draggedTabIds,
						sourceId: active.id,
						groupHeaderId: group.id,
						groupWasCollapsedBeforeDrag: group.collapsed,
						pointerOffsetRatios: visuals.pointerOffsetRatios,
					}),
				) ?? null,
			settling: false,
		};
		setDrag(next);
		if (touchPointer) {
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = window.setTimeout(() => {
				touchDragTimer.current = null;
				setDrag((candidate) =>
					candidate?.pointerId === event.pointerId
						? { ...candidate, touchDragReady: true }
						: candidate,
				);
			}, 500);
		} else {
			event.currentTarget.setPointerCapture?.(event.pointerId);
		}
	};

	const toggleGroupFromHeader = (
		groupId: string,
		source: 'mouse' | 'touch' | 'keyboard' = 'mouse',
	) => {
		if (suppressGroupToggleRef.current === groupId) {
			suppressGroupToggleRef.current = null;
			return;
		}
		if (menu?.kind === 'group') {
			setMenu(null);
			return;
		}
		const group = snapshot.groups.find((candidate) => candidate.id === groupId);
		if (horizontal && group && !group.collapsed && source !== 'keyboard' && horizontalLayout) {
			const headerIndex = items.findIndex(
				(item) => item.type === 'group' && item.group.id === groupId,
			);
			const groupTabIndices = items
				.map((item, index) => (item.type === 'tab' && item.tab.group === groupId ? index : -1))
				.filter((index) => index !== -1);
			const headerBounds = horizontalLayout.bounds[headerIndex];
			const trailingGroupBounds = horizontalLayout.bounds[groupTabIndices.at(-1) ?? -1];
			const trailingBounds = horizontalLayout.bounds.at(-1);
			if (headerBounds && trailingGroupBounds && trailingBounds) {
				const groupWidth = trailingGroupBounds.x + trailingGroupBounds.width - headerBounds.x;
				const collapsedHeaderWidth = groupHeaderWidth(
					{ ...group, collapsed: true },
					groupTabIndices.length,
				);
				setTabClosingMode({
					width: trailingBounds.x + trailingBounds.width - groupWidth + collapsedHeaderWidth,
					source,
				});
			}
		} else if (group?.collapsed) {
			setTabClosingMode(null);
		}
		model.toggleGroupCollapsed(groupId);
	};

	const onGroupKeyDown = (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => {
		if (!(event.ctrlKey || event.metaKey)) return;
		const previousKey = horizontal ? 'ArrowLeft' : 'ArrowUp';
		const nextKey = horizontal ? 'ArrowRight' : 'ArrowDown';
		let direction: -1 | 0 | 1 = 0;
		if (event.key === previousKey) direction = -1;
		else if (event.key === nextKey) direction = 1;
		if (direction === 0) return;
		event.preventDefault();
		model.shiftGroupRelative(groupId, direction);
		let relativeDirection = direction < 0 ? 'up' : 'down';
		if (horizontal) relativeDirection = direction < 0 ? 'left' : 'right';
		announce(`Group moved ${relativeDirection}`);
	};

	const dragPresent = drag !== null;
	useEffect(() => {
		if (!dragPresent) return;
		const move = (event: PointerEvent) => {
			const dragAtStart = dragRef.current;
			if (!dragAtStart || dragAtStart.settling || dragAtStart.pointerId !== event.pointerId) return;
			let current: DragState = dragAtStart;
			const distance = Math.hypot(event.clientX - current.startX, event.clientY - current.startY);
			if (current.pointerType === 'touch' && !current.touchDragReady) {
				if (distance > 10) {
					if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
					touchDragTimer.current = null;
					setDrag(null);
				}
				return;
			}
			const started = current.started || distance > 10;
			if (started && !current.started) current.crossWindowRuntime?.start();
			if (started && !current.started && current.groupHeaderId !== null) {
				suppressGroupToggleRef.current = current.groupHeaderId;
				if (horizontal && current.horizontalGroupDrag !== null) {
					const activation = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
						type: 'activate-drag',
					});
					applyHorizontalGroupDragCommands(model, current.restoreState, activation.commands);
					assertHorizontalGroupDragRefinement(model, activation.state);
					current = { ...current, horizontalGroupDrag: activation.state };
				} else {
					// TabDragController::AttachImpl resets selection to every dragged tab.
					// For a header drag outside the active group, Init records tab zero as
					// active_tab_index_within_group. Do not expand a previously collapsed
					// group while applying that selection.
					model.setSelection(current.tabIds, current.sourceId, { expandGroups: false });
					if (!current.groupWasCollapsedBeforeDrag) {
						model.updateGroup(current.groupHeaderId, { collapsed: true });
					}
				}
			}
			let targetIndex: number | null = null;
			let targetGroup: TabData['group'] = null;
			let targetIdealLeft = current.targetIdealLeft;
			let calculatedHorizontalGroup = false;
			let horizontalTargetGroup: TabData['group'] = null;
			let lastReorderCoordinate = current.lastReorderCoordinate;
			if (started) {
				current.crossWindowRuntime?.update({ x: event.screenX, y: event.screenY });
				const externalTargetActive =
					current.crossWindowRuntime?.getSnapshot?.().targetActive === true;
				const liveSnapshot = model.getSnapshot();
				const source =
					liveSnapshot.tabs.find((candidate) => candidate.id === current.sourceId) ?? null;
				const root = stripRef.current;
				targetIndex = source?.pinned
					? liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned)
					: liveSnapshot.tabs.length;
				if (targetIndex === -1) targetIndex = liveSnapshot.tabs.length;
				let targetExtent = horizontal ? TAB_SIZE.standardWidth : 30;
				let verticalNestedTargetGroup: TabData['group'] = null;
				const pinnedDragAxes = getPinnedTabContainerDragAxes(
					verticalCollapsed
						? VerticalTabStripCollapseState.Collapsed
						: VerticalTabStripCollapseState.Expanded,
				);
				const pinnedGridDrag = !horizontal && source?.pinned === true && pinnedDragAxes === 'both';
				if (!horizontal && root && source) {
					const rootRect = root.getBoundingClientRect();
					const tabElements = [...root.querySelectorAll<HTMLElement>('[data-tab-id]')];
					const containsGroupVisual = current.visualViews.some((view) => view.kind === 'group');
					const children: Array<{
						id: string;
						visible: boolean;
						bounds: { x: number; y: number; width: number; height: number };
						dragging: boolean;
					}> = [];
					const representedGroups = new Set<string>();
					for (let index = 0; index < liveSnapshot.tabs.length; index += 1) {
						const tab = requireDefined(liveSnapshot.tabs[index]);
						if (tab.pinned !== source.pinned) continue;
						if (containsGroupVisual && tab.group !== null) {
							if (representedGroups.has(tab.group)) continue;
							representedGroups.add(tab.group);
							const element = [
								...root.querySelectorAll<HTMLElement>('[data-tab-strip-group][data-group-id]'),
							].find((candidate) => candidate.dataset['groupId'] === tab.group);
							if (!element) continue;
							const rect = element.getBoundingClientRect();
							children.push({
								// A group is one direct collection child. Its first tab id is
								// also Chromium's insertion index for the node.
								id: tab.id,
								visible: true,
								bounds: {
									x: rect.left - rootRect.left,
									y: rect.top - rootRect.top,
									width: rect.width,
									height: rect.height,
								},
								dragging: current.visualViews.some((view) => view.id === `group:${tab.group}`),
							});
							continue;
						}
						const element = tabElements.find((candidate) => candidate.dataset['tabId'] === tab.id);
						if (!element) continue;
						const rect = element.getBoundingClientRect();
						children.push({
							id: tab.id,
							visible: true,
							bounds: {
								x: rect.left - rootRect.left,
								y: rect.top - rootRect.top,
								width: rect.width,
								height: rect.height,
							},
							dragging: current.tabIds.includes(tab.id),
						});
					}
					const sourceVisualId = getSourceVisualId(current);
					const verticalVisualViews = getVerticalDragVisualViews(current);
					const draggedLayout = buildDraggedTabsLayout({
						layout: source.pinned || verticalCollapsed ? 'squash' : 'vertical',
						pointerOffsetRatios: current.pointerOffsetRatios,
						sourceSlotSize: current.sourceSlotSize,
						sourceViewOffsetFromSlot: current.sourceViewOffsetFromSlot,
						views: verticalVisualViews.map((view) => ({
							id: view.id,
							bounds: { x: view.x, y: view.y, width: view.width, height: view.height },
							source: view.id === sourceVisualId,
						})),
					});
					const draggedBoundsInScreen = getDraggingViewsBoundsAtPoint(
						draggedLayout.boundsRelativeToPointer,
						{ x: event.clientX, y: event.clientY },
					);
					if (!source.pinned && current.groupHeaderId === null) {
						const groupElements = [
							...root.querySelectorAll<HTMLElement>('[data-tab-strip-group][data-group-id]'),
						];
						const currentGroupElement =
							current.targetGroup === null
								? null
								: (groupElements.find(
										(element) =>
											element.dataset['groupId'] === current.targetGroup ||
											element.querySelector<HTMLElement>('[data-group-id]')?.dataset['groupId'] ===
												current.targetGroup,
									) ?? null);
						if (currentGroupElement) {
							const groupData = liveSnapshot.groups.find(
								(group) => group.id === current.targetGroup,
							);
							const groupRect = currentGroupElement.getBoundingClientRect();
							const headerHeight =
								currentGroupElement
									.querySelector<HTMLElement>('[data-group-id]')
									?.getBoundingClientRect().height ?? 26;
							if (
								!groupData?.collapsed &&
								shouldRemainInVerticalTabGroup(
									draggedBoundsInScreen,
									{
										x: groupRect.left,
										y: groupRect.top,
										width: groupRect.width,
										height: groupRect.height,
									},
									headerHeight,
								)
							) {
								verticalNestedTargetGroup = current.targetGroup;
							}
						}
						if (verticalNestedTargetGroup === null) {
							for (const groupElement of groupElements) {
								const groupId =
									groupElement.dataset['groupId'] ??
									groupElement.querySelector<HTMLElement>('[data-group-id]')?.dataset['groupId'];
								const groupData = liveSnapshot.groups.find((group) => group.id === groupId);
								if (!groupId || groupData?.collapsed) continue;
								const groupRect = groupElement.getBoundingClientRect();
								const headerHeight =
									groupElement
										.querySelector<HTMLElement>('[data-group-id]')
										?.getBoundingClientRect().height ?? 26;
								if (
									!shouldEnterVerticalTabGroup(
										draggedBoundsInScreen,
										{
											x: groupRect.left,
											y: groupRect.top,
											width: groupRect.width,
											height: groupRect.height,
										},
										headerHeight,
									)
								)
									continue;
								verticalNestedTargetGroup = groupId;
								break;
							}
						}
					}
					const targetId = getTargetBeforeDraggedTabs({
						children,
						draggedBounds: {
							...draggedBoundsInScreen,
							x: draggedBoundsInScreen.x - rootRect.left,
							y: draggedBoundsInScreen.y - rootRect.top,
						},
						axes: source.pinned ? pinnedDragAxes : 'vertical-only',
						rtl: getComputedStyle(root).direction === 'rtl',
						hostWidth: rootRect.width,
					});
					if (targetId !== null) {
						targetIndex = liveSnapshot.tabs.findIndex((tab) => tab.id === targetId);
						const targetChild = children.find((child) => child.id === targetId);
						targetExtent = pinnedGridDrag
							? (targetChild?.bounds.width ?? 30)
							: (targetChild?.bounds.height ?? 30);
					}
				} else if (root && source && horizontalLayout) {
					const rootRect = root.getBoundingClientRect();
					const idealTabBounds = liveSnapshot.tabs.map((tab) => {
						const visualIndex = items.findIndex(
							(item) => item.type === 'tab' && item.tab.id === tab.id,
						);
						const ideal = horizontalLayout.bounds[visualIndex];
						const element = [...root.querySelectorAll<HTMLElement>('[data-tab-id]')].find(
							(candidate) => candidate.dataset['tabId'] === tab.id,
						);
						const rect = element?.getBoundingClientRect();
						if (rootRect.width > 0 && ideal) return ideal;
						return {
							x: (rect?.left ?? rootRect.left) - rootRect.left,
							y: 0,
							width: rect?.width ?? TAB_SIZE.standardWidth,
							height: LAYOUT_CONSTANTS.tabStripHeight,
						};
					});
					const groupHeaderWidths = new Map(
						liveSnapshot.groups.map((group) => {
							const visualIndex = items.findIndex(
								(item) => item.type === 'group' && item.group.id === group.id,
							);
							const element = [...root.querySelectorAll<HTMLElement>('[data-group-id]')].find(
								(candidate) => candidate.dataset['groupId'] === group.id,
							);
							const measuredWidth = element?.getBoundingClientRect().width ?? 0;
							return [
								group.id,
								rootRect.width > 0
									? (horizontalLayout.bounds[visualIndex]?.width ?? measuredWidth)
									: measuredWidth,
							] as const;
						}),
					);
					const sourceVisualId = getSourceVisualId(current);
					const compactGroupHeaderDrag =
						current.groupHeaderId !== null && current.groupWasCollapsedBeforeDrag;
					const targetDragViews = compactGroupHeaderDrag
						? current.visualViews.filter((view) => view.kind === 'group')
						: current.visualViews;
					const sourceWidthOverride =
						compactGroupHeaderDrag && current.groupHeaderId !== null
							? groupHeaderWidths.get(current.groupHeaderId)
							: undefined;
					const packedTargetDrag = packHorizontalDragViews({
						views: targetDragViews,
						sourceVisualId,
						pointerRatioX: current.pointerOffsetRatios.x,
						...(sourceWidthOverride === undefined ? {} : { sourceWidthOverride }),
					});
					const draggedWidth = packedTargetDrag.width;
					const firstDraggedTabIndex = liveSnapshot.tabs.findIndex((tab) =>
						current.tabIds.includes(tab.id),
					);
					const firstUnpinned = liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned);
					const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
					const draggedX = event.clientX - packedTargetDrag.mouseOffset - rootRect.left;
					const dragTarget = getHorizontalDragTarget({
						draggedBounds: {
							x: draggedX,
							y: 0,
							width: draggedWidth,
							height: LAYOUT_CONSTANTS.tabStripHeight,
						},
						tabs: liveSnapshot.tabs,
						idealTabBounds,
						firstDraggedTabIndex: Math.max(0, firstDraggedTabIndex),
						draggedTabCount: current.tabIds.length,
						containsGroupHeader: current.visualViews.some((view) => view.kind === 'group'),
						collapsedGroups: new Set(
							liveSnapshot.groups.filter((group) => group.collapsed).map((group) => group.id),
						),
						groupHeaderWidths,
						containerStartIndex: source.pinned ? 0 : partition,
						containerEndIndex: source.pinned ? partition : liveSnapshot.tabs.length,
					});
					targetIndex = dragTarget.index;
					targetIdealLeft = rootRect.left + dragTarget.idealX;
					targetExtent =
						idealTabBounds[Math.min(targetIndex, idealTabBounds.length - 1)]?.width ??
						TAB_SIZE.standardWidth;
					const draggedIds = new Set(current.tabIds);
					const draggedBeforeTarget = liveSnapshot.tabs
						.slice(0, targetIndex)
						.filter((tab) => draggedIds.has(tab.id)).length;
					const remaining = liveSnapshot.tabs.filter((tab) => !draggedIds.has(tab.id));
					const insertion = Math.max(
						0,
						Math.min(remaining.length, targetIndex - draggedBeforeTarget),
					);
					const leftTab = remaining[insertion - 1];
					const rightTab = remaining[insertion];
					const currentGroup =
						liveSnapshot.tabs.find((tab) => draggedIds.has(tab.id) && !tab.pinned)?.group ?? null;
					const leftTabIndex = leftTab
						? liveSnapshot.tabs.findIndex((tab) => tab.id === leftTab.id)
						: -1;
					const leftBounds = leftTabIndex === -1 ? null : idealTabBounds[leftTabIndex];
					const tabLeftInset = DEFAULT_TAB_STYLE_METRICS.tabOverlap / 2;
					horizontalTargetGroup = calculateGroupForDraggedTabs({
						containsGroupHeader: current.visualViews.some((view) => view.kind === 'group'),
						hasSelectedUnpinnedTab: current.tabIds.some(
							(id) => !liveSnapshot.tabs.find((tab) => tab.id === id)?.pinned,
						),
						leftGroup: leftTab?.group ?? null,
						rightGroup: rightTab?.group ?? null,
						currentGroup,
						leftGroupCollapsed: Boolean(
							liveSnapshot.groups.find((group) => group.id === leftTab?.group)?.collapsed,
						),
						rightGroupCollapsed: Boolean(
							liveSnapshot.groups.find((group) => group.id === rightTab?.group)?.collapsed,
						),
						selectedTabWidth: targetExtent,
						tabLeftInset,
						leftEdge: leftBounds ? leftBounds.x + leftBounds.width - tabLeftInset : tabLeftInset,
						selectedLeft: draggedX + tabLeftInset,
						selectedRight: draggedX + draggedWidth,
						tabDragAreaEnd: rootRect.width || current.dragAreaBounds.width,
					});
					calculatedHorizontalGroup = true;
				}
				const containsGroupHeader = current.visualViews.some((view) => view.kind === 'group');
				if (containsGroupHeader) targetGroup = null;
				else if (calculatedHorizontalGroup) targetGroup = horizontalTargetGroup;
				else if (horizontal || source?.pinned)
					targetGroup = groupForDragInsertion(liveSnapshot, current.tabIds, targetIndex);
				else targetGroup = verticalNestedTargetGroup;

				let coordinate = event.clientY;
				if (pinnedGridDrag) coordinate = event.clientY * 10_000 + event.clientX;
				else if (horizontal) coordinate = event.clientX;
				const threshold =
					horizontal || pinnedGridDrag
						? Math.max(2, calculateHorizontalMoveThreshold(targetExtent, TAB_SIZE.standardWidth))
						: 8;
				const initialMoveNeedsReorder =
					!current.started &&
					!areDraggedTabIdsConsecutive(current.restoreState.tabs, current.tabIds);
				if (
					!externalTargetActive &&
					(Math.abs(coordinate - current.lastReorderCoordinate) > threshold ||
						initialMoveNeedsReorder)
				) {
					if (source) {
						const firstUnpinned = liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned);
						const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
						const beforeIndex = model.getIndexOfTab(current.sourceId);
						if (current.groupHeaderId !== null) {
							if (horizontal && current.horizontalGroupDrag !== null) {
								const movement = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
									type: 'move-to',
									preRemovalTargetIndex: targetIndex,
								});
								applyHorizontalGroupDragCommands(model, current.restoreState, movement.commands);
								assertHorizontalGroupDragRefinement(model, movement.state);
								current = { ...current, horizontalGroupDrag: movement.state };
							} else {
								model.moveGroupByDrag(current.groupHeaderId, targetIndex);
							}
						} else {
							model.moveTabsByDrag(
								current.tabIds,
								targetIndex,
								source.pinned ? 0 : partition,
								source.pinned ? partition : liveSnapshot.tabs.length,
								targetGroup,
								{ coalesceNonconsecutive: horizontal && !current.started },
							);
						}
						const moved = beforeIndex !== model.getIndexOfTab(current.sourceId);
						if (moved) lastReorderCoordinate = coordinate;
					}
				}
			}
			setDrag({
				...current,
				x: event.clientX,
				y: event.clientY,
				started,
				targetIndex,
				targetGroup,
				targetIdealLeft,
				lastReorderCoordinate,
			});
		};
		const finish = (event: PointerEvent) => {
			const dragAtFinish = dragRef.current;
			if (!dragAtFinish || dragAtFinish.settling || dragAtFinish.pointerId !== event.pointerId)
				return;
			let current: DragState = dragAtFinish;
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = null;
			if (event.type === 'pointercancel') {
				if (dragSettleTimer.current !== null) window.clearTimeout(dragSettleTimer.current);
				if (current.started) current.crossWindowRuntime?.end('cancel');
				if (horizontal && current.horizontalGroupDrag !== null) {
					const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
						type: 'cancel',
					});
					applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
					assertHorizontalGroupDragRefinement(model, canceled.state);
				} else {
					model.restoreCanceledDrag(current.restoreState);
					if (
						!horizontal &&
						current.groupHeaderId !== null &&
						!current.groupWasCollapsedBeforeDrag &&
						model.getIndexOfTab(current.sourceId) !== -1
					) {
						model.setSelection([current.sourceId], current.sourceId, { expandGroups: false });
					}
				}
				announce('Tab move canceled');
				setDrag(null);
				return;
			}
			const externalTargetActive =
				current.crossWindowRuntime?.getSnapshot?.().targetActive === true;
			if (current.started) current.crossWindowRuntime?.end('complete');
			if (current.started && !externalTargetActive && current.targetIndex !== null) {
				const liveSnapshot = model.getSnapshot();
				const source = liveSnapshot.tabs.find((tab) => tab.id === current.sourceId);
				if (source) {
					const firstUnpinned = liveSnapshot.tabs.findIndex((tab) => !tab.pinned);
					const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
					if (current.groupHeaderId !== null) {
						if (horizontal && current.horizontalGroupDrag !== null) {
							const movement = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
								type: 'move-to',
								preRemovalTargetIndex: current.targetIndex,
							});
							applyHorizontalGroupDragCommands(model, current.restoreState, movement.commands);
							assertHorizontalGroupDragRefinement(model, movement.state);
							current = { ...current, horizontalGroupDrag: movement.state };
						} else {
							model.moveGroupByDrag(current.groupHeaderId, current.targetIndex);
						}
					} else {
						model.moveTabsByDrag(
							current.tabIds,
							current.targetIndex,
							source.pinned ? 0 : partition,
							source.pinned ? partition : liveSnapshot.tabs.length,
							current.targetGroup,
						);
					}
				}
			} else if (current.pointerType === 'touch' && current.touchDragReady) {
				// Chromium emits LongTap after LongPress when the pointer is released
				// without entering the drag loop. Tabs open their context menu and
				// group headers open the group editor; this is distinct from a tap.
				if (current.groupHeaderId === null) {
					setMenu({ kind: 'tab', id: current.sourceId, x: event.clientX, y: event.clientY });
				} else {
					suppressGroupToggleRef.current = current.groupHeaderId;
					openGroupEditor(current.groupHeaderId, event.clientX, event.clientY);
				}
			} else if (
				current.groupHeaderId === null &&
				!event.shiftKey &&
				!current.shiftPressedOnPointerDown &&
				!(event.metaKey || event.ctrlKey)
			) {
				model.selectTab(current.sourceId);
			}
			if (current.started) {
				if (horizontal && !externalTargetActive && current.horizontalGroupDrag !== null) {
					const committed = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
						type: 'commit',
					});
					applyHorizontalGroupDragCommands(model, current.restoreState, committed.commands);
					assertHorizontalGroupDragRefinement(model, committed.state);
					current = { ...current, horizontalGroupDrag: committed.state };
				}
				const liveSnapshot = model.getSnapshot();
				const finalIndex = liveSnapshot.tabs.findIndex((tab) => tab.id === current.sourceId);
				if (finalIndex !== -1) {
					announce(
						`${current.label} moved to position ${finalIndex + 1} of ${liveSnapshot.tabs.length}`,
					);
				}
				if (current.groupHeaderId !== null) {
					if (
						(current.horizontalGroupDrag === null || externalTargetActive) &&
						!current.groupWasCollapsedBeforeDrag &&
						model.getGroup(current.groupHeaderId)
					) {
						model.updateGroup(current.groupHeaderId, { collapsed: false });
					}
					const completionActiveId = current.sourceId;
					if (
						(!horizontal || externalTargetActive) &&
						completionActiveId !== null &&
						model.getIndexOfTab(completionActiveId) !== -1
					) {
						// The active group member remains authoritative even while the
						// collapsed group header is its visible projection.
						model.setSelection([completionActiveId], completionActiveId, { expandGroups: false });
					}
				}
			}
			if (
				current.started &&
				horizontal &&
				!externalTargetActive &&
				current.targetIdealLeft !== null
			) {
				const settlingDrag = {
					...current,
					x: event.clientX,
					y: event.clientY,
					settling: true,
				};
				setDrag(settlingDrag);
				dragSettleTimer.current = window.setTimeout(() => {
					dragSettleTimer.current = null;
					setDrag((candidate) => {
						if (!candidate?.settling) return candidate;
						if (horizontal && candidate.horizontalGroupDrag !== null) {
							const settled = transitionHorizontalGroupDrag(candidate.horizontalGroupDrag, {
								type: 'settle',
							});
							applyHorizontalGroupDragCommands(model, candidate.restoreState, settled.commands);
							assertHorizontalGroupDragRefinement(model, settled.state);
						}
						return null;
					});
				}, 200);
			} else {
				setDrag(null);
			}
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', finish);
		window.addEventListener('pointercancel', finish);
		const keydown = (event: KeyboardEvent) => {
			const current = dragRef.current;
			if (!current || current.settling || event.key !== 'Escape') return;
			event.preventDefault();
			if (touchDragTimer.current !== null) window.clearTimeout(touchDragTimer.current);
			touchDragTimer.current = null;
			if (dragSettleTimer.current !== null) window.clearTimeout(dragSettleTimer.current);
			if (current.started) current.crossWindowRuntime?.end('cancel');
			if (horizontal && current.horizontalGroupDrag !== null) {
				const canceled = transitionHorizontalGroupDrag(current.horizontalGroupDrag, {
					type: 'cancel',
				});
				applyHorizontalGroupDragCommands(model, current.restoreState, canceled.commands);
				assertHorizontalGroupDragRefinement(model, canceled.state);
			} else {
				model.restoreCanceledDrag(current.restoreState);
				if (
					!horizontal &&
					current.groupHeaderId !== null &&
					!current.groupWasCollapsedBeforeDrag &&
					model.getIndexOfTab(current.sourceId) !== -1
				) {
					model.setSelection([current.sourceId], current.sourceId, { expandGroups: false });
				}
			}
			announce('Tab move canceled');
			setDrag(null);
		};
		window.addEventListener('keydown', keydown);
		return () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', finish);
			window.removeEventListener('pointercancel', finish);
			window.removeEventListener('keydown', keydown);
		};
	}, [
		dragPresent,
		horizontal,
		horizontalLayout,
		items,
		model,
		openGroupEditor,
		snapshot,
		stripRef,
		verticalCollapsed,
	]);

	const onTabKeyDown = (event: React.KeyboardEvent, tabId: string) => {
		if (event.key === 'Enter') {
			if (snapshot.selectedTabIds.has(tabId)) return;
			event.preventDefault();
			model.selectTab(tabId);
			return;
		}
		if (event.key === ' ') {
			event.preventDefault();
			return;
		}
		const previousKey = horizontal ? 'ArrowLeft' : 'ArrowUp';
		const nextKey = horizontal ? 'ArrowRight' : 'ArrowDown';
		let direction: -1 | 0 | 1 = 0;
		if (event.key === previousKey) direction = -1;
		else if (event.key === nextKey) direction = 1;
		if (direction === 0 || !(event.ctrlKey || event.metaKey)) return;
		event.preventDefault();
		if (event.shiftKey) {
			if (direction < 0) model.moveTabFirst(tabId);
			else model.moveTabLast(tabId);
			announce(
				getTabMoveAnnouncement(
					direction < 0 ? 'first' : 'last',
					horizontal ? 'horizontal' : 'vertical',
				),
			);
		} else {
			const before = model.getSnapshot();
			const beforeTab = before.tabs.find((tab) => tab.id === tabId);
			const beforeGroup =
				beforeTab?.group === null || beforeTab?.group === undefined
					? null
					: (before.groups.find((group) => group.id === beforeTab.group) ?? null);
			const tabsInBeforeGroup = beforeGroup
				? before.tabs.filter((tab) => tab.group === beforeGroup.id)
				: [];
			model.shiftTabRelative(tabId, direction);
			const after = model.getSnapshot();
			const afterTab = after.tabs.find((tab) => tab.id === tabId);
			const afterGroup =
				afterTab?.group === null || afterTab?.group === undefined
					? null
					: (after.groups.find((group) => group.id === afterTab.group) ?? null);
			if (beforeTab?.group !== afterTab?.group && (beforeGroup || afterGroup)) {
				const targetGroup = afterGroup ?? requireDefined(beforeGroup);
				const groupTabs = afterGroup
					? before.tabs.filter((tab) => tab.group === afterGroup.id)
					: tabsInBeforeGroup;
				announce(
					getTabGroupMembershipAnnouncement(
						afterGroup ? 'added' : 'removed',
						targetGroup,
						groupTabs,
					),
				);
			} else {
				announce(
					getTabMoveAnnouncement(
						direction < 0 ? 'previous' : 'next',
						horizontal ? 'horizontal' : 'vertical',
					),
				);
			}
		}
	};

	const onTabKeyUp = (event: React.KeyboardEvent, tabId: string) => {
		if (event.key !== ' ') return;
		event.preventDefault();
		if (!snapshot.selectedTabIds.has(tabId)) model.selectTab(tabId);
	};

	const addTab = () => {
		setTabClosingMode(null);
		onNewTabButtonPressed?.();
		if (onNewTab) onNewTab();
		else
			model.addTab({
				title: 'New Tab',
				pinned: false,
				group: null,
			});
	};

	const animatedHorizontalBoundsById = new Map(
		animatedHorizontalFrame?.layout.children.map((child) => [child.id, child.bounds]) ?? [],
	);
	const paintedHorizontalBounds = (index: number) => {
		const item = items[index];
		return item
			? (animatedHorizontalBoundsById.get(item.id) ?? horizontalLayout?.bounds[index])
			: horizontalLayout?.bounds[index];
	};
	const settleTargetLeft = (() => {
		if (!horizontal || !drag?.settling || !horizontalLayout) return null;
		const leadingDraggedView = drag.visualViews[0];
		if (!leadingDraggedView) return null;
		const visualItemId =
			leadingDraggedView.kind === 'group'
				? `group-${leadingDraggedView.id.slice('group:'.length)}`
				: leadingDraggedView.id;
		const visualIndex = items.findIndex((item) => item.id === visualItemId);
		const finalBounds = horizontalLayout.bounds[visualIndex];
		return finalBounds ? drag.dragAreaBounds.left + finalBounds.x : null;
	})();

	const focusedGroupColor = focusedGroup ? GROUP_COLORS[focusedGroup.color] : undefined;

	return {
		model,
		orientation,
		verticalCollapsed,
		verticalExpandOnHover,
		showHoverCardPreviews,
		showHoverCardMemoryUsage,
		showGroupHoverCards,
		showVerticalTopControls,
		verticalGroupFocused,
		showPinnedTabsInFocusedGroups,
		verticalToolbarHeight,
		verticalCaptionButtonWidth,
		verticalIsExitingExpandOnHover,
		onVerticalCollapsedChange,
		onVerticalUnfocusGroup,
		onVerticalTabSearchClick,
		onVerticalExpandOnHoverChange,
		focusedGroupColor,
		showNewTabButton,
		horizontalFrameCondensed,
		touchUi,
		tabStripDeclutter,
		onTabKeyboardFocusChanged,
		onMoveTabsToNewWindow,
		onMoveGroupToNewWindow,
		enableTabGroupFocusing,
		ariaLabel,
		snapshot,
		horizontal,
		focusedGroup,
		groupFocusActive,
		tabVisibleWhileFocused,
		stripRef,
		size,
		items,
		horizontalAnimationRuntimeRef,
		animatedHorizontalFrame,
		menu,
		setMenu,
		hover,
		drag,
		closingTabIds,
		fullyCollapsedGroupIds,
		tabClosingMode,
		announcement,
		groupEditorTracker,
		openGroupEditor,
		hoverCardSnapshot,
		thumbnailSnapshot,
		comboButtonController,
		comboButtonSnapshot,
		expandOnHoverController,
		expandOnHoverSnapshot,
		expandedOnHover,
		effectiveVerticalCollapsed,
		requestCloseTabs,
		expandedHeaderDragGroupId,
		horizontalLayout,
		clearHover,
		updateHoverForElement,
		onTabMouseEnter,
		onGroupMouseEnter,
		onTabPointerDown,
		onGroupPointerDown,
		toggleGroupFromHeader,
		onGroupKeyDown,
		onTabKeyDown,
		onTabKeyUp,
		addTab,
		paintedHorizontalBounds,
		settleTargetLeft,
	} as const;
}
