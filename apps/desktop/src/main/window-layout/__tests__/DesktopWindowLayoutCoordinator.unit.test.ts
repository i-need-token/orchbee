import {
	DesktopWindowLayoutDestinationSchema,
	DesktopWindowLayoutDragSessionIdSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopWindowLayoutCoordinator,
	type DesktopWindowLayoutHost,
} from '../DesktopWindowLayoutCoordinator';
import type { DesktopWindowLayout, DesktopWindowLayoutStore } from '../desktop-window-layout';
import { DesktopWindowLayoutOwner } from '../desktop-window-layout-owner';

const bounds = { height: 720, width: 1_024, x: 20, y: 30 } as const;
const documentFixture = {
	terminalContexts: [
		{
			tabIds: ['tab-1', 'tab-2', 'tab-3'],
			tabs: [
				{ group: 'group-1', id: 'tab-1' },
				{ group: null, id: 'tab-2' },
				{ group: 'group-1', id: 'tab-3' },
			],
			terminalContextId: 'context-1',
		},
	],
} as const;

describe('DesktopWindowLayoutCoordinator', () => {
	it('keeps source ownership unchanged when native preparation fails', async () => {
		const layout = twoWindowLayout();
		const store = createStore();
		const host = createHost();
		host.prepareWindow.mockResolvedValue(false);
		const coordinator = createCoordinator(layout, store, host);

		await expect(
			coordinator.moveTabsToNewWindow('desktop-window-1', 'context-1', ['tab-1']),
		).resolves.toEqual({ code: 'native-window-unavailable', kind: 'failed' });

		expect(coordinator.layout).toEqual(layout);
		expect(store.save).not.toHaveBeenCalled();
		expect(host.discardPreparedWindow).not.toHaveBeenCalled();
	});

	it('discards a prepared shell and restores source ownership when persistence fails', async () => {
		const layout = twoWindowLayout();
		const store = createStore();
		store.save.mockResolvedValue(false);
		const host = createHost();
		const coordinator = createCoordinator(layout, store, host);

		await expect(
			coordinator.moveTabsToNewWindow('desktop-window-1', 'context-1', ['tab-1']),
		).resolves.toEqual({ code: 'persistence', kind: 'failed' });

		expect(coordinator.layout).toEqual(layout);
		expect(host.prepareWindow).toHaveBeenCalledOnce();
		expect(host.discardPreparedWindow).toHaveBeenCalledWith('desktop-window-3');
	});

	it('moves every fragment of a group without restarting its terminal ownership', async () => {
		const store = createStore();
		const host = createHost();
		const coordinator = createCoordinator(twoWindowLayout(), store, host);
		const controller = coordinator.createBrokerController('desktop-window-1');

		await expect(
			controller.moveGroup(
				'context-1',
				'group-1',
				DesktopWindowLayoutDestinationSchema.parse({
					kind: 'existing',
					windowId: 'desktop-window-2',
				}),
			),
		).resolves.toEqual({ destinationWindowId: 'desktop-window-2', kind: 'completed' });

		expect(coordinator.layout.windows[0]?.assignments[0]?.tabIds).toEqual(['tab-2']);
		expect(coordinator.layout.windows[1]?.assignments[0]?.tabIds).toEqual(['tab-1', 'tab-3']);
		expect(host.activateWindow).toHaveBeenCalledWith('desktop-window-2');
		expect(host.retireWindow).not.toHaveBeenCalled();
	});

	it('settles an external drag into the hit-tested window and rehomes a closed window to MRU', async () => {
		const store = createStore();
		const host = createHost();
		host.getWindowIdAtPoint.mockReturnValue('desktop-window-2');
		const coordinator = createCoordinator(twoWindowLayout(), store, host);
		const controller = coordinator.createBrokerController('desktop-window-1');
		const dragId = DesktopWindowLayoutDragSessionIdSchema.parse('drag-1');
		controller.startDrag(dragId, 'context-1', ['tab-1']);

		expect(controller.updateDrag(dragId, { x: 1_200, y: 100 })).toEqual({
			kind: 'existing',
			windowId: 'desktop-window-2',
		});
		await expect(controller.completeDrag(dragId, { x: 1_200, y: 100 })).resolves.toEqual({
			destinationWindowId: 'desktop-window-2',
			kind: 'completed',
		});
		await expect(coordinator.requestWindowClose('desktop-window-1')).resolves.toEqual({
			destinationWindowId: 'desktop-window-2',
			kind: 'completed',
		});

		expect(coordinator.layout.windows).toHaveLength(1);
		expect(coordinator.layout.windows[0]?.assignments[0]?.tabIds).toEqual([
			'tab-1',
			'tab-2',
			'tab-3',
		]);
		expect(host.retireWindow).toHaveBeenCalledWith('desktop-window-1');
		expect(host.activateWindow).toHaveBeenLastCalledWith('desktop-window-2');
	});
});

function createCoordinator(
	layout: DesktopWindowLayout,
	store: ReturnType<typeof createStore>,
	host: ReturnType<typeof createHost>,
): DesktopWindowLayoutCoordinator {
	return new DesktopWindowLayoutCoordinator(
		new DesktopWindowLayoutOwner(store, documentFixture, layout),
		documentFixture,
		host,
	);
}

function createHost() {
	return {
		activateWindow: vi.fn<DesktopWindowLayoutHost['activateWindow']>(),
		discardPreparedWindow: vi
			.fn<DesktopWindowLayoutHost['discardPreparedWindow']>()
			.mockResolvedValue(undefined),
		getNewWindowBounds: vi
			.fn<DesktopWindowLayoutHost['getNewWindowBounds']>()
			.mockReturnValue({ ...bounds, x: 52, y: 62 }),
		getWindowIdAtPoint: vi
			.fn<DesktopWindowLayoutHost['getWindowIdAtPoint']>()
			.mockReturnValue(null),
		prepareWindow: vi.fn<DesktopWindowLayoutHost['prepareWindow']>().mockResolvedValue(true),
		retireWindow: vi.fn<DesktopWindowLayoutHost['retireWindow']>(),
	} satisfies DesktopWindowLayoutHost;
}

function createStore() {
	return {
		flush: vi.fn<DesktopWindowLayoutStore['flush']>().mockResolvedValue(undefined),
		load: vi.fn<DesktopWindowLayoutStore['load']>().mockResolvedValue({ kind: 'missing' }),
		save: vi.fn<DesktopWindowLayoutStore['save']>().mockResolvedValue(true),
	};
}

function twoWindowLayout(): DesktopWindowLayout {
	return {
		activationOrder: ['desktop-window-1', 'desktop-window-2'],
		nextWindowSequence: 3,
		primaryWindowId: 'desktop-window-1',
		version: 1,
		windows: [
			{
				assignments: [{ tabIds: ['tab-1', 'tab-2'], terminalContextId: 'context-1' }],
				bounds,
				maximized: false,
				ordinal: 1,
				windowId: 'desktop-window-1',
			},
			{
				assignments: [{ tabIds: ['tab-3'], terminalContextId: 'context-1' }],
				bounds: { ...bounds, x: 1_100 },
				maximized: false,
				ordinal: 2,
				windowId: 'desktop-window-2',
			},
		],
	};
}
