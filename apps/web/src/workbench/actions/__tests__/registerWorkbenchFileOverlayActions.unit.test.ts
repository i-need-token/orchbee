import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import { FileOverlayRegistry } from '../../../file-overlay';
import { registerWorkbenchFileOverlayActions } from '../registerWorkbenchFileOverlayActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

const WORKSPACE_ID = 'workspace-1';

/** The shown Workspace's overlay with a tab for each path, opened in order, and its Actions. */
function createHarness(paths: readonly string[]) {
	const registry = new FileOverlayRegistry(true);
	const store = registry.store(WORKSPACE_ID);
	for (const path of paths) {
		store.dispatch({
			input: { kind: 'file', resource: { key: path, path } },
			preview: false,
			type: 'file-opened',
		});
	}
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	registerWorkbenchFileOverlayActions({
		dispatch: (event) => {
			store.dispatch(event);
		},
		focusOverlay: () => undefined,
		getState: () => store.state.getState(),
		pickTab: () => Promise.resolve(null),
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		workspaceId: WORKSPACE_ID,
	});
	const invocation = { scope, source: 'keyboard' } as const;
	return {
		activePath: (): string | undefined => {
			const { tabs } = store.state.getState();
			return tabs.tabs.find((tab) => tab.id === tabs.activeId)?.input.resource.path;
		},
		// The runtime resolves a hidden Action to nothing, as it does one it does not know.
		availability: (actionId: string, input?: unknown): string =>
			runtime.resolveAction(actionId, input, invocation)?.availability.kind ?? 'hidden',
		run: async (actionId: string, input?: unknown): Promise<void> => {
			await runtime.execute(actionId, input, invocation).completion;
		},
	};
}

describe('registerWorkbenchFileOverlayActions', () => {
	it('offers a tab Action only while it has a tab to act on, so its keys otherwise pass on', async () => {
		const empty = createHarness([]);
		expect(
			[
				WORKBENCH_ACTION_IDS.pinFileOverlayTab,
				WORKBENCH_ACTION_IDS.reopenFileOverlayTab,
				WORKBENCH_ACTION_IDS.selectLastFileOverlayTab,
				WORKBENCH_ACTION_IDS.selectNextFileOverlayTab,
				WORKBENCH_ACTION_IDS.selectPreviousFileOverlayTab,
				WORKBENCH_ACTION_IDS.unpinFileOverlayTab,
			].filter((actionId) => empty.availability(actionId) !== 'hidden'),
		).toEqual([]);
		expect(
			empty.availability(WORKBENCH_ACTION_IDS.selectFileOverlayTabByPosition, { position: 1 }),
		).toBe('hidden');

		const one = createHarness(['/a.ts']);
		expect(one.availability(WORKBENCH_ACTION_IDS.selectNextFileOverlayTab)).toBe('hidden');
		expect(one.availability(WORKBENCH_ACTION_IDS.selectPreviousFileOverlayTab)).toBe('hidden');
		expect(one.availability(WORKBENCH_ACTION_IDS.selectLastFileOverlayTab)).toBe('enabled');
		expect(
			one.availability(WORKBENCH_ACTION_IDS.selectFileOverlayTabByPosition, { position: 1 }),
		).toBe('enabled');
		expect(
			one.availability(WORKBENCH_ACTION_IDS.selectFileOverlayTabByPosition, { position: 2 }),
		).toBe('hidden');

		// Pin Editor and Unpin Editor share their keys, so they are never offered together.
		expect(one.availability(WORKBENCH_ACTION_IDS.pinFileOverlayTab)).toBe('enabled');
		expect(one.availability(WORKBENCH_ACTION_IDS.unpinFileOverlayTab)).toBe('hidden');
		await one.run(WORKBENCH_ACTION_IDS.pinFileOverlayTab);
		expect(one.availability(WORKBENCH_ACTION_IDS.pinFileOverlayTab)).toBe('hidden');
		expect(one.availability(WORKBENCH_ACTION_IDS.unpinFileOverlayTab)).toBe('enabled');

		// Reopen Closed Editor waits for a closed tab, even once the overlay has closed with it.
		const closing = createHarness(['/a.ts']);
		expect(closing.availability(WORKBENCH_ACTION_IDS.reopenFileOverlayTab)).toBe('hidden');
		await closing.run(WORKBENCH_ACTION_IDS.closeFileOverlayTab);
		expect(closing.activePath()).toBeUndefined();
		expect(closing.availability(WORKBENCH_ACTION_IDS.reopenFileOverlayTab)).toBe('enabled');
		await closing.run(WORKBENCH_ACTION_IDS.reopenFileOverlayTab);
		expect(closing.activePath()).toBe('/a.ts');
		expect(closing.availability(WORKBENCH_ACTION_IDS.reopenFileOverlayTab)).toBe('hidden');
	});

	it('goes round the row with Next and Previous, as VS Code does in one editor group', async () => {
		const harness = createHarness(['/a.ts', '/b.ts', '/c.ts']);
		expect(harness.activePath()).toBe('/c.ts');
		await harness.run(WORKBENCH_ACTION_IDS.selectNextFileOverlayTab);
		expect(harness.activePath()).toBe('/a.ts');
		await harness.run(WORKBENCH_ACTION_IDS.selectPreviousFileOverlayTab);
		expect(harness.activePath()).toBe('/c.ts');
		await harness.run(WORKBENCH_ACTION_IDS.selectPreviousFileOverlayTab);
		expect(harness.activePath()).toBe('/b.ts');
		await harness.run(WORKBENCH_ACTION_IDS.selectFileOverlayTabByPosition, { position: 1 });
		expect(harness.activePath()).toBe('/a.ts');
		await harness.run(WORKBENCH_ACTION_IDS.selectLastFileOverlayTab);
		expect(harness.activePath()).toBe('/c.ts');
	});
});
