import { requireDefined } from '../utils/requireDefined';
import { ToggleTabGroupCollapsedStateOrigin } from '../types/tab-strip-types';
import type { TabGroupId } from './tab-types';
import { moveContiguousBlock } from './moveContiguousBlock';
export { ToggleTabGroupCollapsedStateOrigin } from '../types/tab-strip-types';

/**
 * Browser-independent names for Chromium's `tabs::TabAlert` values.
 *
 * The final three values are compatibility aliases from this package's first
 * release; new code should prefer the Chromium-aligned specific values.
 */
export type TabAlertState =
	| 'none'
	| 'audio-playing'
	| 'audio-muting'
	| 'media-recording'
	| 'audio-recording'
	| 'video-recording'
	| 'desktop-capturing'
	| 'tab-capturing'
	| 'pip-playing'
	| 'bluetooth-connected'
	| 'bluetooth-scan-active'
	| 'usb-connected'
	| 'hid-connected'
	| 'serial-connected'
	| 'vr-presenting-in-headset'
	| 'audio-muted'
	| 'recording'
	| 'capturing';
export interface TabData {
	id: string;
	title: string;
	url?: string;
	faviconUrl?: string;
	shouldDisplayFavicon?: boolean;
	thumbnailUrl?: string;
	shouldDisplayUrl?: boolean;
	pinned: boolean;
	group: TabGroupId | null;
	loading?: boolean;
	/** Mirrors tabs::TabData::should_render_loading_title. */
	shouldRenderLoadingTitle?: boolean;
	networkState?: 'none' | 'waiting' | 'loading' | 'error';
	shouldHideThrobber?: boolean;
	crashed?: boolean;
	blocked?: boolean;
	needsAttention?: boolean;
	permissionRequested?: boolean;
	discarded?: boolean;
	discardedMemorySavingsBytes?: number;
	memoryUsageBytes?: number;
	memoryUsageHigh?: boolean;
	alertState?: TabAlertState;
}
export type TabGroupColor =
	| 'grey'
	| 'blue'
	| 'red'
	| 'yellow'
	| 'green'
	| 'pink'
	| 'purple'
	| 'cyan'
	| 'orange';
export interface TabGroupData {
	id: TabGroupId;
	title: string;
	color: TabGroupColor;
	collapsed: boolean;
	attention?: boolean;
}
export interface TabModelInit {
	tabs?: readonly TabData[];
	groups?: readonly TabGroupData[];
	activeTabId?: string | null;
	selectedTabIds?: readonly string[];
	selectionAnchorId?: string | null;
	mostRecentGroupId?: TabGroupId | null;
}
export interface TabModelSnapshot {
	revision: number;
	tabs: readonly Readonly<TabData>[];
	groups: readonly Readonly<TabGroupData>[];
	activeTabId: string | null;
	selectedTabIds: ReadonlySet<string>;
	selectionAnchorId: string | null;
	mostRecentGroupId: TabGroupId | null;
}
export interface TabDragRestoreState {
	tabs: readonly Readonly<{
		id: string;
		group: TabGroupId | null;
	}>[];
	groups: readonly Readonly<TabGroupData>[];
	activeTabId: string | null;
	selectedTabIds: readonly string[];
	selectionAnchorId: string | null;
	mostRecentGroupId: TabGroupId | null;
}
export interface DetachedTabBundle {
	tabs: readonly Readonly<TabData>[];
	groups: readonly Readonly<TabGroupData>[];
	sourceIndices: readonly number[];
	activeTabId: string | null;
	selectedTabIds: readonly string[];
}
export interface DraggedTabsInsertionInput {
	containerStartIndex: number;
	containerEndIndex: number;
	targetIndex: number;
	draggedIndices: readonly number[];
	draggedGroups: readonly (TabGroupId | null)[];
	targetGroup: TabGroupId | null;
	firstSelectedIndex: number;
}
export interface DraggedTabsInsertionResult {
	adjustedTargetIndex: number;
	noOp: boolean;
}
function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(Math.max(value, minimum), maximum);
}

/**
 * Pure port of TabDragHandlerImpl::HandleDraggedTabsIntoPosition's index math.
 */
export function calculateDraggedTabsInsertion(
	input: DraggedTabsInsertionInput,
): DraggedTabsInsertionResult {
	const targetIndex = clamp(input.targetIndex, input.containerStartIndex, input.containerEndIndex);
	let draggedBeforeTarget = 0;
	let allAlreadyInTargetGroup = true;
	input.draggedIndices.forEach((index, dragIndex) => {
		if (input.draggedGroups[dragIndex] !== input.targetGroup) {
			allAlreadyInTargetGroup = false;
		}
		if (index < targetIndex) {
			draggedBeforeTarget += 1;
		}
	});
	const adjustedTargetIndex = Math.max(0, targetIndex - draggedBeforeTarget);
	return {
		adjustedTargetIndex,
		noOp: allAlreadyInTargetGroup && input.firstSelectedIndex === adjustedTargetIndex,
	};
}
type Listener = () => void;
type BeforeTabAddedListener = () => void;

/**
 * Browser-independent model for controller behaviors exposed by the tabs
 * source tree. Mutations preserve Chromium's pinned-prefix and contiguous-group
 * invariants and publish immutable snapshots for React's useSyncExternalStore.
 */
