export { TabSlotAnimationDelegate, type TabSlotAnimationHost } from './TabSlotAnimationDelegate';
export { TabSlotViewState } from './TabSlotViewState';
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
	type TabGroupData,
	type TabModelInit,
	type TabModelSnapshot,
	type TabDragRestoreState,
	type DetachedTabBundle,
	type DraggedTabsInsertionInput,
	type DraggedTabsInsertionResult,
} from './tab-model';
export { TabOpen, TabPinned, TabActive, type TabGroupId } from './tab-types';
