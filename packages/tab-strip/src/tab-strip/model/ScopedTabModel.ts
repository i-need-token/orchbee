import type { ToggleTabGroupCollapsedStateOrigin } from '../types/tab-strip-types';
import {
	TabModel,
	type DetachedTabBundle,
	type TabData,
	type TabDragRestoreState,
	type TabGroupData,
	type TabModelSnapshot,
} from './tab-model';
import type { TabGroupId } from './tab-types';

type Listener = () => void;

export interface ScopedTabModelOptions {
	readonly onTabCreated?: ((tabId: string) => void) | undefined;
	readonly visibleTabIds?: readonly string[] | undefined;
}

/**
 * A live window-local projection over a shared TabModel.
 *
 * The source remains the only mutation authority. This facade filters reads,
 * translates view-relative indices back into source indices, and prevents
 * window-local commands from accidentally targeting tabs owned by another
 * native window.
 */
export class ScopedTabModel extends TabModel {
	private readonly source: TabModel;
	private readonly onTabCreated: ((tabId: string) => void) | undefined;
	private readonly scopedListeners = new Set<Listener>();
	private visibleTabIds: Set<string>;
	private scopedRevision = 0;
	private disposed = false;
	private reconciling = false;
	private reconcilePending = false;
	private scopedSnapshot: TabModelSnapshot = {
		activeTabId: null,
		groups: [],
		mostRecentGroupId: null,
		revision: 0,
		selectedTabIds: new Set(),
		selectionAnchorId: null,
		tabs: [],
	};
	private readonly unsubscribeSource: () => void;

	public constructor(source: TabModel, options: ScopedTabModelOptions = {}) {
		super();
		this.source = source;
		this.onTabCreated = options.onTabCreated;
		this.visibleTabIds = new Set(options.visibleTabIds ?? []);
		this.unsubscribeSource = source.subscribe(this.reconcile);
		this.reconcile();
	}

	public override readonly getSnapshot = (): TabModelSnapshot => this.scopedSnapshot;

	public override readonly subscribe = (listener: Listener): (() => void) => {
		if (this.disposed) return () => undefined;
		this.scopedListeners.add(listener);
		return () => this.scopedListeners.delete(listener);
	};

	public setVisibleTabIds(tabIds: readonly string[]): void {
		const next = new Set(tabIds);
		if (
			next.size === this.visibleTabIds.size &&
			[...next].every((tabId) => this.visibleTabIds.has(tabId))
		) {
			return;
		}
		this.visibleTabIds = next;
		this.reconcile();
	}

	/** Shows a newly created source tab until the host publishes its authoritative scope. */
	public includeTabOptimistically(tabId: string, activate = true): void {
		if (!this.source.getSnapshot().tabs.some((tab) => tab.id === tabId)) return;
		this.includeCreatedTab(tabId, activate);
	}

