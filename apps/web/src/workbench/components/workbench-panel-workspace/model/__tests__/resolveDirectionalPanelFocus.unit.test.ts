import { describe, expect, it } from 'vitest';

import {
	createSnapshotRuntime,
	fixturePanelId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
	type FixtureRecord,
} from '../../../../../__tests__/server-workbench-snapshot';
import type { PanelLayoutNode } from '../panel-workspace-state';
import {
	resolveDirectionalPanelFocus,
	type PanelFocusDirection,
} from '../resolveDirectionalPanelFocus';

describe('resolveDirectionalPanelFocus', () => {
	it('traverses a four-Panel grid by visual direction without wrapping at an outer edge', () => {
		const root = layout(
			snapshotSplit(
				1,
				'horizontal',
				snapshotSplit(2, 'vertical', snapshotPanel(1), snapshotPanel(4)),
				snapshotSplit(3, 'vertical', snapshotPanel(2), snapshotPanel(3)),
			),
		);

		expect(resolve(root, 4, 'up')).toBe(fixturePanelId(1));
		expect(resolve(root, 1, 'right')).toBe(fixturePanelId(2));
		expect(resolve(root, 2, 'down')).toBe(fixturePanelId(3));
		expect(resolve(root, 3, 'left')).toBe(fixturePanelId(4));
		expect(resolve(root, 1, 'up')).toBeNull();
		expect(resolve(root, 1, 'left')).toBeNull();
		expect(resolve(root, 3, 'right')).toBeNull();
		expect(resolve(root, 4, 'down')).toBeNull();
	});

	it('uses perpendicular center distance and then visual order for multiple adjacent candidates', () => {
		const threePanels = (rightRatio?: number) =>
			layout(
				snapshotSplit(
					1,
					'horizontal',
					snapshotPanel(1),
					snapshotSplit(2, 'vertical', snapshotPanel(2), snapshotPanel(3), rightRatio),
				),
			);

		expect(resolve(threePanels(), 1, 'right')).toBe(fixturePanelId(2));
		expect(resolve(threePanels(0.25), 1, 'right')).toBe(fixturePanelId(3));
	});

	it('returns null for a missing source and a singleton layout', () => {
		const root = layout(snapshotPanel(1));

		expect(resolveDirectionalPanelFocus(root, 'missing-panel', 'right')).toBeNull();
		expect(resolve(root, 1, 'right')).toBeNull();
	});
});

function resolve(
	root: PanelLayoutNode,
	panelSequence: number,
	direction: PanelFocusDirection,
): string | null {
	return resolveDirectionalPanelFocus(root, fixturePanelId(panelSequence), direction);
}

/** The Panel layout a window shows when the Server publishes one Tab holding `root`. */
function layout(root: FixtureRecord): PanelLayoutNode {
	const tab = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1, root)]))
		.getActiveTerminalContext()
		.panelWorkspaceStore.getSnapshot().tabs[0];
	if (tab === undefined) throw new Error('Expected the published Tab.');
	return tab.root;
}
