import { DropGroupInclusion, DropRelativeToIndex, type TabDropIndex } from '../shared';

export const DragPositionHint = {
	Before: 'before',
	After: 'after',
} as const;

export type DragPositionHint = (typeof DragPositionHint)[keyof typeof DragPositionHint];

export type TabLinkDropNode =
	| {
			type: 'tab';
			index: number;
			groupId: string | null;
			isLastTabInGroup: boolean;
	  }
	| {
			type: 'group';
			start: number;
			end: number;
			collapsed: boolean;
	  }
	| {
			type: 'split';
			start: number;
			length: number;
	  }
	| {
			type: 'pinned';
			firstNonPinnedIndex: number;
	  }
	| {
			type: 'unpinned';
			tabCount: number;
	  };

function insertion(
	index: number,
	groupInclusion: DropGroupInclusion = DropGroupInclusion.DontIncludeInGroup,
): TabDropIndex {
	return {
		index,
		relativeToIndex: DropRelativeToIndex.InsertBeforeIndex,
		groupInclusion,
	};
}

/** Port of horizontal TabContainerImpl::GetDropIndex's tab branch. */
export function getHorizontalLinkDropIndexForTab(
	index: number,
	positionHint: DragPositionHint | null,
	firstInGroup: boolean,
): TabDropIndex {
	if (positionHint === null) {
		return {
			index,
			relativeToIndex: DropRelativeToIndex.ReplaceIndex,
			groupInclusion: DropGroupInclusion.DontIncludeInGroup,
		};
	}
	if (positionHint === DragPositionHint.Before) {
		return insertion(
			index,
			firstInGroup ? DropGroupInclusion.IncludeInGroup : DropGroupInclusion.DontIncludeInGroup,
		);
	}
	return insertion(index + 1);
}

/** Pure port of common/TabLinkDropHandler::GetDropIndexForNode. */
export function getLinkDropIndexForNode(
	node: Readonly<TabLinkDropNode>,
	positionHint: DragPositionHint | null,
): TabDropIndex {
	if (node.type === 'tab') {
		if (positionHint === null) {
			return {
				index: node.index,
				relativeToIndex: DropRelativeToIndex.ReplaceIndex,
				groupInclusion: DropGroupInclusion.DontIncludeInGroup,
			};
		}
		const includeInGroup =
			node.groupId !== null && !(positionHint === DragPositionHint.After && node.isLastTabInGroup);
		return insertion(
			node.index + (positionHint === DragPositionHint.After ? 1 : 0),
			includeInGroup ? DropGroupInclusion.IncludeInGroup : DropGroupInclusion.DontIncludeInGroup,
		);
	}

	if (node.type === 'group') {
		const index = positionHint === DragPositionHint.After ? node.end : node.start;
		if (node.collapsed) return insertion(index);
		return insertion(
			index,
			positionHint === null
				? DropGroupInclusion.IncludeInGroup
				: DropGroupInclusion.DontIncludeInGroup,
		);
	}

	if (node.type === 'split') {
		return insertion(node.start + (positionHint === DragPositionHint.After ? node.length : 0));
	}
	if (node.type === 'pinned') return insertion(node.firstNonPinnedIndex);
	return insertion(node.tabCount);
}
