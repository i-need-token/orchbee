import { describe, expect, it } from 'vitest';
import { DropGroupInclusion, DropRelativeToIndex } from '../../shared';
import {
	DragPositionHint,
	getHorizontalLinkDropIndexForTab,
	getLinkDropIndexForNode,
} from '../tab-link-drop-handler';

describe('TabLinkDropHandler', () => {
	it('replaces a tab when dropped over its center', () => {
		expect(
			getLinkDropIndexForNode(
				{ type: 'tab', index: 3, groupId: null, isLastTabInGroup: false },
				null,
			),
		).toEqual({
			index: 3,
			relativeToIndex: DropRelativeToIndex.ReplaceIndex,
			groupInclusion: DropGroupInclusion.DontIncludeInGroup,
		});
	});

	it('includes edge insertions in a group except after its last tab', () => {
		expect(
			getLinkDropIndexForNode(
				{ type: 'tab', index: 3, groupId: 'group', isLastTabInGroup: false },
				DragPositionHint.Before,
			).groupInclusion,
		).toBe(DropGroupInclusion.IncludeInGroup);
		expect(
			getLinkDropIndexForNode(
				{ type: 'tab', index: 4, groupId: 'group', isLastTabInGroup: true },
				DragPositionHint.After,
			),
		).toEqual({
			index: 5,
			relativeToIndex: DropRelativeToIndex.InsertBeforeIndex,
			groupInclusion: DropGroupInclusion.DontIncludeInGroup,
		});
	});

	it('maps expanded and collapsed group headers to Chromium insertion semantics', () => {
		expect(
			getLinkDropIndexForNode({ type: 'group', start: 2, end: 5, collapsed: false }, null)
				.groupInclusion,
		).toBe(DropGroupInclusion.IncludeInGroup);
		expect(
			getLinkDropIndexForNode(
				{ type: 'group', start: 2, end: 5, collapsed: false },
				DragPositionHint.Before,
			).groupInclusion,
		).toBe(DropGroupInclusion.DontIncludeInGroup);
		expect(
			getLinkDropIndexForNode(
				{ type: 'group', start: 2, end: 5, collapsed: true },
				DragPositionHint.After,
			),
		).toEqual({
			index: 5,
			relativeToIndex: DropRelativeToIndex.InsertBeforeIndex,
			groupInclusion: DropGroupInclusion.DontIncludeInGroup,
		});
	});

	it('handles split, pinned, and unpinned collection nodes', () => {
		expect(
			getLinkDropIndexForNode({ type: 'split', start: 2, length: 2 }, DragPositionHint.After).index,
		).toBe(4);
		expect(getLinkDropIndexForNode({ type: 'pinned', firstNonPinnedIndex: 3 }, null).index).toBe(3);
		expect(getLinkDropIndexForNode({ type: 'unpinned', tabCount: 9 }, null).index).toBe(9);
	});

	it('keeps the horizontal TabContainerImpl group-edge rules separate', () => {
		expect(getHorizontalLinkDropIndexForTab(2, DragPositionHint.Before, true).groupInclusion).toBe(
			DropGroupInclusion.IncludeInGroup,
		);
		expect(getHorizontalLinkDropIndexForTab(3, DragPositionHint.Before, false).groupInclusion).toBe(
			DropGroupInclusion.DontIncludeInGroup,
		);
		expect(getHorizontalLinkDropIndexForTab(2, DragPositionHint.After, true)).toEqual({
			index: 3,
			relativeToIndex: DropRelativeToIndex.InsertBeforeIndex,
			groupInclusion: DropGroupInclusion.DontIncludeInGroup,
		});
	});
});
