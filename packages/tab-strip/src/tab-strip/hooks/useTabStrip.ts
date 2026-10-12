import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from 'react';
import {
	HoverCardUpdateType,
	TabHoverCardController,
} from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-controller';
import {
	TAB_STRIP_ACTION_IDS,
	type TabSelectionInput,
} from '../actions/tab-strip-action-contracts';
import { TabHoverCardThumbnailRuntime } from '../components/tab-strip-view/components/hover-card/model/tab-hover-card-thumbnail-runtime';
import { resolveTabKeyboardReorder } from '../accessibility/resolveTabKeyboardReorder';
import {
	isTabStripKeyboardContextMenuRequest,
	projectTabStripRovingItems,
	resolveTabStripRovingFocus,
	resolveTabStripRovingNavigation,
	resolveTabStripRovingNavigationCommand,
	type TabStripRovingItem,
} from '../accessibility/tab-strip-roving-focus';
import { TabActive, TabOpen, TabPinned } from '../model/tab-types';
import { type TabData, type TabGroupData } from '../model/tab-model';
import {
	applyHorizontalGroupDragCommands,
	assertHorizontalGroupDragRefinement,
	createArmedHorizontalGroupDrag,
} from '../dragging/horizontal-group-drag-model-adapter';
import {
	buildDraggedTabsLayout,
	getDragStartAnimationProgress,
	getDraggingViewsBoundsAtPoint,
	getTargetBeforeDraggedTabs,
	shouldEnterVerticalTabGroup,
	shouldRemainInVerticalTabGroup,
} from '../dragging/dragged-tabs-container-layout';
import {
	calculateGroupForDraggedTabs,
	calculateHorizontalMoveThreshold,
	shouldReorderDraggedTabs,
} from '../dragging/dragging-tabs-session';
import { getHorizontalDragTarget } from '../dragging/tab-strip-drag-position';
import { transitionHorizontalGroupDrag } from '../dragging/horizontal-group-drag-machine';
import {
	TAB_CLOSE_TOUCH_RESIZE_DELAY_MS,
	calculateTabClosingModeWidth,
	getTabClosingModeWatchBounds,
	shouldExitTabClosingMode,
	tabClosingModeContainsPoint,
	type TabCloseSource,
} from '../layout/tab-closing-mode';
import { TabLayoutState } from '../layout/TabLayoutState';
import { TabWidthConstraints } from '../layout/TabWidthConstraints';
import { calculateTabBounds } from '../layout/calculateTabBounds';
import { getPinnedTabContainerDragAxes } from '../layout/pinned-tab-container-layout';
import {
	tabCollectionEaseOut,
	type AnimatedLayoutFrame,
} from '../layout/tab-collection-animating-layout-manager';
import { requireDefined } from '../utils/requireDefined';
import { TabStripOrientation, VerticalTabStripCollapseState } from '../types/tab-strip-types';
import { DEFAULT_TAB_STYLE_METRICS } from '../layout/tab-style-metrics';
import { LAYOUT_CONSTANTS } from '../layout/layout-constants';
import { TabGroupEditorBubbleTracker } from '../model/tab-group-editor-bubble-tracker';
import {
	ExpandOnHoverLockType,
	VerticalTabStripExpandOnHoverController,
} from '../model/vertical-tab-strip-expand-on-hover';
import { TabCollectionAnimationRuntime } from '../utils/TabCollectionAnimationRuntime';
import { TabStripComboButtonController } from '../model/tab-strip-combo-button';
import { useElementSize } from './useElementSize';
import { useTabStripActions } from './useTabStripActions';
import { DEFAULT_TAB_STRIP_MESSAGES } from '../constants/default-tab-strip-messages';
import type { TabStripMessages, TabStripMoveDirection } from '../types/tab-strip-messages';
import type { TabStripViewModel } from '../types/tab-strip-view-model';
import type { TabStripOptions } from '../types/tab-strip-options';
import {
	TAB_SIZE,
	TAB_BOUNDS_ANIMATION_DURATION_MS,
	requestManagedAnimationFrame,
	cancelManagedAnimationFrame,
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
	dragRequiresStartCompaction,
	type DragState,
	type MenuState,
	type HoverState,
} from '../utils/tab-strip-internals';

const HORIZONTAL_DRAG_SETTLE_FALLBACK_DELAY_MS = TAB_BOUNDS_ANIMATION_DURATION_MS + 50;

interface DragPointerUpdate {
	readonly pointerId: number;
	readonly clientX: number;
	readonly clientY: number;
	readonly screenX: number;
	readonly screenY: number;
	readonly forceReorder?: boolean;
}

