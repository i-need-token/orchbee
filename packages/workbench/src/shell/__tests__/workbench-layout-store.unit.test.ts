import { describe, expect, it } from 'vitest';

import { createWorkbenchLayoutStore } from '../workbench-layout-store';

describe('createWorkbenchLayoutStore', () => {
	it('tracks session selection, split orientation, and focused pane', () => {
		const store = createWorkbenchLayoutStore('session-1');

		store.getState().splitWithSession('session-2', 'vertical');

		expect(store.getState()).toMatchObject({
			activeSessionId: 'session-1',
			splitSessionId: 'session-2',
			splitOrientation: 'vertical',
			focusedPane: 'secondary',
		});

		store.getState().selectSession('session-2');
		expect(store.getState().focusedPane).toBe('primary');
	});

	it('reconciles layout state after sessions close', () => {
		const store = createWorkbenchLayoutStore('session-1');
		store.getState().splitWithSession('session-2', 'horizontal');

		store.getState().reconcileSessions(['session-3']);

		expect(store.getState()).toMatchObject({
			activeSessionId: 'session-3',
			splitSessionId: undefined,
			focusedPane: 'primary',
		});
	});
});
