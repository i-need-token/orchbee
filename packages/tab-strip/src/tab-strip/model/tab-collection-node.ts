import { requireDefined } from '../utils/requireDefined';
import type { TabStripOrientation } from '../types/tab-strip-types';
import type { TabModel, TabModelSnapshot } from './tab-model';

/** Mirrors tabs::TabCollection::Type plus the leaf TAB type. */
export const TabCollectionNodeType = {
	TabStrip: 'tabstrip',
	Pinned: 'pinned',
	Unpinned: 'unpinned',
	Group: 'group',
	Tab: 'tab',
} as const;
export type TabCollectionNodeType =
	(typeof TabCollectionNodeType)[keyof typeof TabCollectionNodeType];
interface TabCollectionNodeDescriptor {
	handle: string;
	type: TabCollectionNodeType;
	children?: readonly TabCollectionNodeDescriptor[];
}

/** Minimal View-like object used by the default headless adapter. */
export interface TabCollectionMemoryView {
	handle: string;
	type: TabCollectionNodeType;
	parent: TabCollectionMemoryView | null;
	children: TabCollectionMemoryView[];
	focused: boolean;
	focusPrevious: TabCollectionMemoryView | null;
	focusNext: TabCollectionMemoryView | null;
	zOrder: number;
}

/**
 * DOM, React, or test hosts can replace Views operations without changing the
 * collection tree. The callbacks correspond to Chromium's custom add/remove
 * and attach/detach hooks.
 */
interface TabCollectionViewAdapter<View = TabCollectionMemoryView> {
	createView(node: TabCollectionNode<View>): View;
	addChild(parent: View, child: View, index: number): void;
	removeChild(parent: View, child: View): void;
	detachChild(parent: View, child: View): View;
	attachChild(parent: View, child: View, index: number): void;
	moveChildToFront?(parent: View, child: View): void;
	updateFocusOrder?(parent: View, children: readonly View[]): void;
	isFocused?(view: View): boolean;
	restoreFocus?(view: View): void;
}
export class MemoryTabCollectionViewAdapter implements TabCollectionViewAdapter<TabCollectionMemoryView> {
	private zSequence = 0;
	public createView(node: TabCollectionNode<TabCollectionMemoryView>): TabCollectionMemoryView {
		return {
			handle: node.handle,
			type: node.type,
			parent: null,
			children: [],
			focused: false,
			focusPrevious: null,
			focusNext: null,
			zOrder: this.zSequence,
		};
	}
	public addChild(
		parent: TabCollectionMemoryView,
		child: TabCollectionMemoryView,
		index: number,
	): void {
		child.parent = parent;
		parent.children.splice(Math.min(Math.max(index, 0), parent.children.length), 0, child);
	}
	public removeChild(parent: TabCollectionMemoryView, child: TabCollectionMemoryView): void {
		const index = parent.children.indexOf(child);
		if (index !== -1) parent.children.splice(index, 1);
		child.parent = null;
	}
	public detachChild(
		parent: TabCollectionMemoryView,
		child: TabCollectionMemoryView,
	): TabCollectionMemoryView {
		this.removeChild(parent, child);
		return child;
	}
	public attachChild(
		parent: TabCollectionMemoryView,
		child: TabCollectionMemoryView,
		index: number,
	): void {
		this.addChild(parent, child, index);
	}
	public moveChildToFront(_parent: TabCollectionMemoryView, child: TabCollectionMemoryView): void {
		child.zOrder = ++this.zSequence;
	}
	public updateFocusOrder(
		_parent: TabCollectionMemoryView,
		children: readonly TabCollectionMemoryView[],
	): void {
		children.forEach((child, index) => {
			child.focusPrevious = children[index - 1] ?? null;
			child.focusNext = children[index + 1] ?? null;
		});
	}
	public isFocused(view: TabCollectionMemoryView): boolean {
		return view.focused;
	}
	public restoreFocus(view: TabCollectionMemoryView): void {
		view.focused = true;
	}
}
type Subscription = () => void;

