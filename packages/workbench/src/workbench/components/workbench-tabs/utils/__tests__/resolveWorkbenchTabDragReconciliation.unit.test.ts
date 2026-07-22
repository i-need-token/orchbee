import { createWorkbenchTabsState } from '../../../../../workbench-tabs';
import { describe, expect, it } from 'vitest';

import type { WorkbenchTabDragState } from '../../types/workbench-tab-drag';
import { resolveWorkbenchTabDragReconciliation } from '../resolveWorkbenchTabDragReconciliation';
import { getWorkbenchTabsTopologyKey } from '../workbench-tab-drag-state';

const SOURCE_STATE = createWorkbenchTabsState({
	activeTabId: 'first',
	tabs: [
		{ id: 'first', label: 'First' },
		{ id: 'second', label: 'Second' },
	],
});
const PREVIEW_STATE = createWorkbenchTabsState({
	activeTabId: 'first',
	tabs: [
		{ id: 'second', label: 'Second' },
		{ id: 'first', label: 'First' },
	],
});
const SETTLING_STATE: WorkbenchTabDragState = {
	kind: 'settling',
	previewState: PREVIEW_STATE,
};

describe('resolveWorkbenchTabDragReconciliation', () => {
	it('retains the final preview while the authoritative topology is still the source', () => {
		const sourceTopologyKey = getWorkbenchTabsTopologyKey(SOURCE_STATE);

		expect(
			resolveWorkbenchTabDragReconciliation(SETTLING_STATE, sourceTopologyKey, sourceTopologyKey),
		).toBeUndefined();
	});

	it('acknowledges only the authoritative topology matching the final preview', () => {
		expect(
			resolveWorkbenchTabDragReconciliation(
				SETTLING_STATE,
				getWorkbenchTabsTopologyKey(SOURCE_STATE),
				getWorkbenchTabsTopologyKey(PREVIEW_STATE),
			),
		).toEqual({ kind: 'acknowledge-commit' });
	});

	it('rejects the preview when authority changes to a different topology', () => {
		const rejectedState = createWorkbenchTabsState({
			activeTabId: 'first',
			tabs: [{ id: 'first', label: 'First' }],
		});

		expect(
			resolveWorkbenchTabDragReconciliation(
				SETTLING_STATE,
				getWorkbenchTabsTopologyKey(SOURCE_STATE),
				getWorkbenchTabsTopologyKey(rejectedState),
			),
		).toEqual({ kind: 'stale-input' });
	});
});