export class TabModel {
	private tabs: TabData[];
	private groups: TabGroupData[];
	private activeTabId: string | null;
	private selectedTabIds: Set<string>;
	private selectionAnchorId: string | null;
	private mostRecentGroupId: TabGroupId | null;
	private revision = 0;
	private nextTabSequence = 1;
	private readonly listeners = new Set<Listener>();
	private readonly beforeTabAddedListeners = new Set<BeforeTabAddedListener>();
	private snapshot: TabModelSnapshot;
	public constructor(init: TabModelInit = {}) {
		this.tabs = (init.tabs ?? []).map((tab) => ({
			...tab,
		}));
		this.groups = (init.groups ?? []).map((group) => ({
			...group,
		}));
		this.activeTabId = init.activeTabId ?? this.tabs[0]?.id ?? null;
		this.selectedTabIds = new Set(init.selectedTabIds ?? []);
		if (this.activeTabId !== null) {
			this.selectedTabIds.add(this.activeTabId);
		}
		this.selectionAnchorId =
			init.selectionAnchorId === undefined ? this.activeTabId : init.selectionAnchorId;
		const activeGroupId = this.tabs.find((tab) => tab.id === this.activeTabId)?.group ?? null;
		this.mostRecentGroupId =
			init.mostRecentGroupId ?? activeGroupId ?? this.groups.at(-1)?.id ?? null;
		this.assertInvariants();
		this.snapshot = this.createSnapshot();
	}
	public getSnapshot = (): TabModelSnapshot => this.snapshot;
	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	/** Observer hook matching TabStripModelObserver::OnTabWillBeAdded. */
	public subscribeBeforeTabAdded(listener: BeforeTabAddedListener): () => void {
		this.beforeTabAddedListeners.add(listener);
		return () => this.beforeTabAddedListeners.delete(listener);
	}
	public getIndexOfTab(tabId: string): number | null {
		const index = this.tabs.findIndex((tab) => tab.id === tabId);
		return index === -1 ? null : index;
	}
	public getGroup(groupId: TabGroupId): Readonly<TabGroupData> | null {
		return this.groups.find((group) => group.id === groupId) ?? null;
	}
	public addTab(
		tab: Partial<Omit<TabData, 'id'>> & Pick<TabData, 'title'>,
		options: {
			id?: string;
			index?: number;
			activate?: boolean;
		} = {},
	): string {
		let id = options.id;
		if (id === undefined) {
			do {
				id = `tab-${this.nextTabSequence++}`;
			} while (this.getIndexOfTab(id) !== null);
		}
		if (this.getIndexOfTab(id) !== null) {
			throw new Error(`Duplicate tab id ${id}.`);
		}
		const newTab: TabData = {
			...tab,
			id,
			title: tab.title,
			pinned: tab.pinned ?? false,
			group: tab.group ?? null,
		};
		if (newTab.group !== null) this.requireGroup(newTab.group);
		if (newTab.pinned && newTab.group !== null) {
			throw new Error('Pinned tabs cannot be added to a group.');
		}
		this.beforeTabAddedListeners.forEach((listener) => listener());
		const firstUnpinned = this.indexOfFirstNonPinnedTab();
		const groupRange = newTab.group === null ? null : this.groupRange(newTab.group);
		const defaultIndex = newTab.pinned ? firstUnpinned : (groupRange?.end ?? this.tabs.length);
		let index = clamp(options.index ?? defaultIndex, 0, this.tabs.length);
		index = newTab.pinned ? Math.min(index, firstUnpinned) : Math.max(index, firstUnpinned);
		this.tabs.splice(index, 0, newTab);
		if (options.activate ?? true) {
			this.activeTabId = id;
			this.selectedTabIds = new Set([id]);
			this.selectionAnchorId = id;
			if (newTab.group !== null) this.requireGroup(newTab.group).collapsed = false;
		}
		this.commit();
		return id;
	}
	public closeTab(tabId: string): boolean {
		const index = this.getIndexOfTab(tabId);
		if (index === null) {
			return false;
		}
		this.closeTabs([tabId]);
		return true;
	}
	public closeTabs(tabIds: readonly string[]): boolean {
		const idsToClose = new Set(tabIds);
		const firstClosedIndex = this.tabs.findIndex((tab) => idsToClose.has(tab.id));
		if (firstClosedIndex === -1) return false;
		const activeWasClosed = this.activeTabId !== null && idsToClose.has(this.activeTabId);
		this.tabs = this.tabs.filter((tab) => !idsToClose.has(tab.id));
		this.selectedTabIds = new Set([...this.selectedTabIds].filter((id) => !idsToClose.has(id)));
		if (
			activeWasClosed ||
			(this.activeTabId !== null && !this.tabs.some((tab) => tab.id === this.activeTabId))
		) {
			const replacement = this.tabs[Math.min(firstClosedIndex, this.tabs.length - 1)] ?? null;
			this.activeTabId = replacement?.id ?? null;
		}
		if (this.activeTabId !== null) this.selectedTabIds.add(this.activeTabId);
		if (this.selectionAnchorId !== null && idsToClose.has(this.selectionAnchorId)) {
			this.selectionAnchorId = this.activeTabId;
		}
		this.removeEmptyGroups();
		this.commit();
		return true;
	}
	public closeSelectedTabs(): void {
		const selected = this.tabs
			.filter((tab) => this.selectedTabIds.has(tab.id))
			.map((tab) => tab.id);
		if (selected.length === 0) {
			return;
		}
		this.closeTabs(selected);
	}