	public dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.unsubscribeSource();
		this.scopedListeners.clear();
	}

	public override subscribeBeforeTabAdded(listener: () => void): () => void {
		return this.source.subscribeBeforeTabAdded(listener);
	}

	public override getIndexOfTab(tabId: string): number | null {
		const index = this.scopedSnapshot.tabs.findIndex((tab) => tab.id === tabId);
		return index === -1 ? null : index;
	}

	public override getGroup(groupId: TabGroupId): Readonly<TabGroupData> | null {
		return this.scopedSnapshot.groups.find((group) => group.id === groupId) ?? null;
	}

	public override addTab(
		tab: Partial<Omit<TabData, 'id'>> & Pick<TabData, 'title'>,
		options: {
			id?: string;
			index?: number;
			activate?: boolean;
		} = {},
	): string {
		const sourceIndex =
			options.index === undefined ? undefined : this.sourceBoundaryForVisibleIndex(options.index);
		const id = this.source.addTab(tab, {
			...options,
			...(sourceIndex === undefined ? {} : { index: sourceIndex }),
		});
		this.includeCreatedTab(id, options.activate ?? true);
		return id;
	}

	public override closeTab(tabId: string): boolean {
		if (!this.isVisible(tabId)) return false;
		return this.source.closeTab(tabId);
	}

	public override closeTabs(tabIds: readonly string[]): boolean {
		return this.source.closeTabs(this.existingVisibleTabIds(tabIds));
	}

	public override closeSelectedTabs(): void {
		this.source.closeTabs([...this.scopedSnapshot.selectedTabIds]);
	}

	public override detachTabsForTransfer(tabIds: readonly string[]): DetachedTabBundle | null {
		return this.source.detachTabsForTransfer(this.existingVisibleTabIds(tabIds));
	}

	public override insertTransferredTabs(
		bundle: Readonly<DetachedTabBundle>,
		options: {
			index?: number;
			targetGroup?: TabGroupId | null;
			activateTabId?: string | null;
		} = {},
	): boolean {
		const sourceIndex =
			options.index === undefined ? undefined : this.sourceBoundaryForVisibleIndex(options.index);
		const inserted = this.source.insertTransferredTabs(bundle, {
			...options,
			...(sourceIndex === undefined ? {} : { index: sourceIndex }),
		});
		if (inserted) {
			for (const tab of bundle.tabs) this.includeCreatedTab(tab.id, false);
			if (options.activateTabId !== null) {
				const activeTabId = options.activateTabId ?? bundle.activeTabId ?? bundle.tabs[0]?.id;
				if (activeTabId !== undefined) this.source.selectTab(activeTabId);
			}
		}
		return inserted;
	}

	public override transferTabsTo(
		target: TabModel,
		tabIds: readonly string[],
		options: {
			index?: number;
			targetGroup?: TabGroupId | null;
			activateTabId?: string | null;
		} = {},
	): boolean {
		const visibleIds = this.existingVisibleTabIds(tabIds);
		if (visibleIds.length === 0) return false;
		if (target === this) {
			const targetIndex = options.index ?? this.scopedSnapshot.tabs.length;
			return this.moveTabsByDrag(
				visibleIds,
				targetIndex,
				0,
				this.scopedSnapshot.tabs.length,
				options.targetGroup ?? null,
			);
		}
		if (target instanceof ScopedTabModel) {
			if (target.source === this.source) return false;
			const sourceIndex =
				options.index === undefined
					? undefined
					: target.sourceBoundaryForVisibleIndex(options.index);
			const transferred = this.source.transferTabsTo(target.source, visibleIds, {
				...options,
				...(sourceIndex === undefined ? {} : { index: sourceIndex }),
			});
			if (transferred) {
				for (const tabId of visibleIds) target.includeCreatedTab(tabId, false);
			}
			return transferred;
		}
		return this.source.transferTabsTo(target, visibleIds, options);
	}

	public override closeOtherTabs(tabId: string): void {
		this.requireVisibleTab(tabId);
		const tabIds = this.scopedSnapshot.tabs.flatMap((tab) =>
			tab.id === tabId || tab.pinned ? [] : [tab.id],
		);
		this.source.closeTabs(tabIds);
		this.source.selectTab(tabId);
	}

	public override closeTabsToRight(tabId: string): void {
		const index = this.requireVisibleTab(tabId);
		const tabIds = this.scopedSnapshot.tabs
			.slice(index + 1)
			.flatMap((tab) => (tab.pinned ? [] : [tab.id]));
		this.source.closeTabs(tabIds);
	}

	public override duplicateTab(tabId: string): string {
		this.requireVisibleTab(tabId);
		const id = this.source.duplicateTab(tabId);
		this.includeCreatedTab(id, false);
		return id;
	}

	public override updateTab(
		tabId: string,
		update: Partial<Omit<TabData, 'id' | 'pinned' | 'group'>>,
	): void {
		this.requireVisibleTab(tabId);
		this.source.updateTab(tabId, update);
	}

	public override selectTab(tabId: string): void {
		this.requireVisibleTab(tabId);
		this.source.selectTab(tabId);
	}

	public override toggleSelected(tabId: string): void {
		this.requireVisibleTab(tabId);
		this.source.toggleSelected(tabId);
	}

	public override extendSelectionTo(tabId: string): void {
		const range = this.visibleSelectionRangeTo(tabId);
		this.source.setSelection(range, tabId);
	}

	public override addSelectionFromAnchorTo(tabId: string): void {
		const selected = new Set(this.scopedSnapshot.selectedTabIds);
		for (const id of this.visibleSelectionRangeTo(tabId)) selected.add(id);
		this.source.setSelection([...selected], tabId);
	}

	public override setSelection(
		tabIds: readonly string[],
		activeTabId: string,
		options: Readonly<{
			expandGroups?: boolean;
		}> = {},
	): void {
		this.requireVisibleTab(activeTabId);
		this.source.setSelection(this.existingVisibleTabIds(tabIds), activeTabId, options);
	}

	public override shiftTabRelative(tabId: string, offset: -1 | 1): void {
		const startIndex = this.requireVisibleTab(tabId);
		let targetIndex = startIndex + offset;
		const movingTab = this.scopedSnapshot.tabs[startIndex];
		const target = this.scopedSnapshot.tabs[targetIndex];
		if (movingTab === undefined) return;
		if (target === undefined || target.pinned !== movingTab.pinned) {
			if (movingTab.group !== null) this.source.moveTabsToGroup([tabId], null);
			return;
		}
		if (movingTab.group !== target.group) {
			if (movingTab.group !== null) {
				this.source.moveTabsToGroup([tabId], null);
				return;
			}
			if (target.group !== null) {
				const group = this.getGroup(target.group);
				if (group?.collapsed === false) {
					this.source.moveTabsToGroup([tabId], target.group);
					return;
				}
				if (group?.collapsed === true) {
					let candidateIndex = targetIndex + offset;
					while (this.scopedSnapshot.tabs[candidateIndex]?.group === target.group) {
						candidateIndex += offset;
					}
					if (this.scopedSnapshot.tabs[candidateIndex]) {
						targetIndex = candidateIndex - offset;
					} else {
						targetIndex = offset < 0 ? 0 : this.scopedSnapshot.tabs.length - 1;
					}
				}
			}
		}
		const boundary = this.visiblePinnedBoundary();
		this.moveTabsByDrag(
			[tabId],
			offset > 0 ? targetIndex + 1 : targetIndex,
			movingTab.pinned ? 0 : boundary,
			movingTab.pinned ? boundary : this.scopedSnapshot.tabs.length,
			movingTab.group,
		);
	}

	public override moveTabFirst(tabId: string): void {
		const index = this.requireVisibleTab(tabId);
		const tab = this.scopedSnapshot.tabs[index];
		if (tab === undefined) return;
		const boundary = this.visiblePinnedBoundary();
		this.moveTabsByDrag(
			[tabId],
			tab.pinned ? 0 : boundary,
			tab.pinned ? 0 : boundary,
			tab.pinned ? boundary : this.scopedSnapshot.tabs.length,
			null,
		);
	}

	public override moveTabLast(tabId: string): void {
		const index = this.requireVisibleTab(tabId);
		const tab = this.scopedSnapshot.tabs[index];
		if (tab === undefined) return;
		const boundary = this.visiblePinnedBoundary();
		this.moveTabsByDrag(
			[tabId],
			tab.pinned ? boundary : this.scopedSnapshot.tabs.length,
			tab.pinned ? 0 : boundary,
			tab.pinned ? boundary : this.scopedSnapshot.tabs.length,
			null,
		);
	}

	public override shiftGroupRelative(groupId: TabGroupId, offset: -1 | 1): void {
		this.requireVisibleGroup(groupId);
		this.source.shiftGroupRelative(groupId, offset);
	}

	public override moveGroupTo(groupId: TabGroupId, targetIndex: number): void {
		this.requireVisibleGroup(groupId);
		this.source.moveGroupTo(groupId, this.sourceBoundaryForVisibleIndex(targetIndex));
	}

	public override moveGroupByDrag(groupId: TabGroupId, preRemovalTargetIndex: number): void {
		this.requireVisibleGroup(groupId);
		this.source.moveGroupByDrag(groupId, this.sourceBoundaryForVisibleIndex(preRemovalTargetIndex));
	}

	public override setTabsPinned(tabIds: readonly string[], pinned: boolean): void {
		this.source.setTabsPinned(this.existingVisibleTabIds(tabIds), pinned);
	}

	public override createGroup(group: TabGroupData, tabIds: readonly string[]): void {
		this.source.createGroup(group, this.existingVisibleTabIds(tabIds));
	}

	public override moveTabsToGroup(tabIds: readonly string[], groupId: TabGroupId | null): void {
		this.source.moveTabsToGroup(this.existingVisibleTabIds(tabIds), groupId);
	}

	public override toggleGroupCollapsed(
		groupId: TabGroupId,
		origin?: ToggleTabGroupCollapsedStateOrigin,
	): void {
		this.requireVisibleGroup(groupId);
		this.source.toggleGroupCollapsed(groupId, origin);
	}

	public override updateGroup(
		groupId: TabGroupId,
		update: Partial<Omit<TabGroupData, 'id'>>,
	): void {
		this.requireVisibleGroup(groupId);
		this.source.updateGroup(groupId, update);
	}

	public override ungroup(groupId: TabGroupId): void {
		this.requireVisibleGroup(groupId);
		this.source.ungroup(groupId);
	}

	public override closeGroup(groupId: TabGroupId): void {
		this.requireVisibleGroup(groupId);
		this.source.closeGroup(groupId);
	}

	public override moveSelectedTabsByDrag(
		targetIndex: number,
		containerStartIndex: number,
		containerEndIndex: number,
		targetGroup: TabGroupId | null,
	): boolean {
		return this.moveTabsByDrag(
			[...this.scopedSnapshot.selectedTabIds],
			targetIndex,
			containerStartIndex,
			containerEndIndex,
			targetGroup,
		);
	}

	public override captureDragRestoreState(): TabDragRestoreState {
		return this.source.captureDragRestoreState();
	}

	public override restoreCanceledDrag(state: TabDragRestoreState): void {
		this.source.restoreCanceledDrag(state);
	}

	public override moveTabsByDrag(
		tabIds: readonly string[],
		targetIndex: number,
		containerStartIndex: number,
		containerEndIndex: number,
		targetGroup: TabGroupId | null,
		options: Readonly<{
			coalesceNonconsecutive?: boolean;
		}> = {},
	): boolean {
		const visibleIds = this.existingVisibleTabIds(tabIds);
		if (visibleIds.length === 0) return false;
		return this.source.moveTabsByDrag(
			visibleIds,
			this.sourceBoundaryForVisibleIndex(targetIndex),
			this.sourceBoundaryForVisibleIndex(containerStartIndex),
			this.sourceBoundaryForVisibleIndex(containerEndIndex),
			targetGroup,
			options,
		);
	}

	private readonly reconcile = (): void => {
		if (this.disposed) return;
		if (this.reconciling) {
			this.reconcilePending = true;
			return;
		}
		this.reconciling = true;
		do {
			this.reconcilePending = false;
			const sourceSnapshot = this.source.getSnapshot();
			const activeTabId = sourceSnapshot.activeTabId;
			if (
				activeTabId !== null &&
				!this.visibleTabIds.has(activeTabId) &&
				sourceSnapshot.tabs.some((tab) => this.visibleTabIds.has(tab.id))
			) {
				const firstVisibleTab = sourceSnapshot.tabs.find((tab) => this.visibleTabIds.has(tab.id));
				if (firstVisibleTab !== undefined) this.source.selectTab(firstVisibleTab.id);
			}
		} while (this.reconcilePending);

		this.scopedRevision += 1;
		this.scopedSnapshot = this.createScopedSnapshot(this.source.getSnapshot());
		this.reconciling = false;
		this.scopedListeners.forEach((listener) => listener());
	};

	private createScopedSnapshot(source: TabModelSnapshot): TabModelSnapshot {
		const tabs = source.tabs.filter((tab) => this.visibleTabIds.has(tab.id));
		const groupIds = new Set(tabs.flatMap((tab) => (tab.group === null ? [] : [tab.group])));
		const groups = source.groups.filter((group) => groupIds.has(group.id));
		const activeTabId =
			source.activeTabId !== null && this.visibleTabIds.has(source.activeTabId)
				? source.activeTabId
				: (tabs[0]?.id ?? null);
		const selectedTabIds = new Set(
			[...source.selectedTabIds].filter((tabId) => this.visibleTabIds.has(tabId)),
		);
		if (activeTabId !== null) selectedTabIds.add(activeTabId);
		const selectionAnchorId =
			source.selectionAnchorId !== null && this.visibleTabIds.has(source.selectionAnchorId)
				? source.selectionAnchorId
				: activeTabId;
		const activeGroupId = tabs.find((tab) => tab.id === activeTabId)?.group ?? null;
		const mostRecentGroupId =
			source.mostRecentGroupId !== null && groupIds.has(source.mostRecentGroupId)
				? source.mostRecentGroupId
				: (activeGroupId ?? groups.at(-1)?.id ?? null);
		return {
			activeTabId,
			groups,
			mostRecentGroupId,
			revision: this.scopedRevision,
			selectedTabIds,
			selectionAnchorId,
			tabs,
		};
	}

	private includeCreatedTab(tabId: string, activate: boolean): void {
		this.visibleTabIds.add(tabId);
		this.onTabCreated?.(tabId);
		if (activate) this.source.selectTab(tabId);
		this.reconcile();
	}

	private isVisible(tabId: string): boolean {
		return this.scopedSnapshot.tabs.some((tab) => tab.id === tabId);
	}

	private existingVisibleTabIds(tabIds: readonly string[]): string[] {
		const candidates = new Set(tabIds);
		return this.scopedSnapshot.tabs.flatMap((tab) => (candidates.has(tab.id) ? [tab.id] : []));
	}

	private requireVisibleTab(tabId: string): number {
		const index = this.getIndexOfTab(tabId);
		if (index === null) throw new Error(`Unknown window-local tab ${tabId}.`);
		return index;
	}

	private requireVisibleGroup(groupId: TabGroupId): void {
		if (this.getGroup(groupId) === null) {
			throw new Error(`Unknown window-local group ${groupId}.`);
		}
	}

	private visibleSelectionRangeTo(tabId: string): string[] {
		const targetIndex = this.requireVisibleTab(tabId);
		const anchorIndex =
			this.scopedSnapshot.selectionAnchorId === null
				? targetIndex
				: (this.getIndexOfTab(this.scopedSnapshot.selectionAnchorId) ?? targetIndex);
		return this.scopedSnapshot.tabs
			.slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1)
			.map((tab) => tab.id);
	}

	private visiblePinnedBoundary(): number {
		const index = this.scopedSnapshot.tabs.findIndex((tab) => !tab.pinned);
		return index === -1 ? this.scopedSnapshot.tabs.length : index;
	}

	private sourceBoundaryForVisibleIndex(index: number): number {
		const sourceTabs = this.source.getSnapshot().tabs;
		const visibleTabs = this.scopedSnapshot.tabs;
		if (visibleTabs.length === 0) return sourceTabs.length;
		const clampedIndex = Math.min(Math.max(index, 0), visibleTabs.length);
		if (clampedIndex === visibleTabs.length) {
			const lastVisibleId = visibleTabs.at(-1)?.id;
			const lastSourceIndex = sourceTabs.findIndex((tab) => tab.id === lastVisibleId);
			return lastSourceIndex === -1 ? sourceTabs.length : lastSourceIndex + 1;
		}
		const visibleId = visibleTabs[clampedIndex]?.id;
		const sourceIndex = sourceTabs.findIndex((tab) => tab.id === visibleId);
		return sourceIndex === -1 ? sourceTabs.length : sourceIndex;
	}
}