/** Browser-independent port of common/TabCollectionNode. */
export class TabCollectionNode<View = TabCollectionMemoryView> {
	public readonly orientation: TabStripOrientation;
	protected readonly adapter: TabCollectionViewAdapter<View>;
	private descriptor: TabCollectionNodeDescriptor;
	private childNodes: TabCollectionNode<View>[] = [];
	private nodeView: View | null = null;
	private controller: unknown = null;
	private destroyed = false;
	private readonly willDestroyCallbacks = new Set<() => void>();
	private readonly selectionChangedCallbacks = new Set<() => void>();
	public constructor(
		descriptor: TabCollectionNodeDescriptor,
		orientation: TabStripOrientation,
		adapter: TabCollectionViewAdapter<View>,
	) {
		this.descriptor = descriptor;
		this.orientation = orientation;
		this.adapter = adapter;
	}
	public get handle(): string {
		return this.descriptor.handle;
	}
	public get type(): TabCollectionNodeType {
		return this.descriptor.type;
	}
	public get children(): readonly TabCollectionNode<View>[] {
		return this.childNodes;
	}
	public get view(): View | null {
		return this.nodeView;
	}
	public initialize(): View {
		if (this.nodeView !== null) return this.nodeView;
		this.nodeView = this.adapter.createView(this);
		for (const child of this.descriptor.children ?? []) {
			this.addNewChild(child, this.childNodes.length, true);
		}
		return this.nodeView;
	}
	public deinitialize(): void {
		while (this.childNodes.length > 0) {
			this.removeChild(requireDefined(this.childNodes[0]).handle, true);
		}
	}

