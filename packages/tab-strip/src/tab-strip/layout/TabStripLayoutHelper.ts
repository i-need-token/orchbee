import { requireDefined } from '../utils/requireDefined';
import type { Rect } from '../utils/rectRight';
import { rectRight } from '../utils/rectRight';
import { TabActive, TabOpen, TabPinned, type TabGroupId } from '../model/tab-types';
import { DEFAULT_TAB_STYLE_METRICS, type TabStyleMetrics } from './tab-style-metrics';
import { TabLayoutState } from './TabLayoutState';
import { calculateTabBounds } from './calculateTabBounds';
import { LayoutDomain, type TabSizeInfo } from './tab-strip-layout-types';
import { TabWidthConstraints } from './TabWidthConstraints';
type TabSlotViewType = 'tab' | 'group-header';

/** Headless counterpart of the fields TabSlotView exposes to layout. */
export interface TabSlotViewData {
	id: string;
	type: TabSlotViewType;
	group: TabGroupId | null;
	sizeInfo: Readonly<TabSizeInfo>;
}
export interface TabStripLayoutController {
	getActiveIndex(): number | null;
	isGroupCollapsed(group: TabGroupId): boolean;
}
interface TabStripIdealBounds {
	totalWidth: number;
	layoutDomain: LayoutDomain;
	tabBounds: ReadonlyMap<string, Rect>;
	groupHeaderBounds: ReadonlyMap<TabGroupId, Rect>;
	slotBounds: readonly Rect[];
}
interface TabSlot {
	view: TabSlotViewData;
	state: TabLayoutState;
}
function checkIndex(index: number, length: number, message: string): void {
	if (!Number.isInteger(index) || index < 0 || index >= length) {
		throw new RangeError(`${message}: ${index}`);
	}
}

/**
 * Headless port of TabStripLayoutHelper from tab_strip_layout_helper.{h,cc}.
 *
 * It deliberately retains closing tabs in the visual slot list while omitting
 * them from model-index calculations, just like views::ViewModelT in Chrome.
 */
export class TabStripLayoutHelper {
	private readonly controller: TabStripLayoutController;
	private readonly style: Readonly<TabStyleMetrics>;
	private readonly slots: TabSlot[] = [];
	private currentLayoutDomain: LayoutDomain = LayoutDomain.InactiveWidthEqualsActiveWidth;
	public constructor(
		controller: TabStripLayoutController,
		style: Readonly<TabStyleMetrics> = DEFAULT_TAB_STYLE_METRICS,
	) {
		this.controller = controller;
		this.style = style;
	}
	public get layoutDomain(): LayoutDomain {
		return this.currentLayoutDomain;
	}

