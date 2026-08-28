import { describe, expect, it, vi } from 'vitest';

import type {
	DesktopWindowLayout,
	DesktopWindowLayoutDocument,
	DesktopWindowLayoutStore,
} from '../desktop-window-layout';
import {
	DesktopWindowLayoutOwner,
	loadDesktopWindowLayoutOwner,
} from '../desktop-window-layout-owner';

const bounds = { height: 720, width: 1_024, x: 0, y: 0 } as const;
const documentFixture = {
	terminalContexts: [{ tabIds: ['tab-1', 'tab-2', 'tab-3'], terminalContextId: 'context-1' }],
} satisfies DesktopWindowLayoutDocument;

describe('DesktopWindowLayoutOwner', () => {
	it('loads missing state as one durable primary layout', async () => {
		const store = createStore({ kind: 'missing' });

		const owner = await loadDesktopWindowLayoutOwner(store, documentFixture, bounds);

		expect(owner.getSnapshot().windows).toHaveLength(1);
		expect(owner.getSnapshot().windows[0]?.assignments[0]?.tabIds).toEqual([
			'tab-1',
			'tab-2',
			'tab-3',
		]);
		expect(store.save).toHaveBeenCalledOnce();
	});

	it('keeps the source on every rejected or failed move', async () => {
		const layout = twoWindowLayout();
		const store = createStore({ kind: 'available', layout });
		vi.mocked(store.save).mockResolvedValue(false);
		const owner = new DesktopWindowLayoutOwner(store, documentFixture, layout);
		const listener = vi.fn<() => void>();
		owner.subscribe(listener);

		const result = await owner.moveTabs(['tab-1'], {
			kind: 'existing',
			windowId: 'desktop-window-2',
		});

		expect(result).toEqual({ kind: 'failed', reason: 'persistence' });
		expect(owner.getSnapshot()).toEqual(layout);
		expect(listener).not.toHaveBeenCalled();
	});

	it('serializes commits so a later close observes the committed move', async () => {
		const layout = twoWindowLayout();
		const store = createStore({ kind: 'available', layout });
		const firstSave = deferred<boolean>();
		vi.mocked(store.save).mockImplementationOnce(() => firstSave.promise);
		const owner = new DesktopWindowLayoutOwner(store, documentFixture, layout);

		const move = owner.moveTabs(['tab-1'], {
			kind: 'existing',
			windowId: 'desktop-window-2',
		});
		const close = owner.closeWindow('desktop-window-1');
		await Promise.resolve();
		expect(store.save).toHaveBeenCalledOnce();
		firstSave.resolve(true);

		expect((await move).kind).toBe('completed');
		expect((await close).kind).toBe('completed');
		expect(store.save).toHaveBeenCalledTimes(2);
		expect(owner.getSnapshot().windows).toHaveLength(1);
		expect(owner.getSnapshot().windows[0]?.assignments[0]?.tabIds).toEqual([
			'tab-1',
			'tab-2',
			'tab-3',
		]);
	});

	it('publishes safe document reconciliation even when preference persistence fails', async () => {
		const layout = twoWindowLayout();
		const store = createStore({ kind: 'available', layout });
		vi.mocked(store.save).mockResolvedValue(false);
		const owner = new DesktopWindowLayoutOwner(store, documentFixture, layout);
		const listener = vi.fn<() => void>();
		owner.subscribe(listener);
		const nextDocument = {
			terminalContexts: [
				{
					tabIds: ['tab-1', 'tab-2', 'tab-3', 'tab-4'],
					terminalContextId: 'context-1',
				},
			],
		} satisfies DesktopWindowLayoutDocument;

		const result = await owner.reconcileDocument(nextDocument, bounds);

		expect(result).toEqual({
			changed: true,
			kind: 'completed',
			persisted: false,
			recovered: false,
		});
		expect(owner.getSnapshot().windows[1]?.assignments[0]?.tabIds).toEqual(['tab-3', 'tab-4']);
		expect(listener).toHaveBeenCalledOnce();
	});

	it('publishes focus despite a recoverable write failure and flushes in order', async () => {
		const layout = twoWindowLayout();
		const store = createStore({ kind: 'available', layout });
		vi.mocked(store.save).mockResolvedValue(false);
		const owner = new DesktopWindowLayoutOwner(store, documentFixture, layout);

		const result = await owner.focusWindow('desktop-window-1');
		await owner.flush();

		expect(result.kind).toBe('completed');
		if (result.kind !== 'completed') return;
		expect(result.persisted).toBe(false);
		expect(owner.getSnapshot().activationOrder).toEqual(['desktop-window-2', 'desktop-window-1']);
		expect(store.flush).toHaveBeenCalledOnce();
	});
});

function createStore(
	loadResult: Awaited<ReturnType<DesktopWindowLayoutStore['load']>>,
): DesktopWindowLayoutStore {
	return {
		flush: vi.fn<DesktopWindowLayoutStore['flush']>().mockResolvedValue(undefined),
		load: vi.fn<DesktopWindowLayoutStore['load']>().mockResolvedValue(loadResult),
		save: vi.fn<DesktopWindowLayoutStore['save']>().mockResolvedValue(true),
	};
}

function deferred<Value>(): {
	readonly promise: Promise<Value>;
	resolve(value: Value): void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
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