	/** Root-only lifecycle hook mirroring RootTabCollectionNode::Reset(). */
	public resetForReinitialization(descriptor: TabCollectionNodeDescriptor): View | null {
		if (descriptor.handle !== this.handle || descriptor.type !== this.type) {
			throw new Error('A collection node can only be reset with the same handle and type.');
		}
		this.deinitialize();
		const previousView = this.nodeView;
		this.nodeView = null;
		this.descriptor = descriptor;
		return previousView;
	}
	public getNodeForHandle(handle: string): TabCollectionNode<View> | null {
		if (this.handle === handle) return this;
		for (const child of this.childNodes) {
			const match = child.getNodeForHandle(handle);
			if (match) return match;
		}
		return null;
	}
	public getParentNodeForHandle(handle: string): TabCollectionNode<View> | null {
		if (this.childNodes.some((child) => child.handle === handle)) return this;
		for (const child of this.childNodes) {
			const match = child.getParentNodeForHandle(handle);
			if (match) return match;
		}
		return null;
	}
	public getChildNodeOfType(type: TabCollectionNodeType): TabCollectionNode<View> | null {
		if (this.type === type) return this;
		return this.childNodes.find((child) => child.type === type) ?? null;
	}
	public addNewChild(
		descriptor: TabCollectionNodeDescriptor,
		modelIndex: number,
		performInitialization: boolean,
	): TabCollectionNode<View> {
		const parentView = this.requireView();
		const index = Math.min(Math.max(modelIndex, 0), this.childNodes.length);
		const child = new TabCollectionNode<View>(descriptor, this.orientation, this.adapter);
		child.setController(this.controller);
		this.childNodes.splice(index, 0, child);
		const childView = performInitialization ? child.initialize() : child.createAndSetView();
		this.adapter.addChild(parentView, childView, index);
		this.ensureFocusOrder();
		return child;
	}
	public removeChild(handle: string, performDeinitialization: boolean): boolean {
		const index = this.childNodes.findIndex((child) => child.handle === handle);
		if (index === -1) return false;
		const child = requireDefined(this.childNodes[index]);
		if (performDeinitialization) child.deinitialize();
		const childView = child.requireView();
		// Match the UAF fix in Chromium: remove/destroy the node before the view.
		this.childNodes.splice(index, 1);
		child.destroy();
		this.adapter.removeChild(this.requireView(), childView);
		this.ensureFocusOrder();
		return true;
	}
	public moveChild(handle: string, newIndex: number): boolean {
		const oldIndex = this.childNodes.findIndex((child) => child.handle === handle);
		if (oldIndex === -1) return false;
		const targetIndex = Math.min(Math.max(newIndex, 0), this.childNodes.length - 1);
		if (oldIndex === targetIndex) return false;
		const [child] = this.childNodes.splice(oldIndex, 1);
		this.childNodes.splice(targetIndex, 0, requireDefined(child));
		const parentView = this.requireView();
		const childView = requireDefined(child).requireView();
		this.adapter.detachChild(parentView, childView);
		this.adapter.attachChild(parentView, childView, targetIndex);
		this.adapter.moveChildToFront?.(parentView, childView);
		this.ensureFocusOrder();
		return true;
	}
	public static moveChildAcrossParents<View>(
		handle: string,
		newIndex: number,
		sourceParent: TabCollectionNode<View>,
		destinationParent: TabCollectionNode<View>,
	): TabCollectionNode<View> | null {
		const sourceIndex = sourceParent.childNodes.findIndex((child) => child.handle === handle);
		if (sourceIndex === -1) return null;
		const child = requireDefined(sourceParent.childNodes[sourceIndex]);
		const childView = child.requireView();
		const wasFocused = sourceParent.adapter.isFocused?.(childView) ?? false;
		const detachedView = sourceParent.adapter.detachChild(sourceParent.requireView(), childView);
		sourceParent.childNodes.splice(sourceIndex, 1);
		sourceParent.ensureFocusOrder();
		const targetIndex = Math.min(Math.max(newIndex, 0), destinationParent.childNodes.length);
		destinationParent.childNodes.splice(targetIndex, 0, child);
		child.setController(destinationParent.controller);
		destinationParent.adapter.attachChild(
			destinationParent.requireView(),
			detachedView,
			targetIndex,
		);
		destinationParent.ensureFocusOrder();
		if (wasFocused) destinationParent.adapter.restoreFocus?.(childView);
		return child;
	}
	public getDirectChildren(): View[] {
		return this.childNodes.map((child) => child.requireView());
	}
	public registerWillDestroyCallback(callback: () => void): Subscription {
		this.willDestroyCallbacks.add(callback);
		return () => this.willDestroyCallbacks.delete(callback);
	}
	public registerTabSelectionChangedCallback(callback: () => void): Subscription {
		this.selectionChangedCallbacks.add(callback);
		return () => this.selectionChangedCallbacks.delete(callback);
	}
	public notifyTabSelectionChanged(): void {
		this.selectionChangedCallbacks.forEach((callback) => callback());
	}
	public setController(controller: unknown): void {
		this.controller = controller;
		this.childNodes.forEach((child) => child.setController(controller));
	}
	public getController(): unknown {
		return this.controller;
	}
	private createAndSetView(): View {
		if (this.nodeView === null) this.nodeView = this.adapter.createView(this);
		return this.nodeView;
	}
	private requireView(): View {
		if (this.nodeView === null)
			throw new Error(`Collection node ${this.handle} is not initialized.`);
		return this.nodeView;
	}
	private ensureFocusOrder(): void {
		if (this.type === TabCollectionNodeType.TabStrip) return;
		this.adapter.updateFocusOrder?.(this.requireView(), this.getDirectChildren());
	}
	private destroy(): void {
		if (this.destroyed) return;
		this.destroyed = true;
		const callbacks = [...this.willDestroyCallbacks];
		this.willDestroyCallbacks.clear();
		callbacks.forEach((callback) => callback());
		this.selectionChangedCallbacks.clear();
	}
}
function createTabDescriptors(
	tabs: readonly TabModelSnapshot['tabs'][number][],
): TabCollectionNodeDescriptor[] {
	return tabs.map((tab) => ({
		handle: `tab:${tab.id}`,
		type: TabCollectionNodeType.Tab,
	}));
}