	/** Includes closing tabs, matching TabStripLayoutHelper::GetTabs. */
	public getTabs(): readonly TabSlotViewData[] {
		return this.slots.filter((slot) => slot.view.type === 'tab').map((slot) => slot.view);
	}
	public getTabSlotViews(): readonly TabSlotViewData[] {
		return this.slots.map((slot) => slot.view);
	}
	public getPinnedTabCount(): number {
		return this.slots.filter(
			(slot) => slot.state.pinned === TabPinned.Pinned && !slot.state.isClosed(),
		).length;
	}
	public insertTabAt(modelIndex: number, view: TabSlotViewData, pinned: TabPinned): void {
		if (view.type !== 'tab') {
			throw new TypeError('insertTabAt requires a tab view.');
		}
		const slotIndex = this.getSlotInsertionIndexForNewTab(modelIndex, view.group);
		this.slots.splice(slotIndex, 0, this.createSlot(view, pinned));
	}
	public markTabAsClosing(modelIndex: number, tabId: string): void {
		const slotIndex = this.getSlotIndexForExistingTab(modelIndex);
		const slot = this.slots[slotIndex];
		if (!slot || slot.view.id !== tabId) {
			throw new Error(`Tab ${tabId} is not at model index ${modelIndex}.`);
		}
		slot.state.open = TabOpen.Closed;
	}
	public removeTab(tabId: string): void {
		const slotIndex = this.slots.findIndex(
			(slot) => slot.view.type === 'tab' && slot.view.id === tabId,
		);
		if (slotIndex !== -1) {
			this.slots.splice(slotIndex, 1);
		}
	}
	public moveTab(movingTabGroup: TabGroupId | null, previousIndex: number, newIndex: number): void {
		const previousSlotIndex = this.getSlotIndexForExistingTab(previousIndex);
		const movingTab = this.slots[previousSlotIndex];
		if (!movingTab) {
			throw new Error('Moving tab slot disappeared.');
		}
		this.slots.splice(previousSlotIndex, 1);
		const newSlotIndex = this.getSlotInsertionIndexForNewTab(newIndex, movingTabGroup);
		this.slots.splice(newSlotIndex, 0, movingTab);
		if (movingTabGroup !== null) {
			this.updateGroupHeaderIndex(movingTabGroup);
		}
	}
	public setTabPinned(modelIndex: number, pinned: TabPinned): void {
		const slot = this.slots[this.getSlotIndexForExistingTab(modelIndex)];
		if (!slot) {
			throw new Error(`No tab at model index ${modelIndex}.`);
		}
		slot.state.pinned = pinned;
	}
	public setTabGroup(tabId: string, group: TabGroupId | null): void {
		const slot = this.slots.find(
			(candidate) => candidate.view.type === 'tab' && candidate.view.id === tabId,
		);
		if (!slot) {
			throw new Error(`Unknown tab ${tabId}.`);
		}
		slot.view = {
			...slot.view,
			group,
		};
	}
	public insertGroupHeader(group: TabGroupId, view: TabSlotViewData): void {
		if (view.type !== 'group-header' || view.group !== group) {
			throw new TypeError('Group header type and group must match insertGroupHeader arguments.');
		}
		const firstTabModelIndex = this.getFirstTabModelIndexForGroup(group);
		if (firstTabModelIndex === null) {
			throw new Error(`Cannot insert header for empty group ${group}.`);
		}
		const headerSlotIndex = this.getSlotInsertionIndexForNewTab(firstTabModelIndex, group);
		this.slots.splice(headerSlotIndex, 0, this.createSlot(view, TabPinned.Unpinned));
	}
	public removeGroupHeader(group: TabGroupId): void {
		this.slots.splice(this.getSlotIndexForGroupHeader(group), 1);
	}
	public updateGroupHeaderIndex(group: TabGroupId): void {
		if (this.getFirstTabSlotForGroup(group) === null) {
			return;
		}
		const headerSlotIndex = this.getSlotIndexForGroupHeader(group);
		const [headerSlot] = this.slots.splice(headerSlotIndex, 1);
		if (!headerSlot) {
			throw new Error(`Missing header slot for group ${group}.`);
		}

		// Recalculate after removing the header, as the Chromium implementation does.
		const firstTabSlot = this.getFirstTabSlotForGroup(group);
		if (firstTabSlot === null) {
			throw new Error(`Group ${group} became empty while moving its header.`);
		}
		this.slots.splice(firstTabSlot, 0, headerSlot);
	}
	public setActiveTab(previousActiveIndex: number | null, newActiveIndex: number | null): void {
		this.setActiveAtModelIndex(previousActiveIndex, TabActive.Inactive);
		this.setActiveAtModelIndex(newActiveIndex, TabActive.Active);
	}
	public calculateMinimumWidth(): number {
		const result = this.calculateIdealBounds(0);
		return result.bounds.length === 0 ? 0 : rectRight(requireDefined(result.bounds.at(-1)));
	}
	public calculatePreferredWidth(): number {
		const result = this.calculateIdealBounds(null);
		return result.bounds.length === 0 ? 0 : rectRight(requireDefined(result.bounds.at(-1)));
	}
	public updateIdealBounds(availableWidth: number): TabStripIdealBounds {
		const { bounds, domain } = this.calculateIdealBounds(availableWidth);
		this.currentLayoutDomain = domain;
		const tabBounds = new Map<string, Rect>();
		const groupHeaderBounds = new Map<TabGroupId, Rect>();
		this.slots.forEach((slot, index) => {
			const slotBounds = bounds[index];
			if (!slotBounds) {
				throw new Error('Slot and bounds lengths diverged.');
			}
			if (slot.view.type === 'tab') {
				if (!slot.state.isClosed()) {
					tabBounds.set(slot.view.id, slotBounds);
				}
			} else if (slot.view.group !== null) {
				groupHeaderBounds.set(slot.view.group, slotBounds);
			}
		});
		return {
			totalWidth: bounds.length === 0 ? 0 : rectRight(requireDefined(bounds.at(-1))),
			layoutDomain: domain,
			tabBounds,
			groupHeaderBounds,
			slotBounds: bounds,
		};
	}
	private createSlot(view: TabSlotViewData, pinned: TabPinned): TabSlot {
		return {
			view,
			state: new TabLayoutState(TabOpen.Open, pinned, TabActive.Inactive),
		};
	}
	private calculateIdealBounds(availableWidth: number | null) {
		const activeModelIndex = this.controller.getActiveIndex();
		const activeSlotIndex =
			activeModelIndex === null ? null : this.getSlotIndexForExistingTab(activeModelIndex);
		const pinnedTabCount = this.getPinnedTabCount();
		const lastPinnedTabSlotIndex =
			pinnedTabCount > 0 ? this.getSlotIndexForExistingTab(pinnedTabCount - 1) : null;
		const constraints = this.slots.map((slot, index) => {
			const active = index === activeSlotIndex ? TabActive.Active : TabActive.Inactive;
			const pinned =
				lastPinnedTabSlotIndex !== null && index <= lastPinnedTabSlotIndex
					? TabPinned.Pinned
					: TabPinned.Unpinned;
			const open =
				slot.state.isClosed() || this.slotIsCollapsedTab(index) ? TabOpen.Closed : TabOpen.Open;
			return new TabWidthConstraints(
				new TabLayoutState(open, pinned, active),
				slot.view.sizeInfo,
				this.style,
			);
		});
		return calculateTabBounds(constraints, availableWidth, this.style);
	}
	private getSlotIndexForExistingTab(modelIndex: number): number {
		const originalSlotIndex = this.getFirstSlotIndexForTabModelIndex(modelIndex);
		checkIndex(
			originalSlotIndex,
			this.slots.length,
			'Model index does not represent an existing tab',
		);
		let slotIndex = originalSlotIndex;
		const originalSlot = requireDefined(this.slots[slotIndex]);
		if (originalSlot.view.type === 'tab') {
			if (originalSlot.state.isClosed()) {
				throw new Error(`Model index ${modelIndex} resolved to a closing tab.`);
			}
			return slotIndex;
		}

		// A group header shares the first grouped tab's model index. Skip it and
		// any tabs that are still occupying space while animating closed.
		do {
			slotIndex += 1;
		} while (
			slotIndex < this.slots.length &&
			requireDefined(this.slots[slotIndex]).state.isClosed()
		);
		checkIndex(slotIndex, this.slots.length, 'Group header is not followed by an open tab');
		if (requireDefined(this.slots[slotIndex]).view.type !== 'tab') {
			throw new Error('Group header is not followed by a tab.');
		}
		return slotIndex;
	}
	private getSlotInsertionIndexForNewTab(modelIndex: number, group: TabGroupId | null): number {
		const slotIndex = this.getFirstSlotIndexForTabModelIndex(modelIndex);
		if (slotIndex === this.slots.length) {
			return slotIndex;
		}
		const slot = requireDefined(this.slots[slotIndex]);
		if (slot.view.type === 'group-header' && slot.view.group === group) {
			return slotIndex + 1;
		}
		return slotIndex;
	}
	private getFirstTabSlotForGroup(group: TabGroupId): number | null {
		for (let slotIndex = 0; slotIndex < this.slots.length; slotIndex += 1) {
			const slot = requireDefined(this.slots[slotIndex]);
			if (slot.state.isClosed()) {
				continue;
			}
			if (slot.view.type === 'tab' && slot.view.group === group) {
				return slotIndex;
			}
		}
		return null;
	}
	private getFirstTabModelIndexForGroup(group: TabGroupId): number | null {
		let modelIndex = 0;
		for (const slot of this.slots) {
			if (slot.state.isClosed() || slot.view.type !== 'tab') {
				continue;
			}
			if (slot.view.group === group) {
				return modelIndex;
			}
			modelIndex += 1;
		}
		return null;
	}
	private getFirstSlotIndexForTabModelIndex(modelIndex: number): number {
		if (!Number.isInteger(modelIndex) || modelIndex < 0) {
			throw new RangeError(`Invalid model index: ${modelIndex}`);
		}
		let currentModelIndex = 0;
		for (let slotIndex = 0; slotIndex < this.slots.length; slotIndex += 1) {
			const slot = requireDefined(this.slots[slotIndex]);
			if (slot.state.isClosed()) {
				continue;
			}
			if (modelIndex === currentModelIndex) {
				return slotIndex;
			}
			if (slot.view.type === 'tab') {
				currentModelIndex += 1;
			}
		}
		if (currentModelIndex !== modelIndex) {
			throw new RangeError(`Model index is too large: ${modelIndex}`);
		}
		return this.slots.length;
	}
	private getSlotIndexForGroupHeader(group: TabGroupId): number {
		const slotIndex = this.slots.findIndex(
			(slot) => slot.view.type === 'group-header' && slot.view.group === group,
		);
		if (slotIndex === -1) {
			throw new Error(`Unknown group header ${group}.`);
		}
		return slotIndex;
	}
	private slotIsCollapsedTab(index: number): boolean {
		const slot = this.slots[index];
		if (!slot) {
			throw new RangeError(`Unknown slot ${index}.`);
		}
		return (
			slot.view.type === 'tab' &&
			slot.view.group !== null &&
			this.controller.isGroupCollapsed(slot.view.group)
		);
	}
	private setActiveAtModelIndex(modelIndex: number | null, active: TabActive): void {
		if (modelIndex === null) {
			return;
		}
		const slotIndex = this.getSlotIndexForExistingTab(modelIndex);
		requireDefined(this.slots[slotIndex]).state.active = active;
	}
}
