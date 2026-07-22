export { TabSlotAnimationDelegate, type TabSlotAnimationHost } from './TabSlotAnimationDelegate';
export { TabSlotViewState } from './TabSlotViewState';
export { HOVER_TAB_TRANSITION_DELAY_MS, HoverTabSelector } from './hover-tab-selector';
export { moveContiguousBlock, type ContiguousBlockMove } from './moveContiguousBlock';
export {
	TabCollectionNodeType,
	MemoryTabCollectionViewAdapter,
	TabCollectionNode,
	buildTabCollectionDescriptor,
	RootTabCollectionNode,
	type TabCollectionNodeDescriptor,
	type TabCollectionMemoryView,
	type TabCollectionViewAdapter,
} from './tab-collection-node';
export {
	calculateDraggedTabsInsertion,
	TabModel,
	type TabAlertState,
	type TabData,
	type TabGroupColor,
	type TabGroupMember,
	type TabGroupActivity,
	type TabGroupData,
	type TabModelInit,
	type TabModelSnapshot,
	type TabDragRestoreState,
	type DetachedTabBundle,
	type DraggedTabsInsertionInput,
	type DraggedTabsInsertionResult,
} from './tab-model';
export { TabOpen, TabPinned, TabActive, type SplitTabId, type TabGroupId } from './tab-types';