/** Builds Chromium's root -> pinned/unpinned -> group/tab hierarchy. */
export function buildTabCollectionDescriptor(
	snapshot: TabModelSnapshot,
): TabCollectionNodeDescriptor {
	const pinnedTabs = snapshot.tabs.filter((tab) => tab.pinned);
	const unpinnedTabs = snapshot.tabs.filter((tab) => !tab.pinned);
	const unpinnedChildren: TabCollectionNodeDescriptor[] = [];
	for (let index = 0; index < unpinnedTabs.length; index += 1) {
		const tab = requireDefined(unpinnedTabs[index]);
		if (tab.group === null) {
			unpinnedChildren.push(requireDefined(createTabDescriptors([tab])[0]));
			continue;
		}
		const groupTabs = [tab];
		while (unpinnedTabs[index + 1]?.group === tab.group)
			groupTabs.push(requireDefined(unpinnedTabs[++index]));
		unpinnedChildren.push({
			handle: `group:${tab.group}`,
			type: TabCollectionNodeType.Group,
			children: createTabDescriptors(groupTabs),
		});
	}
	return {
		handle: 'collection:tabstrip',
		type: TabCollectionNodeType.TabStrip,
		children: [
			{
				handle: 'collection:pinned',
				type: TabCollectionNodeType.Pinned,
				children: createTabDescriptors(pinnedTabs),
			},
			{
				handle: 'collection:unpinned',
				type: TabCollectionNodeType.Unpinned,
				children: unpinnedChildren,
			},
		],
	};
}
interface DescriptorPosition {
	parent: string | null;
	index: number;
}
function descriptorPositions(
	descriptor: TabCollectionNodeDescriptor,
	parent: string | null = null,
	result = new Map<string, DescriptorPosition>(),
): Map<string, DescriptorPosition> {
	result.set(descriptor.handle, {
		parent,
		index: 0,
	});
	(descriptor.children ?? []).forEach((child, index) => {
		descriptorPositions(child, descriptor.handle, result);
		result.set(child.handle, {
			parent: descriptor.handle,
			index,
		});
	});
	return result;
}
function setsDiffer(left: ReadonlySet<string>, right: ReadonlySet<string>): Set<string> {
	return new Set([...left].filter((value) => !right.has(value)));
}