	/** Removes tabs without losing the metadata required by cross-window insertion. */
	public detachTabsForTransfer(tabIds: readonly string[]): DetachedTabBundle | null {
		const ids = new Set(tabIds);
		const sourceIndices = this.tabs
			.map((tab, index) => (ids.has(tab.id) ? index : -1))
			.filter((index) => index !== -1);
		if (sourceIndices.length === 0) return null;
		const tabs = sourceIndices.map((index) =>
			Object.freeze({
				...requireDefined(this.tabs[index]),
			}),
		);
		const groupIds = new Set(tabs.flatMap((tab) => (tab.group === null ? [] : [tab.group])));
		const groups = this.groups
			.filter((group) => groupIds.has(group.id))
			.map((group) =>
				Object.freeze({
					...group,
				}),
			);
		const bundle: DetachedTabBundle = Object.freeze({
			tabs: Object.freeze(tabs),
			groups: Object.freeze(groups),
			sourceIndices: Object.freeze(sourceIndices),
			activeTabId: this.activeTabId !== null && ids.has(this.activeTabId) ? this.activeTabId : null,
			selectedTabIds: Object.freeze([...this.selectedTabIds].filter((id) => ids.has(id))),
		});
		const firstDetachedIndex = requireDefined(sourceIndices[0]);
		const activeWasDetached = this.activeTabId !== null && ids.has(this.activeTabId);
		this.tabs = this.tabs.filter((tab) => !ids.has(tab.id));
		this.selectedTabIds = new Set([...this.selectedTabIds].filter((id) => !ids.has(id)));
		if (
			activeWasDetached ||
			(this.activeTabId !== null && !this.tabs.some((tab) => tab.id === this.activeTabId))
		) {
			const replacement = this.tabs[Math.min(firstDetachedIndex, this.tabs.length - 1)] ?? null;
			this.activeTabId = replacement?.id ?? null;
		}
		if (this.activeTabId !== null) this.selectedTabIds.add(this.activeTabId);
		if (this.selectionAnchorId !== null && ids.has(this.selectionAnchorId)) {
			this.selectionAnchorId = this.activeTabId;
		}
		this.removeEmptyGroups();
		this.commit();
		return bundle;
	}

	/** Inserts a detached bundle, preserving IDs and group visual data. */
	public insertTransferredTabs(
		bundle: Readonly<DetachedTabBundle>,
		options: {
			index?: number;
			targetGroup?: TabGroupId | null;
			activateTabId?: string | null;
		} = {},
	): boolean {
		if (bundle.tabs.length === 0 || !this.canInsertTransferredTabs(bundle, options.targetGroup))
			return false;
		const moving = bundle.tabs.map((tab) => ({
			...tab,
		}));
		if (options.targetGroup !== undefined)
			moving.forEach((tab) => {
				tab.group = requireDefined(options.targetGroup);
			});
		if (options.targetGroup === undefined) {
			const existingGroups = new Set(this.groups.map((group) => group.id));
			bundle.groups.forEach((group) => {
				if (!existingGroups.has(group.id)) {
					this.groups.push({
						...group,
					});
					existingGroups.add(group.id);
				}
			});
		}
		const pinned = requireDefined(moving[0]).pinned;
		const boundary = this.indexOfFirstNonPinnedTab();
		let index = clamp(options.index ?? (pinned ? boundary : this.tabs.length), 0, this.tabs.length);
		index = pinned ? Math.min(index, boundary) : Math.max(index, boundary);
		if (options.targetGroup !== undefined && options.targetGroup !== null) {
			const range = this.groupRange(options.targetGroup);
			if (range) index = clamp(index, range.start, range.end);
		}
		this.tabs.splice(index, 0, ...moving);
		const requestedActive =
			options.activateTabId ?? bundle.activeTabId ?? requireDefined(moving[0]).id;
		const active = moving.find((tab) => tab.id === requestedActive) ?? requireDefined(moving[0]);
		this.activeTabId = active.id;
		this.selectedTabIds = new Set(
			bundle.selectedTabIds.filter((id) => moving.some((tab) => tab.id === id)),
		);
		this.selectedTabIds.add(active.id);
		this.selectionAnchorId = active.id;
		this.commit();
		return true;
	}