export function useTabStrip(
	{
		actionRuntime,
		focusRequest = 0,
		identityIcons = new Map(),
		model,
		statusAdornments = new Map(),
		orientation = TabStripOrientation.Horizontal,
		onOrientationChange,
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
		verticalTabSearchOpen = false,
		verticalComboButtonController: externalComboButtonController,
		verticalToolbarHeight = 0,
		verticalCaptionButtonWidth = 0,
		verticalIsExitingExpandOnHover = false,
		onVerticalCollapsedChange,
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
		ariaLabel,
		onNewTab,
		getTabCreationAvailability,
		onCloseTabs,
		getCloseTabsConfirmation,
		onSelectTab,
		onSetTabTitle,
		onNewTabButtonPressed,
		parentActionScope,
		shortcutLabels,
	}: TabStripOptions,
	messages: TabStripMessages = DEFAULT_TAB_STRIP_MESSAGES,
): TabStripViewModel {
	const snapshot = useSyncExternalStore(model.subscribe, model.getSnapshot, model.getSnapshot);
	const horizontal = orientation === TabStripOrientation.Horizontal;
	const [stripRef, stripSize] = useElementSize<HTMLDivElement>();
	const [horizontalTrackRef, horizontalTrackSize] = useElementSize<HTMLDivElement>(horizontal);
	const size = horizontal ? horizontalTrackSize : stripSize;
	const items = useMemo(() => buildVisualItems(snapshot), [snapshot]);
	const rovingItems = useMemo(() => projectTabStripRovingItems(snapshot), [snapshot]);
	const [rememberedRovingFocusKey, setRememberedRovingFocusKey] = useState<
		TabStripRovingItem['key'] | null
	>(null);
	const previousRovingItemsRef = useRef(rovingItems);
	const previousRovingIndex =
		rememberedRovingFocusKey === null
			? null
			: previousRovingItemsRef.current.findIndex((item) => item.key === rememberedRovingFocusKey);
	const rovingFocusKey = resolveTabStripRovingFocus({
		activeTabId: snapshot.activeTabId,
		items: rovingItems,
		preferredKey: rememberedRovingFocusKey,
		previousIndex:
			previousRovingIndex === null || previousRovingIndex === -1 ? null : previousRovingIndex,
	});
	const rovingFocusWithinRef = useRef(false);
	const lastFocusRequestRef = useRef(0);
	const focusRovingItem = useMemoizedFn((key: TabStripRovingItem['key']): boolean => {
		const root = stripRef.current;
		if (root === null) return false;
		const element = [...root.querySelectorAll<HTMLElement>('[data-tab-strip-roving-key]')].find(
			(candidate) =>
				candidate.dataset['tabStripRovingKey'] === key &&
				candidate.closest('[aria-hidden="true"]') === null,
		);
		if (element === undefined) return false;
		setRememberedRovingFocusKey(key);
		element.focus({ preventScroll: true });
		return true;
	});
	const focusTabStripEntry = useMemoizedFn((): boolean => {
		if (rovingFocusKey !== null && focusRovingItem(rovingFocusKey)) return true;
		const newTabButton =
			stripRef.current?.querySelector<HTMLButtonElement>('[data-tab-strip-new-tab]') ?? null;
		if (newTabButton === null) return false;
		newTabButton.focus({ preventScroll: true });
		return true;
	});
	const onRovingFocusChanged = useMemoizedFn((key: TabStripRovingItem['key'] | null): void => {
		rovingFocusWithinRef.current = true;
		if (key !== null) setRememberedRovingFocusKey(key);
	});
	const onRovingFocusExited = useMemoizedFn((): void => {
		rovingFocusWithinRef.current = false;
	});
	useEffect(() => {
		previousRovingItemsRef.current = rovingItems;
		if (rememberedRovingFocusKey !== null && rememberedRovingFocusKey !== rovingFocusKey) {
			setRememberedRovingFocusKey(rovingFocusKey);
		}
	}, [rememberedRovingFocusKey, rovingFocusKey, rovingItems]);
	useLayoutEffect(() => {
		if (focusRequest === lastFocusRequestRef.current) return;
		lastFocusRequestRef.current = focusRequest;
		if (focusRequest > 0) focusTabStripEntry();
	}, [focusRequest, focusTabStripEntry]);
	useLayoutEffect(() => {
		if (!rovingFocusWithinRef.current) return;
		const root = stripRef.current;
		if (root === null) return;
		const activeElement = root.ownerDocument.activeElement;
		if (activeElement instanceof Node && root.contains(activeElement)) return;
		focusTabStripEntry();
	}, [focusTabStripEntry, rovingFocusKey, rovingItems, stripRef]);
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
	const dragStartAnimationRequestRef = useRef<number | null>(null);
	const verticalGroupPickupFrameRef = useRef<number | null>(null);
	const [tabClosingMode, setTabClosingMode] = useState<{
		width: number;
		source: Exclude<TabCloseSource, 'non-ui'>;
	} | null>(null);
	const [announcement, setAnnouncement] = useState({ text: '', sequence: 0 });
	const middlePressRef = useRef<{ tabId: string; pointerId: number } | null>(null);
	const touchLongPressTimer = useRef<number | null>(null);
	const animationBlockedPressRef = useRef<{
		kind: 'tab' | 'group';
		id: string;
		pointerId: number;
		pointerType: string;
		startX: number;
		startY: number;
		shiftPressed: boolean;
		modifierPressed: boolean;
		longPressRecognized: boolean;
	} | null>(null);
	const dragSettleTimer = useRef<number | null>(null);
	const dragRef = useRef<DragState | null>(null);
	const repositionDragAfterScrollRef = useRef<() => void>(() => {});
	const lastMousePositionRef = useRef<{ x: number; y: number } | null>(null);
	const horizontalAnimationWasRunningRef = useRef(false);
	const previousTabCountRef = useRef(snapshot.tabs.length);
	const verticalTabCountTransitionFrameRef = useRef<number | null>(null);
	const [verticalTabCountTransitionSuppressed, setVerticalTabCountTransitionSuppressed] =
		useState(false);
	const previousActiveTabIdRef = useRef(snapshot.activeTabId);
	const previousHorizontalActivationGeometryKeyRef = useRef<string | null>(null);
	const previousSelectionRef = useRef({
		activeTabId: snapshot.activeTabId,
		selectedTabIds: snapshot.selectedTabIds,
	});
	const tabCountChanged = snapshot.tabs.length !== previousTabCountRef.current;
	const suppressGroupToggleRef = useRef<string | null>(null);
	const suppressGroupToggleResetTimer = useRef<number | null>(null);
	const previousDraggingRef = useRef(false);
	const onDraggingChangeRef = useRef(onDraggingChange);
	const groupEditorTrackerRef = useRef<TabGroupEditorBubbleTracker | null>(null);
	const previousOrientationRef = useRef(orientation);
	if (groupEditorTrackerRef.current === null) {
		groupEditorTrackerRef.current = new TabGroupEditorBubbleTracker();
	}
	const groupEditorTracker = groupEditorTrackerRef.current;
	const internalComboButtonController = useRef<TabStripComboButtonController | null>(null);
	const comboLifecycleGeneration = useRef(0);
	if (internalComboButtonController.current === null) {
		internalComboButtonController.current = new TabStripComboButtonController({
			tabSearchPinned: showVerticalTabSearchButton,
		});
	}
	const ownedComboButtonController = internalComboButtonController.current;
	const comboButtonController = externalComboButtonController ?? ownedComboButtonController;
	const comboButtonSnapshot = useSyncExternalStore(
		comboButtonController.subscribe,
		comboButtonController.getSnapshot,
		comboButtonController.getSnapshot,
	);
	const previousTabSearchOpen = useRef<{
		controller: TabStripComboButtonController;
		open: boolean;
	} | null>(null);
	const openGroupEditor = useMemoizedFn((id: string, x: number, y: number) => {
		groupEditorTracker.open();
		setMenu({ kind: 'group', id, x, y });
	});
	const scheduleSuppressedGroupToggleReset = useMemoizedFn((groupId: string) => {
		if (suppressGroupToggleResetTimer.current !== null) {
			window.clearTimeout(suppressGroupToggleResetTimer.current);
		}
		suppressGroupToggleResetTimer.current = window.setTimeout(() => {
			suppressGroupToggleResetTimer.current = null;
			if (suppressGroupToggleRef.current === groupId) suppressGroupToggleRef.current = null;
		}, 0);
	});
	const cancelVerticalGroupPickupFrame = useMemoizedFn(() => {
		if (verticalGroupPickupFrameRef.current === null) return;
		cancelManagedAnimationFrame(verticalGroupPickupFrameRef.current);
		verticalGroupPickupFrameRef.current = null;
	});
	dragRef.current = drag;
	onDraggingChangeRef.current = onDraggingChange;
	const actions = useTabStripActions({
		actionRuntime,
		comboButtonController,
		messages,
		model,
		onMoveGroupToNewWindow,
		onMoveTabsToNewWindow,
		onNewTab,
		getTabCreationAvailability,
		onCloseTabs,
		getCloseTabsConfirmation,
		onSelectTab,
		onVerticalCollapsedChange,
		onVerticalExpandOnHoverChange,
		onVerticalTabSearchClick,
		parentActionScope,
		requestCloseTabs: commitCloseTabs,
		verticalCollapsed,
		verticalExpandOnHover,
	});

	useEffect(() => {
		if (dragStartAnimationRequestRef.current !== null) {
			cancelManagedAnimationFrame(dragStartAnimationRequestRef.current);
			dragStartAnimationRequestRef.current = null;
		}
		if (
			drag?.started !== true ||
			drag.settling ||
			!drag.dragStartAnimationRequired ||
			drag.dragStartAnimationStartedAt === null
		)
			return;
		const startedAt = drag.dragStartAnimationStartedAt;
		const tick = (now: number) => {
			const progress = getDragStartAnimationProgress(startedAt, now);
			setDrag((candidate) =>
				candidate?.dragStartAnimationStartedAt === startedAt
					? { ...candidate, dragStartAnimationProgress: progress }
					: candidate,
			);
			if (progress < 1) {
				dragStartAnimationRequestRef.current = requestManagedAnimationFrame(tick);
			} else {
				dragStartAnimationRequestRef.current = null;
			}
		};
		dragStartAnimationRequestRef.current = requestManagedAnimationFrame(tick);
		return () => {
			if (dragStartAnimationRequestRef.current === null) return;
			cancelManagedAnimationFrame(dragStartAnimationRequestRef.current);
			dragStartAnimationRequestRef.current = null;
		};
	}, [
		drag?.dragStartAnimationRequired,
		drag?.dragStartAnimationStartedAt,
		drag?.settling,
		drag?.started,
	]);

	useEffect(() => {
		const started = drag?.started === true && !drag.settling;
		if (started === previousDraggingRef.current) return;
		previousDraggingRef.current = started;
		onDraggingChangeRef.current?.(started);
	}, [drag?.settling, drag?.started]);

	useEffect(
		() => () => {
			if (touchLongPressTimer.current !== null) {
				window.clearTimeout(touchLongPressTimer.current);
				touchLongPressTimer.current = null;
			}
			if (dragSettleTimer.current !== null) {
				window.clearTimeout(dragSettleTimer.current);
				dragSettleTimer.current = null;
			}
			if (suppressGroupToggleResetTimer.current !== null) {
				window.clearTimeout(suppressGroupToggleResetTimer.current);
				suppressGroupToggleResetTimer.current = null;
			}
			if (dragStartAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(dragStartAnimationRequestRef.current);
				dragStartAnimationRequestRef.current = null;
			}
			cancelVerticalGroupPickupFrame();
			if (!previousDraggingRef.current) return;
			previousDraggingRef.current = false;
			onDraggingChangeRef.current?.(false);
		},
		[cancelVerticalGroupPickupFrame],
	);

	useEffect(() => {
		const finishAnimationBlockedPress = (event: PointerEvent) => {
			const press = animationBlockedPressRef.current;
			if (!press || press.pointerId !== event.pointerId) return;
			animationBlockedPressRef.current = null;
			if (touchLongPressTimer.current !== null) {
				window.clearTimeout(touchLongPressTimer.current);
				touchLongPressTimer.current = null;
			}
			if (event.type === 'pointercancel') return;
			if (press.pointerType === 'touch' && press.longPressRecognized) {
				if (press.kind === 'tab') {
					setMenu({ kind: 'tab', id: press.id, x: event.clientX, y: event.clientY });
				} else {
					suppressGroupToggleRef.current = press.id;
					scheduleSuppressedGroupToggleReset(press.id);
					openGroupEditor(press.id, event.clientX, event.clientY);
				}
				return;
			}
			if (press.kind === 'tab' && !press.shiftPressed && !press.modifierPressed) {
				actions.execute(
					TAB_STRIP_ACTION_IDS.selectTab,
					{ mode: 'replace', tabId: press.id },
					'pointer',
				);
			}
		};
		window.addEventListener('pointerup', finishAnimationBlockedPress);
		window.addEventListener('pointercancel', finishAnimationBlockedPress);
		const moveAnimationBlockedPress = (event: PointerEvent) => {
			const press = animationBlockedPressRef.current;
			if (
				!press ||
				press.pointerId !== event.pointerId ||
				press.pointerType !== 'touch' ||
				Math.hypot(event.clientX - press.startX, event.clientY - press.startY) <= 10 ||
				touchLongPressTimer.current === null
			)
				return;
			window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = null;
		};
		window.addEventListener('pointermove', moveAnimationBlockedPress);
		return () => {
			window.removeEventListener('pointerup', finishAnimationBlockedPress);
			window.removeEventListener('pointercancel', finishAnimationBlockedPress);
			window.removeEventListener('pointermove', moveAnimationBlockedPress);
		};
	}, [actions, openGroupEditor, scheduleSuppressedGroupToggleReset]);

	useEffect(() => {
		const recordMousePosition = (event: PointerEvent) => {
			if (event.pointerType !== 'mouse') return;
			lastMousePositionRef.current = { x: event.clientX, y: event.clientY };
		};
		window.addEventListener('pointermove', recordMousePosition);
		return () => window.removeEventListener('pointermove', recordMousePosition);
	}, []);

	const finishHorizontalDragSettle = useMemoizedFn(() => {
		if (dragSettleTimer.current !== null) {
			window.clearTimeout(dragSettleTimer.current);
			dragSettleTimer.current = null;
		}
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
	});

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
			if (
				target instanceof Element &&
				target.closest('[data-slot="context-menu-content"], [data-slot="context-menu-sub-content"]')
			)
				return;
			const header =
				target instanceof Element ? target.closest<HTMLElement>('[data-group-id]') : null;
			if (menu.kind === 'group' && header?.dataset['groupId'] === menu.id) {
				suppressGroupToggleRef.current = menu.id;
				scheduleSuppressedGroupToggleReset(menu.id);
			}
			setMenu(null);
		};
		window.addEventListener('pointerdown', dismissOnOutsidePointer, true);
		return () => window.removeEventListener('pointerdown', dismissOnOutsidePointer, true);
	}, [menu, scheduleSuppressedGroupToggleReset]);

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
		if (!current || current.settling) return;
		const liveIds = new Set(snapshot.tabs.map((tab) => tab.id));
		if (current.tabIds.every((id) => liveIds.has(id))) return;
		const runtimeSnapshot = current.crossWindowRuntime?.getSnapshot?.();
		const transferredToAnotherContext =
			current.started &&
			runtimeSnapshot?.sourceContextId !== undefined &&
			runtimeSnapshot.attachedContextId !== undefined &&
			runtimeSnapshot.attachedContextId !== runtimeSnapshot.sourceContextId;
		if (transferredToAnotherContext) return;
		if (touchLongPressTimer.current !== null) {
			window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = null;
		}
		if (current.started) {
			current.crossWindowRuntime?.end('tab-destroyed');
			announce(messages.tabMoveCanceled);
		}
		cancelVerticalGroupPickupFrame();
		setDrag(null);
	}, [cancelVerticalGroupPickupFrame, messages.tabMoveCanceled, snapshot.revision, snapshot.tabs]);

	useEffect(() => {
		const runtime = drag?.crossWindowRuntime;
		if (!runtime?.subscribe) return;
		return runtime.subscribe(() => {
			const runtimeSnapshot = runtime.getSnapshot?.();
			if (runtimeSnapshot?.endReason !== 'tab-destroyed') return;
			if (touchLongPressTimer.current !== null) {
				window.clearTimeout(touchLongPressTimer.current);
				touchLongPressTimer.current = null;
			}
			setAnnouncement((current) => ({
				text: messages.tabMoveCanceled,
				sequence: current.sequence + 1,
			}));
			cancelVerticalGroupPickupFrame();
			setDrag((current) => (current?.crossWindowRuntime === runtime ? null : current));
		});
	}, [cancelVerticalGroupPickupFrame, drag?.crossWindowRuntime, messages.tabMoveCanceled]);

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
					announce(messages.groupMoveCanceled);
				}
				dragRef.current = null;
				cancelVerticalGroupPickupFrame();
				setDrag(null);
			}),
		[cancelVerticalGroupPickupFrame, horizontal, messages.groupMoveCanceled, model],
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

	useEffect(() => {
		comboLifecycleGeneration.current += 1;
		const generation = comboLifecycleGeneration.current;
		return () => {
			// Effect replay retains the original timers; only final unmount retires the owner.
			queueMicrotask(() => {
				if (comboLifecycleGeneration.current === generation) ownedComboButtonController.destroy();
			});
		};
	}, [ownedComboButtonController]);
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
		if (
			previousTabSearchOpen.current?.controller === comboButtonController &&
			previousTabSearchOpen.current.open === verticalTabSearchOpen
		)
			return;
		previousTabSearchOpen.current = {
			controller: comboButtonController,
			open: verticalTabSearchOpen,
		};
		if (comboButtonController.getSnapshot().bubbleOpen === verticalTabSearchOpen) return;
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

	function commitCloseTabs(tabIds: readonly string[], source: TabCloseSource = 'non-ui'): void {
		const beforeClose = model.getSnapshot();
		const existingIds = new Set(beforeClose.tabs.map((tab) => tab.id));
		const ids = [...new Set(tabIds)].filter((id) => existingIds.has(id));
		if (ids.length === 0) return;
		const closingTitles = beforeClose.tabs
			.filter((tab) => ids.includes(tab.id))
			.map((tab) => tab.title || messages.newTabTitle);
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
		model.closeTabs(ids);
		if (ids.length === 1) {
			announce(messages.formatClosedTabAnnouncement(requireDefined(closingTitles[0])));
		} else {
			announce(messages.formatClosedTabsAnnouncement(ids.length));
		}
	}
	const requestCloseTabs = (tabIds: readonly string[], source: TabCloseSource = 'non-ui'): void => {
		actions.execute(
			TAB_STRIP_ACTION_IDS.closeTabs,
			{ closeSource: source, tabIds },
			source === 'non-ui' ? 'programmatic' : 'pointer',
		);
	};

	useEffect(() => {
		if (tabClosingMode?.source !== 'touch') return;
		let timer = 0;
		const finishWhenIdle = () => {
			timer = window.setTimeout(() => {
				if (dragRef.current?.started && !dragRef.current.settling) finishWhenIdle();
				else setTabClosingMode(null);
			}, TAB_CLOSE_TOUCH_RESIZE_DELAY_MS);
		};
		finishWhenIdle();
		return () => window.clearTimeout(timer);
	}, [tabClosingMode?.source, tabClosingMode?.width]);

	useEffect(() => {
		if (tabClosingMode?.source !== 'mouse' || menu?.kind === 'group') return;
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
	}, [menu?.kind, stripRef, tabClosingMode?.source]);

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
	const horizontalLayoutProjection = useMemo(() => {
		if (!horizontal) return null;
		const activationIndependentGeometry: Array<{
			id: string;
			open: TabOpen;
			pinned: TabPinned;
			size: ReturnType<typeof sizeInfoForItem>;
		}> = [];
		const constraints = items.map((item) => {
			const group =
				item.type === 'tab' && item.tab.group !== null
					? snapshot.groups.find((candidate) => candidate.id === item.tab.group)
					: null;
			const hiddenByCollapsedGroup =
				item.type === 'tab' && Boolean(group?.collapsed) && group?.id !== expandedHeaderDragGroupId;
			const state = new TabLayoutState(
				hiddenByCollapsedGroup ? TabOpen.Closed : TabOpen.Open,
				item.type === 'tab' && item.tab.pinned ? TabPinned.Pinned : TabPinned.Unpinned,
				item.type === 'tab' && item.tab.id === snapshot.activeTabId
					? TabActive.Active
					: TabActive.Inactive,
			);
			const sizeItem =
				item.type === 'group' && item.group.id === expandedHeaderDragGroupId
					? { ...item, group: { ...item.group, collapsed: false } }
					: item;
			const sizeInfo = sizeInfoForItem(sizeItem);
			activationIndependentGeometry.push({
				id: item.id,
				open: state.open,
				pinned: state.pinned,
				size: sizeInfo,
			});
			return new TabWidthConstraints(state, sizeInfo);
		});
		return {
			activationIndependentGeometryKey: JSON.stringify([
				layoutWidth,
				effectiveLayoutWidth,
				activationIndependentGeometry,
			]),
			layout: calculateTabBounds(constraints, effectiveLayoutWidth),
		};
	}, [
		effectiveLayoutWidth,
		expandedHeaderDragGroupId,
		horizontal,
		items,
		layoutWidth,
		snapshot.groups,
		snapshot.activeTabId,
	]);
	const horizontalLayout = horizontalLayoutProjection?.layout ?? null;
	const horizontalActivationGeometryKey =
		horizontalLayoutProjection?.activationIndependentGeometryKey ?? null;

	useEffect(() => {
		const runtime = horizontalAnimationRuntimeRef.current;
		const previousActiveTabId = previousActiveTabIdRef.current;
		const previousActivationGeometryKey = previousHorizontalActivationGeometryKeyRef.current;
		previousActiveTabIdRef.current = snapshot.activeTabId;
		previousHorizontalActivationGeometryKeyRef.current = horizontalActivationGeometryKey;
		if (!runtime || !horizontal || !horizontalLayout) {
			setAnimatedHorizontalFrame(null);
			return;
		}
		const activeChanged = previousActiveTabId !== snapshot.activeTabId;
		const activationOnlyGeometryChange =
			activeChanged &&
			previousActivationGeometryKey !== null &&
			previousActivationGeometryKey === horizontalActivationGeometryKey;
		const animationWasRunning = runtime.isAnimating;
		const target = {
			// Tab-closing mode constrains child target bounds without resizing the
			// collection host. Keeping the host width stable lets exiting closing
			// mode animate the ordinary tabs back to their unconstrained layout.
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
				pinned: item.type === 'tab' ? item.tab.pinned : null,
				dragging:
					item.type === 'group'
						? Boolean(
								drag?.started &&
								drag.visualViews.some((view) => view.id === `group:${item.group.id}`),
							)
						: Boolean(drag?.started && drag.tabIds.includes(item.tab.id)),
			})),
		);
		// Chromium snaps active-width changes that begin from a steady tab strip.
		// Existing bounds motion or a live drag is retargeted instead.
		if (activationOnlyGeometryChange && !animationWasRunning && !drag?.started) {
			if (horizontalAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
				horizontalAnimationRequestRef.current = null;
			}
			setAnimatedHorizontalFrame(runtime.complete());
			return;
		}
		// Adding and closing tabs are immediate; the shared runtime still animates
		// reorders and other same-count layout changes.
		if (tabCountChanged) {
			if (horizontalAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
				horizontalAnimationRequestRef.current = null;
			}
			setAnimatedHorizontalFrame(runtime.complete());
			return;
		}
		setAnimatedHorizontalFrame(synced.frame);
		if (!runtime.isAnimating) {
			if (horizontalAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
				horizontalAnimationRequestRef.current = null;
			}
			return;
		}
		if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			if (horizontalAnimationRequestRef.current !== null) {
				cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
				horizontalAnimationRequestRef.current = null;
			}
			setAnimatedHorizontalFrame(runtime.complete());
			return;
		}

		// Drag cleanup changes slot ownership without changing target bounds. Keep
		// the existing timeline running instead of restarting it from its current
		// frame, matching BoundsAnimator's stable-target behavior.
		if (!synced.changed && horizontalAnimationRequestRef.current !== null) return;
		if (horizontalAnimationRequestRef.current !== null) {
			cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
			horizontalAnimationRequestRef.current = null;
		}

		const startedAt = performance.now();
		const startProgress = synced.startProgress;
		const duration = synced.changed
			? TAB_BOUNDS_ANIMATION_DURATION_MS
			: TAB_BOUNDS_ANIMATION_DURATION_MS * (1 - startProgress);
		if (duration <= 0) {
			setAnimatedHorizontalFrame(runtime.complete());
			return;
		}
		const tick = (now: number) => {
			const elapsed = Math.max(0, now - startedAt);
			const timeline = Math.min(1, elapsed / duration);
			const progress = startProgress + (1 - startProgress) * tabCollectionEaseOut(timeline);
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
		horizontalActivationGeometryKey,
		horizontalLayout,
		items,
		layoutWidth,
		snapshot.activeTabId,
		tabCountChanged,
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
		if (!tabCountChanged) return;
		setVerticalTabCountTransitionSuppressed(true);
		if (verticalTabCountTransitionFrameRef.current !== null) {
			cancelManagedAnimationFrame(verticalTabCountTransitionFrameRef.current);
		}
		verticalTabCountTransitionFrameRef.current = requestManagedAnimationFrame(() => {
			verticalTabCountTransitionFrameRef.current = null;
			setVerticalTabCountTransitionSuppressed(false);
		});
	}, [snapshot.tabs.length, tabCountChanged]);

	useEffect(
		() => () => {
			if (verticalTabCountTransitionFrameRef.current !== null) {
				cancelManagedAnimationFrame(verticalTabCountTransitionFrameRef.current);
			}
		},
		[],
	);

	useEffect(() => {
		if (!tabClosingMode) return;
		const trailing = horizontalLayout?.bounds.at(-1);
		if (trailing && shouldExitTabClosingMode(tabClosingMode.width, trailing.x + trailing.width)) {
			setTabClosingMode(null);
		}
	}, [horizontalLayout, tabClosingMode]);

	const clearHover = useMemoizedFn(
		(updateType: HoverCardUpdateType = HoverCardUpdateType.Hover) => {
			if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
			hoverTimer.current = null;
			hoverCardController.updateHoverCard(null, updateType);
			setHover(null);
		},
	);

	const completeHorizontalAnimationsForDragStart = useMemoizedFn(() => {
		if (!horizontal) return;
		if (horizontalAnimationRequestRef.current !== null) {
			cancelManagedAnimationFrame(horizontalAnimationRequestRef.current);
			horizontalAnimationRequestRef.current = null;
		}
		const runtime = horizontalAnimationRuntimeRef.current;
		if (runtime?.isAnimating) setAnimatedHorizontalFrame(runtime.complete());
	});

	useEffect(() => {
		const previousSelection = previousSelectionRef.current;
		previousSelectionRef.current = {
			activeTabId: snapshot.activeTabId,
			selectedTabIds: snapshot.selectedTabIds,
		};
		const selectionChanged =
			previousSelection.activeTabId !== snapshot.activeTabId ||
			previousSelection.selectedTabIds.size !== snapshot.selectedTabIds.size ||
			[...snapshot.selectedTabIds].some((tabId) => !previousSelection.selectedTabIds.has(tabId));
		if (!selectionChanged) return;
		clearHover(HoverCardUpdateType.SelectionChanged);
	}, [clearHover, snapshot.activeTabId, snapshot.selectedTabIds]);

	useEffect(() => () => clearHover(HoverCardUpdateType.Event), [clearHover]);

	const updateHoverForElement = useMemoizedFn(
		(
			element: HTMLElement,
			id: string,
			updateType: HoverCardUpdateType,
			kind: HoverState['kind'] = 'tab',
		) => {
			const targetTab =
				kind === 'tab' ? (snapshot.tabs.find((tab) => tab.id === id) ?? null) : null;
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
							...(element
								.closest('[data-ct-root]')
								?.querySelectorAll<HTMLElement>('[data-tab-id]') ?? []),
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
		},
	);

	const onTabMouseEnter = (event: ReactMouseEvent<HTMLDivElement>, tabId: string) => {
		lastMousePositionRef.current = { x: event.clientX, y: event.clientY };
		updateHoverForElement(event.currentTarget, tabId, HoverCardUpdateType.Hover);
	};

	const onGroupMouseEnter = (event: ReactMouseEvent<HTMLDivElement>, groupId: string) => {
		if (!showGroupHoverCards) return;
		lastMousePositionRef.current = { x: event.clientX, y: event.clientY };
		updateHoverForElement(event.currentTarget, groupId, HoverCardUpdateType.Hover, 'group');
	};

	useEffect(() => {
		const animationRunning = horizontalAnimationRuntimeRef.current?.isAnimating ?? false;
		const wasRunning = horizontalAnimationWasRunningRef.current;
		horizontalAnimationWasRunningRef.current = animationRunning;
		if (animationRunning) {
			// TabContainerImpl suppresses hover cards while any tab-slot bounds
			// are animating so cards never remain anchored to a moving view.
			clearHover(HoverCardUpdateType.Animating);
			return;
		}
		if (!wasRunning || (dragRef.current?.started && !dragRef.current.settling)) return;
		const point = lastMousePositionRef.current;
		const root = stripRef.current;
		if (!point || !root) return;
		const hit = document.elementFromPoint(point.x, point.y);
		const anchor =
			hit instanceof Element ? hit.closest<HTMLElement>('[data-tab-id], [data-group-id]') : null;
		if (!anchor || !root.contains(anchor)) return;
		const tabId = anchor.dataset['tabId'];
		if (tabId) {
			updateHoverForElement(anchor, tabId, HoverCardUpdateType.Hover);
			return;
		}
		const groupId = anchor.dataset['groupId'];
		if (groupId && showGroupHoverCards) {
			updateHoverForElement(anchor, groupId, HoverCardUpdateType.Hover, 'group');
		}
	}, [
		animatedHorizontalFrame,
		clearHover,
		horizontalAnimationRuntimeRef,
		showGroupHoverCards,
		stripRef,
		updateHoverForElement,
	]);

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
		let mode: TabSelectionInput['mode'] = 'replace';
		if (event.shiftKey && modifier) mode = 'add-range';
		else if (event.shiftKey) mode = 'extend';
		else if (modifier) mode = 'toggle';
		else if (model.getSnapshot().selectedTabIds.has(tab.id)) return;
		actions.execute(TAB_STRIP_ACTION_IDS.selectTab, { mode, tabId: tab.id }, 'pointer');
	};

	const armAnimationBlockedPress = (
		event: ReactPointerEvent<HTMLDivElement>,
		kind: 'tab' | 'group',
		id: string,
	) => {
		const touchPointer = event.pointerType === 'touch';
		animationBlockedPressRef.current = {
			kind,
			id,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			startX: event.clientX,
			startY: event.clientY,
			shiftPressed: event.shiftKey,
			modifierPressed: event.metaKey || event.ctrlKey,
			longPressRecognized: false,
		};
		if (touchPointer) {
			if (touchLongPressTimer.current !== null) window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = window.setTimeout(() => {
				touchLongPressTimer.current = null;
				const press = animationBlockedPressRef.current;
				if (!press || press.pointerId !== event.pointerId) return;
				animationBlockedPressRef.current = { ...press, longPressRecognized: true };
			}, 500);
		} else {
			event.currentTarget.setPointerCapture?.(event.pointerId);
		}
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
		// TabStrip::MaybeStartDrag refuses to create a controller while tab-slot
		// bounds are animating. Selection and ordinary click completion still run.
		if (horizontal && horizontalAnimationRuntimeRef.current?.isAnimating) {
			armAnimationBlockedPress(event, 'tab', tab.id);
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
		const dragSessionData = buildDragSessionData({
			snapshot: pointerSnapshot,
			tabIds: draggedTabIds,
			sourceId: tab.id,
			groupHeaderId: null,
			groupWasCollapsedBeforeDrag: false,
			pointerOffsetRatios: visuals.pointerOffsetRatios,
		});
		const next: DragState = {
			sourceId: tab.id,
			tabIds: draggedTabIds,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			touchLongPressRecognized: false,
			startX: event.clientX,
			startY: event.clientY,
			screenX: event.screenX,
			screenY: event.screenY,
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
			dragSessionData,
			dragStartAnimationRequired: false,
			dragStartAnimationStartedAt: null,
			dragStartAnimationProgress: 1,
			crossWindowRuntime: createCrossWindowDragRuntime?.(dragSessionData) ?? null,
			settling: false,
		};
		setDrag(next);
		if (touchPointer) {
			if (touchLongPressTimer.current !== null) window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = window.setTimeout(() => {
				touchLongPressTimer.current = null;
				const current = dragRef.current;
				if (!current || current.pointerId !== event.pointerId || current.pointerType !== 'touch')
					return;
				const liveSnapshot = model.getSnapshot();
				if (!liveSnapshot.selectedTabIds.has(current.sourceId)) {
					actions.execute(
						TAB_STRIP_ACTION_IDS.selectTab,
						{ mode: 'replace', tabId: current.sourceId },
						'pointer',
					);
				}
				setDrag((candidate) =>
					candidate?.pointerId === event.pointerId
						? { ...candidate, touchLongPressRecognized: true }
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
		if (horizontal && horizontalAnimationRuntimeRef.current?.isAnimating) {
			armAnimationBlockedPress(event, 'group', group.id);
			return;
		}
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
		const dragSessionData = buildDragSessionData({
			snapshot: pointerSnapshot,
			tabIds: draggedTabIds,
			sourceId: active.id,
			groupHeaderId: group.id,
			groupWasCollapsedBeforeDrag: group.collapsed,
			pointerOffsetRatios: visuals.pointerOffsetRatios,
		});
		const next: DragState = {
			sourceId: active.id,
			tabIds: draggedTabIds,
			pointerId: event.pointerId,
			pointerType: event.pointerType,
			touchLongPressRecognized: false,
			startX: event.clientX,
			startY: event.clientY,
			screenX: event.screenX,
			screenY: event.screenY,
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
			label: group.title || messages.unnamedGroup,
			...visuals,
			dragSessionData,
			dragStartAnimationRequired: false,
			dragStartAnimationStartedAt: null,
			dragStartAnimationProgress: 1,
			crossWindowRuntime: createCrossWindowDragRuntime?.(dragSessionData) ?? null,
			settling: false,
		};
		setDrag(next);
		if (touchPointer) {
			if (touchLongPressTimer.current !== null) window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = window.setTimeout(() => {
				touchLongPressTimer.current = null;
				setDrag((candidate) =>
					candidate?.pointerId === event.pointerId
						? { ...candidate, touchLongPressRecognized: true }
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
			if (suppressGroupToggleResetTimer.current !== null) {
				window.clearTimeout(suppressGroupToggleResetTimer.current);
				suppressGroupToggleResetTimer.current = null;
			}
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
		if (group) {
			actions.execute(
				TAB_STRIP_ACTION_IDS.setGroupCollapsed,
				{ collapsed: !group.collapsed, groupId },
				source === 'keyboard' ? 'keyboard' : 'pointer',
			);
		}
	};

	const onGroupKeyDown = (event: React.KeyboardEvent<HTMLDivElement>, groupId: string) => {
		if (isTabStripKeyboardContextMenuRequest(event)) {
			event.preventDefault();
			const bounds = event.currentTarget.getBoundingClientRect();
			openGroupEditor(groupId, bounds.left + bounds.width / 2, bounds.bottom);
			return;
		}
		const navigation = resolveTabStripRovingNavigationCommand(event, horizontal);
		if (navigation !== null) {
			event.preventDefault();
			const target = resolveTabStripRovingNavigation({
				command: navigation,
				currentKey: `group:${groupId}`,
				items: rovingItems,
			});
			if (target !== null) focusRovingItem(target);
			return;
		}
		if (!(event.ctrlKey || event.metaKey)) return;
		const previousKey = horizontal ? 'ArrowLeft' : 'ArrowUp';
		const nextKey = horizontal ? 'ArrowRight' : 'ArrowDown';
		let direction: -1 | 0 | 1 = 0;
		if (event.key === previousKey) direction = -1;
		else if (event.key === nextKey) direction = 1;
		if (direction === 0) return;
		event.preventDefault();
		actions.execute(TAB_STRIP_ACTION_IDS.moveGroup, { direction, groupId }, 'keyboard');
		let relativeDirection: TabStripMoveDirection = direction < 0 ? 'up' : 'down';
		if (horizontal) relativeDirection = direction < 0 ? 'left' : 'right';
		announce(messages.formatGroupMovedAnnouncement(relativeDirection));
	};

	const dragPresent = drag !== null;
	const repositionDragAfterScroll = useMemoizedFn(() => {
		repositionDragAfterScrollRef.current();
	});
	useEffect(() => {
		if (!dragPresent) return;
		const moveDrag = (event: Readonly<DragPointerUpdate>) => {
			const dragAtStart = dragRef.current;
			if (!dragAtStart || dragAtStart.settling || dragAtStart.pointerId !== event.pointerId) return;
			let current: DragState = dragAtStart;
			if (verticalGroupPickupFrameRef.current !== null && current.started) {
				const pendingPickup = {
					...current,
					x: event.clientX,
					y: event.clientY,
					screenX: event.screenX,
					screenY: event.screenY,
				};
				dragRef.current = pendingPickup;
				setDrag(pendingPickup);
				return;
			}
			const distance = Math.hypot(event.clientX - current.startX, event.clientY - current.startY);
			if (current.pointerType === 'touch' && current.touchLongPressRecognized) return;
			if (
				current.pointerType === 'touch' &&
				!current.touchLongPressRecognized &&
				distance > 10 &&
				touchLongPressTimer.current !== null
			) {
				window.clearTimeout(touchLongPressTimer.current);
				touchLongPressTimer.current = null;
			}
			const started = current.started || distance > 10;
			if (started && !current.started) {
				const liveSnapshot = model.getSnapshot();
				const liveIds = new Set(liveSnapshot.tabs.map((tab) => tab.id));
				const groupStillExists =
					current.groupHeaderId === null ||
					liveSnapshot.groups.some((group) => group.id === current.groupHeaderId);
				if (!groupStillExists || current.tabIds.some((id) => !liveIds.has(id))) {
					setDrag(null);
					return;
				}
				if (touchLongPressTimer.current !== null) {
					window.clearTimeout(touchLongPressTimer.current);
					touchLongPressTimer.current = null;
				}
				// StartedDragging completes any residual end-drag or slot animation
				// before the real views are promoted into the drag context.
				completeHorizontalAnimationsForDragStart();
				const dragStartAnimationStartedAt = performance.now();
				const dragStartAnimationRequired =
					!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches &&
					dragRequiresStartCompaction({
						drag: current,
						snapshot: liveSnapshot,
						horizontal,
						verticalCollapsed,
					});
				current.dragSessionData.markDragStarted(
					dragStartAnimationStartedAt,
					dragStartAnimationRequired,
				);
				current = {
					...current,
					dragStartAnimationRequired,
					dragStartAnimationStartedAt,
					dragStartAnimationProgress: dragStartAnimationRequired ? 0 : 1,
				};
				current.crossWindowRuntime?.start();
			}
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
						if (!horizontal) {
							const pendingPickup = {
								...current,
								x: event.clientX,
								y: event.clientY,
								screenX: event.screenX,
								screenY: event.screenY,
								started: true,
								targetIndex: null,
								targetGroup: null,
							};
							dragRef.current = pendingPickup;
							setDrag(pendingPickup);
							verticalGroupPickupFrameRef.current = requestManagedAnimationFrame(() => {
								verticalGroupPickupFrameRef.current = null;
								// Chromium completes the programmatic group collapse before
								// running live drag targeting. Force that collapsed layout to
								// become the next reorder animation's source geometry.
								stripRef.current?.getBoundingClientRect();
								const latest = dragRef.current;
								if (!latest?.started || latest.settling || latest.pointerId !== event.pointerId)
									return;
								moveDrag({
									pointerId: latest.pointerId,
									clientX: latest.x,
									clientY: latest.y,
									screenX: latest.screenX,
									screenY: latest.screenY,
									forceReorder: true,
								});
							});
							return;
						}
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
					const rootRect = (horizontalTrackRef.current ?? root).getBoundingClientRect();
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
					targetIndex = dragTarget.preRemovalIndex;
					targetIdealLeft = rootRect.left + dragTarget.idealX;
					targetExtent =
						idealTabBounds[Math.min(dragTarget.insertionIndex, idealTabBounds.length - 1)]?.width ??
						TAB_SIZE.standardWidth;
					const draggedIds = new Set(current.tabIds);
					const remaining = liveSnapshot.tabs.filter((tab) => !draggedIds.has(tab.id));
					const insertion = Math.max(0, Math.min(remaining.length, dragTarget.insertionIndex));
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
				let threshold = 8;
				if (horizontal) {
					threshold = calculateHorizontalMoveThreshold(targetExtent, TAB_SIZE.standardWidth);
				} else if (pinnedGridDrag) {
					threshold = Math.max(
						2,
						calculateHorizontalMoveThreshold(targetExtent, TAB_SIZE.standardWidth),
					);
				}
				const initialMove = !current.started;
				const tabsConsecutive = areDraggedTabIdsConsecutive(
					current.restoreState.tabs,
					current.tabIds,
				);
				if (
					!externalTargetActive &&
					(event.forceReorder === true ||
						shouldReorderDraggedTabs({
							justAttached: initialMove,
							movementSinceLastReorder: coordinate - current.lastReorderCoordinate,
							threshold,
							initialMove,
							tabsConsecutive,
						}))
				) {
					if (source) {
						const orderBeforeMove = liveSnapshot.tabs.map((tab) => tab.id);
						const firstUnpinned = liveSnapshot.tabs.findIndex((candidate) => !candidate.pinned);
						const partition = firstUnpinned === -1 ? liveSnapshot.tabs.length : firstUnpinned;
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
						const orderAfterMove = model.getSnapshot().tabs;
						const modelOrderChanged =
							orderBeforeMove.length !== orderAfterMove.length ||
							orderBeforeMove.some((id, index) => orderAfterMove[index]?.id !== id);
						if (modelOrderChanged) lastReorderCoordinate = coordinate;
					}
				}
			}
			setDrag({
				...current,
				x: event.clientX,
				y: event.clientY,
				screenX: event.screenX,
				screenY: event.screenY,
				started,
				targetIndex,
				targetGroup,
				targetIdealLeft,
				lastReorderCoordinate,
			});
		};
		const move = (event: PointerEvent) => {
			moveDrag(event);
		};
		const repositionAfterScroll = () => {
			const current = dragRef.current;
			if (!current?.started || current.settling) return;
			moveDrag({
				pointerId: current.pointerId,
				clientX: current.x,
				clientY: current.y,
				screenX: current.screenX,
				screenY: current.screenY,
				forceReorder: true,
			});
		};
		repositionDragAfterScrollRef.current = repositionAfterScroll;
		const finish = (event: PointerEvent) => {
			const dragAtFinish = dragRef.current;
			if (!dragAtFinish || dragAtFinish.settling || dragAtFinish.pointerId !== event.pointerId)
				return;
			let current: DragState = dragAtFinish;
			cancelVerticalGroupPickupFrame();
			if (touchLongPressTimer.current !== null) window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = null;
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
						model.getIndexOfTab(current.sourceId) !== null
					) {
						model.setSelection([current.sourceId], current.sourceId, { expandGroups: false });
					}
				}
				if (current.groupHeaderId !== null) {
					scheduleSuppressedGroupToggleReset(current.groupHeaderId);
				}
				announce(messages.tabMoveCanceled);
				setDrag(null);
				return;
			}
			const externalTargetActive =
				current.crossWindowRuntime?.getSnapshot?.().targetActive === true;
			if (current.started) current.crossWindowRuntime?.end('complete');
			// Both tab-strip orientations update the model live while the
			// pointer moves. Release accepts that order instead of submitting
			// the same target a second time.
			if (!current.started && current.pointerType === 'touch' && current.touchLongPressRecognized) {
				// Chromium emits LongTap after LongPress when the pointer is released
				// without entering the drag loop. Tabs open their context menu and
				// group headers open the group editor; this is distinct from a tap.
				if (current.groupHeaderId === null) {
					setMenu({ kind: 'tab', id: current.sourceId, x: event.clientX, y: event.clientY });
				} else {
					suppressGroupToggleRef.current = current.groupHeaderId;
					scheduleSuppressedGroupToggleReset(current.groupHeaderId);
					openGroupEditor(current.groupHeaderId, event.clientX, event.clientY);
				}
			} else if (
				current.groupHeaderId === null &&
				!event.shiftKey &&
				!current.shiftPressedOnPointerDown &&
				!(event.metaKey || event.ctrlKey)
			) {
				actions.execute(
					TAB_STRIP_ACTION_IDS.selectTab,
					{ mode: 'replace', tabId: current.sourceId },
					'pointer',
				);
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
						messages.formatDragCompleteAnnouncement(
							current.label,
							finalIndex + 1,
							liveSnapshot.tabs.length,
						),
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
						model.getIndexOfTab(completionActiveId) !== null
					) {
						// The active group member remains authoritative even while the
						// collapsed group header is its visible projection.
						model.setSelection([completionActiveId], completionActiveId, { expandGroups: false });
					}
				}
				if (!externalTargetActive) {
					const committedArrangement = model.captureDragRestoreState();
					model.restoreCanceledDrag(current.restoreState);
					actions.execute(
						TAB_STRIP_ACTION_IDS.applyArrangement,
						{ state: committedArrangement },
						'pointer',
					);
				}
			}
			if (current.groupHeaderId !== null) {
				scheduleSuppressedGroupToggleReset(current.groupHeaderId);
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
					dragStartAnimationRequired: false,
					dragStartAnimationProgress: 1,
					settling: true,
				};
				setDrag(settlingDrag);
				if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
					finishHorizontalDragSettle();
				} else {
					// Transition completion normally owns cleanup. The fallback
					// covers unchanged bounds and interrupted browser transitions.
					dragSettleTimer.current = window.setTimeout(
						finishHorizontalDragSettle,
						HORIZONTAL_DRAG_SETTLE_FALLBACK_DELAY_MS,
					);
				}
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
			cancelVerticalGroupPickupFrame();
			if (touchLongPressTimer.current !== null) window.clearTimeout(touchLongPressTimer.current);
			touchLongPressTimer.current = null;
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
					model.getIndexOfTab(current.sourceId) !== null
				) {
					model.setSelection([current.sourceId], current.sourceId, { expandGroups: false });
				}
			}
			if (current.groupHeaderId !== null) {
				suppressGroupToggleRef.current = null;
				if (suppressGroupToggleResetTimer.current !== null) {
					window.clearTimeout(suppressGroupToggleResetTimer.current);
					suppressGroupToggleResetTimer.current = null;
				}
			}
			announce(messages.tabMoveCanceled);
			setDrag(null);
		};
		window.addEventListener('keydown', keydown);
		return () => {
			if (repositionDragAfterScrollRef.current === repositionAfterScroll) {
				repositionDragAfterScrollRef.current = () => {};
			}
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', finish);
			window.removeEventListener('pointercancel', finish);
			window.removeEventListener('keydown', keydown);
		};
	}, [
		actions,
		dragPresent,
		horizontal,
		horizontalLayout,
		horizontalTrackRef,
		items,
		messages,
		model,
		openGroupEditor,
		snapshot,
		stripRef,
		verticalCollapsed,
		finishHorizontalDragSettle,
		completeHorizontalAnimationsForDragStart,
		cancelVerticalGroupPickupFrame,
		scheduleSuppressedGroupToggleReset,
	]);

	const onTabKeyDown = (event: React.KeyboardEvent, tabId: string) => {
		if (isTabStripKeyboardContextMenuRequest(event)) {
			event.preventDefault();
			const bounds = event.currentTarget.getBoundingClientRect();
			setMenu({ kind: 'tab', id: tabId, x: bounds.left + bounds.width / 2, y: bounds.bottom });
			return;
		}
		if (
			event.key === 'Enter' &&
			!event.altKey &&
			!event.ctrlKey &&
			!event.metaKey &&
			!event.shiftKey
		) {
			event.preventDefault();
			if (snapshot.activeTabId !== tabId) {
				actions.execute(TAB_STRIP_ACTION_IDS.selectTab, { mode: 'replace', tabId }, 'keyboard');
			}
			return;
		}
		if (event.key === ' ' && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
			event.preventDefault();
			return;
		}
		const navigation = resolveTabStripRovingNavigationCommand(event, horizontal);
		if (navigation !== null) {
			event.preventDefault();
			const target = resolveTabStripRovingNavigation({
				command: navigation,
				currentKey: `tab:${tabId}`,
				items: rovingItems,
			});
			if (target !== null) focusRovingItem(target);
			return;
		}
		const reorder = resolveTabKeyboardReorder({
			key: event.key,
			commandOrControl: event.ctrlKey || event.metaKey,
			shift: event.shiftKey,
			horizontal,
			mac: typeof navigator !== 'undefined' && navigator.platform.startsWith('Mac'),
		});
		if (!reorder) return;
		event.preventDefault();
		if (reorder.moveToEnd) {
			actions.execute(
				TAB_STRIP_ACTION_IDS.moveTab,
				{ direction: reorder.direction < 0 ? 'first' : 'last', tabId },
				'keyboard',
			);
			announce(
				messages.formatTabMoveAnnouncement(
					reorder.direction < 0 ? 'first' : 'last',
					horizontal ? 'horizontal' : 'vertical',
				),
			);
		} else {
			const before = model.getSnapshot();
			const beforeIndex = before.tabs.findIndex((tab) => tab.id === tabId);
			const beforeTab = before.tabs.find((tab) => tab.id === tabId);
			const beforeGroup =
				beforeTab?.group === null || beforeTab?.group === undefined
					? null
					: (before.groups.find((group) => group.id === beforeTab.group) ?? null);
			const tabsInBeforeGroup = beforeGroup
				? before.tabs.filter((tab) => tab.group === beforeGroup.id)
				: [];
			actions.execute(
				TAB_STRIP_ACTION_IDS.moveTab,
				{ direction: reorder.direction < 0 ? 'previous' : 'next', tabId },
				'keyboard',
			);
			const after = model.getSnapshot();
			const afterIndex = after.tabs.findIndex((tab) => tab.id === tabId);
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
					messages.formatTabGroupMembershipAnnouncement(
						afterGroup ? 'added' : 'removed',
						targetGroup,
						groupTabs,
					),
				);
			} else if (beforeIndex !== afterIndex) {
				announce(
					messages.formatTabMoveAnnouncement(
						reorder.direction < 0 ? 'previous' : 'next',
						horizontal ? 'horizontal' : 'vertical',
					),
				);
			}
		}
	};

	const onTabKeyUp = (event: React.KeyboardEvent, tabId: string) => {
		if (event.key !== ' ' || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) {
			return;
		}
		event.preventDefault();
		if (snapshot.activeTabId !== tabId) {
			actions.execute(TAB_STRIP_ACTION_IDS.selectTab, { mode: 'replace', tabId }, 'keyboard');
		}
	};

	const addTab = () => {
		setTabClosingMode(null);
		onNewTabButtonPressed?.();
		actions.createTab({ groupId: null, title: messages.newTabTitle }, 'button');
	};

	const animatedHorizontalBoundsById = new Map(
		tabCountChanged
			? []
			: (animatedHorizontalFrame?.layout.children.map((child) => [child.id, child.bounds]) ?? []),
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

	return {
		actions,
		identityIcons,
		model,
		statusAdornments,
		orientation,
		onOrientationChange,
		verticalCollapsed,
		verticalExpandOnHover,
		showHoverCardPreviews,
		showHoverCardMemoryUsage,
		showGroupHoverCards,
		showVerticalTopControls,
		verticalToolbarHeight,
		verticalCaptionButtonWidth,
		verticalIsExitingExpandOnHover,
		onVerticalCollapsedChange,
		onVerticalTabSearchClick,
		onVerticalExpandOnHoverChange,
		showNewTabButton,
		horizontalFrameCondensed,
		touchUi,
		tabStripDeclutter,
		onTabKeyboardFocusChanged,
		onRovingFocusChanged,
		onRovingFocusExited,
		rovingFocusKey,
		onMoveTabsToNewWindow,
		onMoveGroupToNewWindow,
		onSetTabTitle,
		shortcutLabels,
		ariaLabel: ariaLabel ?? messages.tabStripLabel,
		snapshot,
		verticalTabCountTransitionSuppressed: tabCountChanged || verticalTabCountTransitionSuppressed,
		horizontal,
		stripRef,
		horizontalTrackRef,
		size,
		items,
		horizontalAnimationRuntimeRef,
		menu,
		setMenu,
		hover,
		drag,
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
		repositionDragAfterScroll,
		toggleGroupFromHeader,
		onGroupKeyDown,
		onTabKeyDown,
		onTabKeyUp,
		addTab,
		finishHorizontalDragSettle,
		paintedHorizontalBounds,
		settleTargetLeft,
	} as const;
}