/** Observable RootTabCollectionNode port backed by TabModel. */
export class RootTabCollectionNode<View = TabCollectionMemoryView> {
	private readonly model: Pick<TabModel, 'getSnapshot' | 'subscribe'>;
	public readonly root: TabCollectionNode<View>;
	private snapshot: TabModelSnapshot;
	private unsubscribeModel: Subscription | null = null;
	private initialized = false;
	private readonly childrenAddedCallbacks = new Set<() => void>();
	private readonly childRemovedCallbacks = new Set<() => void>();
	private readonly childMovedCallbacks = new Set<(node: TabCollectionNode<View>) => void>();
	private readonly activeTabChangedCallbacks = new Set<(tabId: string | null) => void>();
	public constructor(
		model: Pick<TabModel, 'getSnapshot' | 'subscribe'>,
		orientation: TabStripOrientation,
		adapter: TabCollectionViewAdapter<View>,
	) {
		this.model = model;
		this.snapshot = model.getSnapshot();
		this.root = new TabCollectionNode(
			buildTabCollectionDescriptor(this.snapshot),
			orientation,
			adapter,
		);
	}
	public init(): View {
		if (!this.initialized && this.root.view === null) {
			this.snapshot = this.model.getSnapshot();
			this.root.resetForReinitialization(buildTabCollectionDescriptor(this.snapshot));
		}
		const view = this.root.initialize();
		if (!this.initialized) {
			this.initialized = true;
			this.unsubscribeModel = this.model.subscribe(() => this.updateFromModel());
		}
		return view;
	}
	public reset(): void {
		this.unsubscribeModel?.();
		this.unsubscribeModel = null;
		this.initialized = false;
		this.snapshot = this.model.getSnapshot();
		this.root.resetForReinitialization(buildTabCollectionDescriptor(this.snapshot));
	}
	public getNodeForHandle(handle: string): TabCollectionNode<View> | null {
		return this.root.getNodeForHandle(handle);
	}
	public registerOnChildrenAddedCallback(callback: () => void): Subscription {
		this.childrenAddedCallbacks.add(callback);
		return () => this.childrenAddedCallbacks.delete(callback);
	}
	public registerOnChildRemovedCallback(callback: () => void): Subscription {
		this.childRemovedCallbacks.add(callback);
		return () => this.childRemovedCallbacks.delete(callback);
	}
	public registerOnChildMovedCallback(
		callback: (node: TabCollectionNode<View>) => void,
	): Subscription {
		this.childMovedCallbacks.add(callback);
		return () => this.childMovedCallbacks.delete(callback);
	}
	public registerOnActiveTabChangedCallback(
		callback: (tabId: string | null) => void,
	): Subscription {
		this.activeTabChangedCallbacks.add(callback);
		return () => this.activeTabChangedCallbacks.delete(callback);
	}
	private updateFromModel(): void {
		const previous = this.snapshot;
		const next = this.model.getSnapshot();
		const previousDescriptor = buildTabCollectionDescriptor(previous);
		const nextDescriptor = buildTabCollectionDescriptor(next);
		const previousPositions = descriptorPositions(previousDescriptor);
		const nextPositions = descriptorPositions(nextDescriptor);
		this.reconcileNode(this.root, nextDescriptor);
		if ([...nextPositions.keys()].some((handle) => !previousPositions.has(handle))) {
			this.childrenAddedCallbacks.forEach((callback) => callback());
		}
		if ([...previousPositions.keys()].some((handle) => !nextPositions.has(handle))) {
			this.childRemovedCallbacks.forEach((callback) => callback());
		}
		nextPositions.forEach((position, handle) => {
			const old = previousPositions.get(handle);
			if (!old || (old.parent === position.parent && old.index === position.index)) return;
			const node = this.root.getNodeForHandle(handle);
			if (node) this.childMovedCallbacks.forEach((callback) => callback(node));
		});
		if (previous.activeTabId !== next.activeTabId) {
			this.activeTabChangedCallbacks.forEach((callback) => callback(next.activeTabId));
		}
		const changedSelections = new Set([
			...setsDiffer(previous.selectedTabIds, next.selectedTabIds),
			...setsDiffer(next.selectedTabIds, previous.selectedTabIds),
		]);
		if (previous.activeTabId !== next.activeTabId) {
			if (previous.activeTabId) changedSelections.add(previous.activeTabId);
			if (next.activeTabId) changedSelections.add(next.activeTabId);
		}
		changedSelections.forEach((id) =>
			this.root.getNodeForHandle(`tab:${id}`)?.notifyTabSelectionChanged(),
		);
		this.snapshot = next;
	}
	private reconcileNode(
		parent: TabCollectionNode<View>,
		desired: TabCollectionNodeDescriptor,
	): void {
		const desiredChildren = desired.children ?? [];
		desiredChildren.forEach((childDescriptor, index) => {
			let child =
				parent.children.find((candidate) => candidate.handle === childDescriptor.handle) ?? null;
			if (child) {
				const currentIndex = parent.children.indexOf(child);
				if (currentIndex !== index) parent.moveChild(child.handle, index);
			} else {
				const existing = this.root.getNodeForHandle(childDescriptor.handle);
				const sourceParent = existing ? this.root.getParentNodeForHandle(existing.handle) : null;
				if (existing && sourceParent && existing.type === childDescriptor.type) {
					const pinned = this.root.getNodeForHandle('collection:pinned');
					const sourcePinned = Boolean(pinned?.getNodeForHandle(sourceParent.handle));
					const destinationPinned = Boolean(pinned?.getNodeForHandle(parent.handle));
					if (sourcePinned !== destinationPinned) {
						sourceParent.removeChild(existing.handle, true);
						child = parent.addNewChild(childDescriptor, index, false);
					} else {
						child = TabCollectionNode.moveChildAcrossParents(
							existing.handle,
							index,
							sourceParent,
							parent,
						);
					}
				} else {
					child = parent.addNewChild(childDescriptor, index, false);
				}
			}
			if (child) this.reconcileNode(child, childDescriptor);
		});
		const desiredHandles = new Set(desiredChildren.map((child) => child.handle));
		[...parent.children].forEach((child) => {
			if (!desiredHandles.has(child.handle)) parent.removeChild(child.handle, true);
		});
	}
}