	/** Validates the target first, then transfers the bundle between two models. */
	public transferTabsTo(
		target: TabModel,
		tabIds: readonly string[],
		options: {
			index?: number;
			targetGroup?: TabGroupId | null;
			activateTabId?: string | null;
		} = {},
	): boolean {
		if (target === this) {
			const source = this.tabs.find((tab) => tabIds.includes(tab.id));
			if (!source) return false;
			const boundary = this.indexOfFirstNonPinnedTab();
			return this.moveTabsByDrag(
				tabIds,
				options.index ?? this.tabs.length,
				source.pinned ? 0 : boundary,
				source.pinned ? boundary : this.tabs.length,
				options.targetGroup === undefined ? source.group : options.targetGroup,
			);
		}
		const tabs = this.tabs.filter((tab) => tabIds.includes(tab.id));
		const groups = this.groups.filter((group) => tabs.some((tab) => tab.group === group.id));
		const preview: DetachedTabBundle = {
			tabs,
			groups,
			sourceIndices: tabs.map((tab) => this.requireTabIndex(tab.id)),
			activeTabId:
				this.activeTabId !== null && tabIds.includes(this.activeTabId) ? this.activeTabId : null,
			selectedTabIds: [...this.selectedTabIds].filter((id) => tabIds.includes(id)),
		};
		if (!target.canInsertTransferredTabs(preview, options.targetGroup)) return false;
		const bundle = this.detachTabsForTransfer(tabIds);
		return bundle ? target.insertTransferredTabs(bundle, options) : false;
	}
	public closeOtherTabs(tabId: string): void {
		this.requireTabIndex(tabId);
		this.tabs = this.tabs.filter((tab) => tab.id === tabId || tab.pinned);
		this.activeTabId = tabId;
		this.selectedTabIds = new Set([tabId]);
		this.selectionAnchorId = tabId;
		this.removeEmptyGroups();
		this.commit();
	}
	public closeTabsToRight(tabId: string): void {
		const index = this.requireTabIndex(tabId);
		const keep = this.tabs.slice(0, index + 1);
		const activeStillExists = keep.some((tab) => tab.id === this.activeTabId);
		this.tabs = keep;
		if (!activeStillExists) {
			this.activeTabId = tabId;
			this.selectedTabIds = new Set([tabId]);
			this.selectionAnchorId = tabId;
		} else {
			this.selectedTabIds = new Set(
				[...this.selectedTabIds].filter((selectedId) => keep.some((tab) => tab.id === selectedId)),
			);
			if (
				this.selectionAnchorId !== null &&
				!keep.some((tab) => tab.id === this.selectionAnchorId)
			) {
				this.selectionAnchorId = this.activeTabId;
			}
		}
		this.removeEmptyGroups();
		this.commit();
	}
	public duplicateTab(tabId: string): string {
		const index = this.requireTabIndex(tabId);
		const source = requireDefined(this.tabs[index]);
		return this.addTab(
			{
				...source,
				title: source.title,
			},
			{
				index: index + 1,
				activate: false,
			},
		);
	}
	public updateTab(tabId: string, update: Partial<Omit<TabData, 'id' | 'pinned' | 'group'>>): void {
		Object.assign(requireDefined(this.tabs[this.requireTabIndex(tabId)]), update);
		this.commit();
	}
	public selectTab(tabId: string): void {
		this.requireTabIndex(tabId);
		this.activeTabId = tabId;
		this.selectedTabIds = new Set([tabId]);
		this.selectionAnchorId = tabId;
		this.expandGroupForTab(tabId);
		this.commit();
	}
	public toggleSelected(tabId: string): void {
		this.requireTabIndex(tabId);
		if (this.selectedTabIds.has(tabId)) {
			if (this.selectedTabIds.size === 1 || this.activeTabId === tabId) {
				return;
			}
			this.selectedTabIds.delete(tabId);
		} else {
			this.selectedTabIds.add(tabId);
			this.activeTabId = tabId;
			this.selectionAnchorId ??= tabId;
		}
		this.commit();
	}
	public extendSelectionTo(tabId: string): void {
		const range = this.selectionRangeTo(tabId);
		this.selectedTabIds = new Set(range.map((tab) => tab.id));
		this.activeTabId = tabId;
		this.expandGroupsForTabs(range);
		this.commit();
	}
	public addSelectionFromAnchorTo(tabId: string): void {
		const range = this.selectionRangeTo(tabId);
		range.forEach((tab) => this.selectedTabIds.add(tab.id));
		this.activeTabId = tabId;
		this.expandGroupsForTabs(range);
		this.commit();
	}
	public setSelection(
		tabIds: readonly string[],
		activeTabId: string,
		options: Readonly<{
			expandGroups?: boolean;
		}> = {},
	): void {
		this.requireTabIndex(activeTabId);
		const knownIds = new Set(this.tabs.map((tab) => tab.id));
		this.selectedTabIds = new Set(tabIds.filter((id) => knownIds.has(id)));
		this.selectedTabIds.add(activeTabId);
		this.activeTabId = activeTabId;
		this.selectionAnchorId = activeTabId;
		if (options.expandGroups ?? true) {
			this.expandGroupsForTabs(this.tabs.filter((tab) => this.selectedTabIds.has(tab.id)));
		}
		this.commit();
	}

	/** Port of TabStripCollectionController::ShiftTabRelative. */
	public shiftTabRelative(tabId: string, offset: -1 | 1): void {
		const startIndex = this.requireTabIndex(tabId);
		let targetIndex = startIndex + offset;
		const movingTab = requireDefined(this.tabs[startIndex]);
		const oldGroup = movingTab.group;
		const target = this.tabs[targetIndex];
		if (!target || target.pinned !== movingTab.pinned) {
			if (oldGroup !== null) {
				movingTab.group = null;
				this.removeEmptyGroups();
				this.commit();
			}
			return;
		}
		const targetGroup = target.group;
		if (oldGroup !== targetGroup) {
			if (oldGroup !== null) {
				movingTab.group = null;
				this.removeEmptyGroups();
				this.commit();
				return;
			}
			if (targetGroup !== null) {
				const group = this.requireGroup(targetGroup);
				if (group.collapsed) {
					let candidateIndex = targetIndex + offset;
					while (this.tabs[candidateIndex]?.group === targetGroup) {
						candidateIndex += offset;
					}
					if (this.tabs[candidateIndex]) targetIndex = candidateIndex - offset;
					else targetIndex = offset < 0 ? 0 : this.tabs.length - 1;
				} else {
					movingTab.group = targetGroup;
					this.commit();
					return;
				}
			}
		}
		this.moveOne(startIndex, targetIndex);
		this.commit();
	}

	/** Port of MoveTabFirst. Group membership is always removed. */
	public moveTabFirst(tabId: string): void {
		const startIndex = this.requireTabIndex(tabId);
		const tab = requireDefined(this.tabs[startIndex]);
		const targetIndex = tab.pinned ? 0 : this.indexOfFirstNonPinnedTab();
		this.moveOne(startIndex, targetIndex);
		tab.group = null;
		this.removeEmptyGroups();
		this.commit();
	}

	/** Port of MoveTabLast. Pinned tabs stop at the pinned boundary. */
	public moveTabLast(tabId: string): void {
		const startIndex = this.requireTabIndex(tabId);
		const tab = requireDefined(this.tabs[startIndex]);
		const targetIndex = tab.pinned ? this.indexOfFirstNonPinnedTab() - 1 : this.tabs.length - 1;
		this.moveOne(startIndex, targetIndex);
		tab.group = null;
		this.removeEmptyGroups();
		this.commit();
	}

