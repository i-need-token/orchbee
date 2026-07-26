import { describe, expect, it } from 'vitest';
import type { TabModelSnapshot } from '../../../../../../model/tab-model';
import type { VerticalPinAnimation } from '../../model/VerticalPinAnimationRuntime';
import { buildVerticalPinAnimationEntries } from '../buildVerticalPinAnimationEntries';

const current: TabModelSnapshot = {
	revision: 1,
	tabs: [
		{ id: 'pin', title: 'Pin', pinned: true, group: null },
		{ id: 'other', title: 'Other', pinned: false, group: null },
	],
	groups: [],
	activeTabId: 'pin',
	selectedTabIds: new Set(['pin']),
	selectionAnchorId: 'pin',
	mostRecentGroupId: null,
};

const pinning: VerticalPinAnimation = {
	tabId: 'pin',
	direction: 'pinning',
	sourceTab: { id: 'pin', title: 'Pin', pinned: false, group: null },
	sourceGroup: null,
	sourceIndex: 0,
	progress: 0.5,
	sourceMainAxisScale: 0.5,
	sourceOpacity: 0.5,
	destinationMainAxisScale: 0.5,
	destinationOpacity: 0.5,
};

describe('buildVerticalPinAnimationEntries', () => {
	it('projects one logical pin move into concurrent source and destination entries', () => {
		const entries = buildVerticalPinAnimationEntries({
			snapshot: current,
			animations: [pinning],
		});

		expect(entries.pinned).toEqual([
			expect.objectContaining({
				key: 'tab:pin',
				phase: 'destination',
				mainAxisScale: 0.5,
				opacity: 0.5,
			}),
		]);
		expect(entries.unpinned).toEqual([
			expect.objectContaining({ key: 'pin-source:pin', phase: 'source', mainAxisScale: 0.5 }),
			expect.objectContaining({ key: 'tab:other', phase: 'stable' }),
		]);
	});
});