	/** Port of ShiftGroupRelative. */
	public shiftGroupRelative(groupId: TabGroupId, offset: -1 | 1): void {
		const range = this.groupRange(groupId);
		if (range === null) {
			return;
		}
		const skippedIndex = offset === 1 ? range.end : range.start - 1;
		const skippedTab = this.tabs[skippedIndex];
		if (!skippedTab || skippedTab.pinned) {
			return;
		}
		const skippedRange = skippedTab.group === null ? null : this.groupRange(skippedTab.group);
		const skippedCount = skippedRange === null ? 1 : skippedRange.end - skippedRange.start;
		this.moveGroupTo(groupId, range.start + offset * skippedCount);
	}
	public moveGroupTo(groupId: TabGroupId, targetIndex: number): void {
		const groupTabs = this.tabs.filter((tab) => tab.group === groupId);
		if (groupTabs.length === 0) {
			return;
		}
		this.tabs = this.tabs.filter((tab) => tab.group !== groupId);
		const insertionIndex = clamp(targetIndex, this.indexOfFirstNonPinnedTab(), this.tabs.length);
		this.tabs.splice(insertionIndex, 0, ...groupTabs);
		this.commit();
	}

	/**
	 * Drag-position delegates report an index in the current, pre-removal tab
	 * strip. TabStripModel::MoveGroupTo instead consumes an index after the
	 * moving group's tabs have been removed. Chromium performs this conversion
	 * in CalculateInsertionIndex before calling MoveGroupTo.
	 */
	public moveGroupByDrag(groupId: TabGroupId, preRemovalTargetIndex: number): void {
		const range = this.groupRange(groupId);
		if (range === null) return;
		this.tabs = [
			...moveContiguousBlock(this.tabs, {
				start: range.start,
				length: range.end - range.start,
				preRemovalTargetIndex,
			}),
		];
		this.commit();
	}
	public setTabsPinned(tabIds: readonly string[], pinned: boolean): void {
		const ids = new Set(tabIds);
		const moving = this.tabs.filter((tab) => ids.has(tab.id) && tab.pinned !== pinned);
		if (moving.length === 0) {
			return;
		}
		const movingIds = new Set(moving.map((tab) => tab.id));
		this.tabs = this.tabs.filter((tab) => !movingIds.has(tab.id));
		moving.forEach((tab) => {
			tab.pinned = pinned;
			if (pinned) {
				tab.group = null;
			}
		});
		const insertionIndex = this.indexOfFirstNonPinnedTab();
		this.tabs.splice(insertionIndex, 0, ...moving);
		this.removeEmptyGroups();
		this.commit();
	}
	public createGroup(group: TabGroupData, tabIds: readonly string[]): void {
		if (this.getGroup(group.id)) {
			throw new Error(`Duplicate group id ${group.id}.`);
		}
		const idSet = new Set(tabIds);
		if (!this.tabs.some((tab) => idSet.has(tab.id))) {
			throw new Error('A tab group must contain at least one known tab.');
		}
		this.groups.push({
			...group,
		});
		this.moveTabsToGroup(tabIds, group.id);
		// AddToNewGroupImpl deselects every newly grouped tab except the active
		// one; selections outside the new group are retained.
		let selectionChanged = false;
		tabIds.forEach((tabId) => {
			if (tabId !== this.activeTabId && this.selectedTabIds.delete(tabId)) {
				selectionChanged = true;
			}
		});
		if (selectionChanged) this.commit();
	}
	public moveTabsToGroup(tabIds: readonly string[], groupId: TabGroupId | null): void {
		if (groupId !== null) {
			this.requireGroup(groupId);
		}
		const idSet = new Set(tabIds);
		const firstMovingIndex = this.tabs.findIndex(
			(tab) => idSet.has(tab.id) && tab.group !== groupId,
		);
		const targetGroupWasEmpty = groupId !== null && this.groupRange(groupId) === null;
		const moving = this.tabs.filter((tab) => idSet.has(tab.id) && tab.group !== groupId);
		if (moving.length === 0) {
			return;
		}
		const movingIds = new Set(moving.map((tab) => tab.id));
		const movingIndices = this.tabs.flatMap((tab, index) => (movingIds.has(tab.id) ? [index] : []));
		const movingIsContiguous = movingIndices.every(
			(index, offset) => index === firstMovingIndex + offset,
		);
		const sourceGroupIds = new Set(
			moving.flatMap((tab) => (tab.group === null ? [] : [tab.group])),
		);
		const removesWholeSourceGroups =
			groupId === null &&
			movingIsContiguous &&
			moving.every((tab) => tab.group !== null) &&
			this.tabs.every(
				(tab) => tab.group === null || !sourceGroupIds.has(tab.group) || movingIds.has(tab.id),
			);
		this.tabs = this.tabs.filter((tab) => !movingIds.has(tab.id));
		moving.forEach((tab) => {
			tab.group = groupId;
			// TabStripModel::MoveTabsAndSetPropertiesImpl always passes pinned=false
			// for grouping, so selecting a pinned tab for a group unpins it.
			if (groupId !== null) tab.pinned = false;
		});
		let insertionIndex = this.tabs.length;
		if (groupId !== null) {
			const range = this.groupRange(groupId);
			insertionIndex =
				range?.end ??
				(targetGroupWasEmpty
					? clamp(firstMovingIndex, this.indexOfFirstNonPinnedTab(), this.tabs.length)
					: this.tabs.length);
		} else if (removesWholeSourceGroups) {
			insertionIndex = clamp(firstMovingIndex, this.indexOfFirstNonPinnedTab(), this.tabs.length);
		}
		// A fresh group made from a middle member must not split its surviving source group.
		const previousGroup = this.tabs[insertionIndex - 1]?.group;
		if (
			previousGroup !== undefined &&
			previousGroup !== null &&
			this.tabs[insertionIndex]?.group === previousGroup
		) {
			const survivingRange = this.groupRange(previousGroup);
			if (survivingRange !== null) insertionIndex = survivingRange.end;
		}
		this.tabs.splice(insertionIndex, 0, ...moving);
		if (groupId !== null && moving.some((tab) => tab.id === this.activeTabId)) {
			this.requireGroup(groupId).collapsed = false;
		}
		this.removeEmptyGroups();
		this.commit();
	}
	public toggleGroupCollapsed(
		groupId: TabGroupId,
		_origin: ToggleTabGroupCollapsedStateOrigin = ToggleTabGroupCollapsedStateOrigin.Mouse,
	): void {
		const group = this.requireGroup(groupId);
		if (group.collapsed) {
			group.collapsed = false;
			this.commit();
			return;
		}
		const activeTab = this.tabs.find((tab) => tab.id === this.activeTabId) ?? null;
		if (activeTab?.group === groupId) {
			// A collapsed group header is the visible projection of its active
			// member. Keep the active identity in the group and reduce selection to
			// that member; expanding the group reveals the same active tab again.
			this.selectedTabIds = new Set([activeTab.id]);
			this.selectionAnchorId = activeTab.id;
		} else if (activeTab) {
			this.selectedTabIds = new Set([activeTab.id]);
			this.selectionAnchorId = activeTab.id;
		}
		group.collapsed = true;
		this.commit();
	}
	public updateGroup(groupId: TabGroupId, update: Partial<Omit<TabGroupData, 'id'>>): void {
		Object.assign(this.requireGroup(groupId), update);
		this.commit();
	}
	public ungroup(groupId: TabGroupId): void {
		let changed = false;
		this.tabs.forEach((tab) => {
			if (tab.group === groupId) {
				tab.group = null;
				changed = true;
			}
		});
		if (changed) {
			this.removeEmptyGroups();
			this.commit();
		}
	}
	public closeGroup(groupId: TabGroupId): void {
		const tabIds = this.tabs.filter((tab) => tab.group === groupId).map((tab) => tab.id);
		if (tabIds.length === 0) {
			return;
		}
		const closing = new Set(tabIds);
		const activeIndex = this.activeTabId === null ? 0 : (this.getIndexOfTab(this.activeTabId) ?? 0);
		this.tabs = this.tabs.filter((tab) => !closing.has(tab.id));
		if (this.activeTabId !== null && closing.has(this.activeTabId)) {
			const replacement = this.tabs[Math.min(activeIndex, this.tabs.length - 1)] ?? null;
			this.activeTabId = replacement?.id ?? null;
			this.selectedTabIds = new Set(replacement ? [replacement.id] : []);
			this.selectionAnchorId = this.activeTabId;
		} else {
			tabIds.forEach((id) => this.selectedTabIds.delete(id));
			if (this.selectionAnchorId !== null && closing.has(this.selectionAnchorId)) {
				this.selectionAnchorId = this.activeTabId;
			}
		}
		this.removeEmptyGroups();
		this.commit();
	}

	/**
	 * Applies the final model mutation used by HandleDraggedTabsIntoPosition.
	 * `targetIndex` is the pre-removal container/target index from the view tree.
	 */
	public moveSelectedTabsByDrag(
		targetIndex: number,
		containerStartIndex: number,
		containerEndIndex: number,
		targetGroup: TabGroupId | null,
	): boolean {
		return this.moveTabsByDrag(
			[...this.selectedTabIds],
			targetIndex,
			containerStartIndex,
			containerEndIndex,
			targetGroup,
		);
	}

	/** Captures the model pieces Chromium restores when a drag is canceled. */
	public captureDragRestoreState(): TabDragRestoreState {
		return {
			tabs: this.tabs.map((tab) =>
				Object.freeze({
					id: tab.id,
					group: tab.group,
				}),
			),
			groups: this.groups.map((group) =>
				Object.freeze({
					...group,
				}),
			),
			activeTabId: this.activeTabId,
			selectedTabIds: [...this.selectedTabIds],
			selectionAnchorId: this.selectionAnchorId,
			mostRecentGroupId: this.mostRecentGroupId,
		};
	}

	/** Restores source indices and group membership after a canceled drag. */
	public restoreCanceledDrag(state: TabDragRestoreState): void {
		const currentById = new Map(this.tabs.map((tab) => [tab.id, tab]));
		const restoredIds = new Set<string>();
		const restored: TabData[] = [];
		const knownGroups = new Set(state.groups.map((group) => group.id));
		for (const source of state.tabs) {
			const tab = currentById.get(source.id);
			if (!tab) continue;
			tab.group =
				!tab.pinned && source.group !== null && knownGroups.has(source.group) ? source.group : null;
			restored.push(tab);
			restoredIds.add(tab.id);
		}
		restored.push(...this.tabs.filter((tab) => !restoredIds.has(tab.id)));
		this.tabs = restored;
		this.groups = state.groups.map((group) => ({
			...group,
		}));
		this.removeEmptyGroups();
		const existingIds = new Set(this.tabs.map((tab) => tab.id));
		this.activeTabId =
			state.activeTabId !== null && existingIds.has(state.activeTabId)
				? state.activeTabId
				: (this.tabs[0]?.id ?? null);
		this.selectedTabIds = new Set(state.selectedTabIds.filter((id) => existingIds.has(id)));
		if (this.activeTabId !== null) this.selectedTabIds.add(this.activeTabId);
		this.selectionAnchorId =
			state.selectionAnchorId !== null && existingIds.has(state.selectionAnchorId)
				? state.selectionAnchorId
				: this.activeTabId;
		this.mostRecentGroupId =
			state.mostRecentGroupId !== null &&
			this.groups.some((group) => group.id === state.mostRecentGroupId)
				? state.mostRecentGroupId
				: null;
		this.commit();
	}
	public moveTabsByDrag(
		tabIds: readonly string[],
		targetIndex: number,
		containerStartIndex: number,
		containerEndIndex: number,
		targetGroup: TabGroupId | null,
		options: Readonly<{
			coalesceNonconsecutive?: boolean;
		}> = {},
	): boolean {
		const draggedIds = new Set(tabIds);
		const draggedIndices = this.tabs
			.map((tab, index) => (draggedIds.has(tab.id) ? index : -1))
			.filter((index) => index !== -1);
		if (draggedIndices.length === 0) {
			return false;
		}
		const result = calculateDraggedTabsInsertion({
			containerStartIndex,
			containerEndIndex,
			targetIndex,
			draggedIndices,
			draggedGroups: draggedIndices.map((index) => requireDefined(this.tabs[index]).group),
			targetGroup,
			firstSelectedIndex: requireDefined(draggedIndices[0]),
		});
		const tabsAreConsecutive = draggedIndices.every(
			(index, position) =>
				position === 0 || index === requireDefined(draggedIndices[position - 1]) + 1,
		);
		if (result.noOp && !(options.coalesceNonconsecutive && !tabsAreConsecutive)) {
			return false;
		}
		if (targetGroup !== null) this.requireGroup(targetGroup);
		// TabStripModel::MoveTabsToIndexImpl retains GROUP collection types. In
		// the flat DOM model that means a completely selected group
		// keeps its identity while a partial selection takes the calculated
		// destination group. This is also what allows a mixed multi-selection to
		// carry whole groups (and their headers) without dissolving them.
		const fullyDraggedGroups = new Set(
			this.groups
				.map((group) => group.id)
				.filter((groupId) => {
					const members = this.tabs.filter((tab) => tab.group === groupId);
					return members.length > 0 && members.every((tab) => draggedIds.has(tab.id));
				}),
		);
		const moving = this.tabs
			.filter((tab) => draggedIds.has(tab.id))
			.map((tab) =>
				Object.assign(tab, {
					group: tab.group !== null && fullyDraggedGroups.has(tab.group) ? tab.group : targetGroup,
				}),
			);
		if (targetGroup !== null && moving.some((tab) => tab.pinned)) {
			throw new Error('Pinned tabs cannot be added to a group.');
		}
		// Build the prospective order without mutating live tab objects. A group
		// header drag can point between two tabs of another group; Chromium snaps
		// that drop to the group's leading/trailing boundary instead of splitting
		// the untouched group.
		const remaining = this.tabs
			.filter((tab) => !draggedIds.has(tab.id))
			.map((tab) => ({
				...tab,
			}));
		let insertionIndex = clamp(result.adjustedTargetIndex, 0, remaining.length);
		if (targetGroup !== null) {
			const targetStart = remaining.findIndex((tab) => tab.group === targetGroup);
			if (targetStart !== -1) {
				let targetEnd = targetStart + 1;
				while (remaining[targetEnd]?.group === targetGroup) targetEnd += 1;
				insertionIndex = clamp(insertionIndex, targetStart, targetEnd);
			}
		}
		const leftGroup = remaining[insertionIndex - 1]?.group ?? null;
		const rightGroup = remaining[insertionIndex]?.group ?? null;
		if (leftGroup !== null && leftGroup === rightGroup && leftGroup !== targetGroup) {
			let groupStart = insertionIndex - 1;
			while (remaining[groupStart - 1]?.group === leftGroup) groupStart -= 1;
			let groupEnd = insertionIndex;
			while (remaining[groupEnd]?.group === leftGroup) groupEnd += 1;
			insertionIndex = targetIndex <= requireDefined(draggedIndices[0]) ? groupStart : groupEnd;
		}
		remaining.splice(insertionIndex, 0, ...moving);
		this.tabs = remaining;
		this.removeEmptyGroups();
		this.commit();
		return true;
	}
	private requireTabIndex(tabId: string): number {
		const index = this.getIndexOfTab(tabId);
		if (index === null) {
			throw new Error(`Unknown tab ${tabId}.`);
		}
		return index;
	}
	private canInsertTransferredTabs(
		bundle: Readonly<DetachedTabBundle>,
		targetGroup: TabGroupId | null | undefined,
	): boolean {
		const existingIds = new Set(this.tabs.map((tab) => tab.id));
		if (bundle.tabs.some((tab) => existingIds.has(tab.id))) return false;
		const pinnedStates = new Set(bundle.tabs.map((tab) => tab.pinned));
		if (pinnedStates.size !== 1) return false;
		if (
			targetGroup !== undefined &&
			targetGroup !== null &&
			!this.groups.some((group) => group.id === targetGroup)
		)
			return false;
		if (targetGroup !== undefined && targetGroup !== null && bundle.tabs.some((tab) => tab.pinned))
			return false;
		return true;
	}
	private requireGroup(groupId: TabGroupId): TabGroupData {
		const group = this.groups.find((candidate) => candidate.id === groupId);
		if (!group) {
			throw new Error(`Unknown group ${groupId}.`);
		}
		return group;
	}
	private indexOfFirstNonPinnedTab(): number {
		const index = this.tabs.findIndex((tab) => !tab.pinned);
		return index === -1 ? this.tabs.length : index;
	}
	private moveOne(from: number, to: number): void {
		if (from === to) {
			return;
		}
		const [tab] = this.tabs.splice(from, 1);
		if (!tab) {
			throw new RangeError(`No tab at index ${from}.`);
		}
		this.tabs.splice(clamp(to, 0, this.tabs.length), 0, tab);
	}
	private groupRange(groupId: TabGroupId): {
		start: number;
		end: number;
	} | null {
		const start = this.tabs.findIndex((tab) => tab.group === groupId);
		if (start === -1) {
			return null;
		}
		let end = start + 1;
		while (this.tabs[end]?.group === groupId) {
			end += 1;
		}
		return {
			start,
			end,
		};
	}
	private selectionRangeTo(tabId: string): TabData[] {
		const targetIndex = this.requireTabIndex(tabId);
		const anchorIndex =
			this.selectionAnchorId === null
				? targetIndex
				: (this.getIndexOfTab(this.selectionAnchorId) ?? targetIndex);
		return this.tabs.slice(
			Math.min(anchorIndex, targetIndex),
			Math.max(anchorIndex, targetIndex) + 1,
		);
	}
	private expandGroupForTab(tabId: string): void {
		const tab = requireDefined(this.tabs[this.requireTabIndex(tabId)]);
		if (tab.group !== null) {
			this.requireGroup(tab.group).collapsed = false;
		}
	}
	private expandGroupsForTabs(tabs: readonly TabData[]): void {
		const ids = new Set(tabs.map((tab) => tab.group).filter((id): id is string => id !== null));
		ids.forEach((id) => {
			this.requireGroup(id).collapsed = false;
		});
	}
	private removeEmptyGroups(): void {
		const usedGroups = new Set(
			this.tabs.map((tab) => tab.group).filter((id): id is string => id !== null),
		);
		this.groups = this.groups.filter((group) => usedGroups.has(group.id));
	}
	private assertInvariants(): void {
		const ids = new Set<string>();
		const groupIds = new Set<TabGroupId>();
		for (const group of this.groups) {
			if (groupIds.has(group.id)) {
				throw new Error(`Duplicate group id ${group.id}.`);
			}
			groupIds.add(group.id);
		}
		let sawUnpinned = false;
		const closedGroups = new Set<TabGroupId>();
		let previousGroup: TabGroupId | null = null;
		for (const tab of this.tabs) {
			if (ids.has(tab.id)) {
				throw new Error(`Duplicate tab id ${tab.id}.`);
			}
			ids.add(tab.id);
			if (!tab.pinned) {
				sawUnpinned = true;
			} else if (sawUnpinned) {
				throw new Error('Pinned tabs must form a contiguous prefix.');
			}
			if (tab.pinned && tab.group !== null) {
				throw new Error('Pinned tabs cannot be grouped.');
			}
			if (previousGroup !== tab.group && previousGroup !== null) {
				closedGroups.add(previousGroup);
			}
			if (tab.group !== null && closedGroups.has(tab.group)) {
				throw new Error(`Group ${tab.group} must be contiguous.`);
			}
			previousGroup = tab.group;
		}
		for (const tab of this.tabs) {
			if (tab.group !== null && !this.groups.some((group) => group.id === tab.group)) {
				throw new Error(`Tab ${tab.id} references unknown group ${tab.group}.`);
			}
		}
		if (this.activeTabId !== null && !ids.has(this.activeTabId)) {
			throw new Error(`Unknown active tab ${this.activeTabId}.`);
		}
		if (this.tabs.length > 0 && this.activeTabId === null) {
			throw new Error('A non-empty tab model must have an active tab.');
		}
		if (this.tabs.length === 0 && this.activeTabId !== null) {
			throw new Error('An empty tab model cannot have an active tab.');
		}
		for (const selectedTabId of this.selectedTabIds) {
			if (!ids.has(selectedTabId)) {
				throw new Error(`Unknown selected tab ${selectedTabId}.`);
			}
		}
		if (this.activeTabId !== null && !this.selectedTabIds.has(this.activeTabId)) {
			throw new Error(`Active tab ${this.activeTabId} must be selected.`);
		}
		if (this.selectionAnchorId !== null && !ids.has(this.selectionAnchorId)) {
			throw new Error(`Unknown selection anchor ${this.selectionAnchorId}.`);
		}
		if (
			this.mostRecentGroupId !== null &&
			!this.groups.some((group) => group.id === this.mostRecentGroupId)
		) {
			throw new Error(`Unknown most-recent group ${this.mostRecentGroupId}.`);
		}
	}
	private createSnapshot(): TabModelSnapshot {
		return {
			revision: this.revision,
			tabs: this.tabs.map((tab) =>
				Object.freeze({
					...tab,
				}),
			),
			groups: this.groups.map((group) =>
				Object.freeze({
					...group,
				}),
			),
			activeTabId: this.activeTabId,
			selectedTabIds: new Set(this.selectedTabIds),
			selectionAnchorId: this.selectionAnchorId,
			mostRecentGroupId: this.mostRecentGroupId,
		};
	}
	private commit(): void {
		const activeGroupId = this.tabs.find((tab) => tab.id === this.activeTabId)?.group ?? null;
		if (activeGroupId !== null) {
			this.mostRecentGroupId = activeGroupId;
		} else if (
			this.mostRecentGroupId !== null &&
			!this.groups.some((group) => group.id === this.mostRecentGroupId)
		) {
			this.mostRecentGroupId = this.groups.at(-1)?.id ?? null;
		}
		this.assertInvariants();
		this.revision += 1;
		this.snapshot = this.createSnapshot();
		this.listeners.forEach((listener) => listener());
	}
}
