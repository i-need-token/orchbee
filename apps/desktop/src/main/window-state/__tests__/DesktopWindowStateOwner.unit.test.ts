import { EventEmitter } from 'node:events';

import { describe, expect, it, vi } from 'vitest';

import {
	createDesktopWindowState,
	type DesktopWindowBounds,
	type DesktopWindowState,
	type DesktopWindowStateStore,
} from '../desktop-window-state';
import { DesktopWindowStateOwner, type DesktopWindowStatePort } from '../DesktopWindowStateOwner';

describe('DesktopWindowStateOwner', () => {
	it('loads and safely projects the last valid state', async () => {
		const state = requiredState({ height: 700, width: 1_100, x: 3_000, y: 100 }, true);
		const store = createStore({ kind: 'available', state });
		const owner = new DesktopWindowStateOwner(store, 'darwin');

		expect(
			await owner.loadLaunchState({
				primaryWorkArea: { height: 800, width: 1_200, x: 0, y: 0 },
				workAreas: [{ height: 800, width: 1_200, x: 0, y: 0 }],
			}),
		).toEqual({ bounds: { height: 700, width: 1_100, x: 100, y: 100 }, maximized: true });
	});

	it('captures Linux geometry and maximize events in observation order', async () => {
		const store = createStore({ kind: 'missing' });
		const owner = new DesktopWindowStateOwner(store, 'linux');
		const window = new TestWindow();
		const attachment = owner.attach(window);

		window.bounds = { height: 650, width: 950, x: 10, y: 20 };
		window.emit('move');
		window.maximized = true;
		window.emit('maximize');
		window.fullScreen = true;
		window.bounds = { height: 700, width: 1_000, x: 30, y: 40 };
		window.emit('resize');
		await owner.flush();

		expect(store.save).toHaveBeenCalledTimes(2);
		expect(store.save).toHaveBeenNthCalledWith(
			1,
			requiredState({ height: 650, width: 950, x: 10, y: 20 }, false),
		);
		expect(store.save).toHaveBeenNthCalledWith(
			2,
			requiredState({ height: 650, width: 950, x: 10, y: 20 }, true),
		);
		expect(store.flush).toHaveBeenCalledOnce();
		attachment.dispose();
	});

	it('uses completed geometry events off Linux and removes every listener on disposal', async () => {
		const store = createStore({ kind: 'missing' });
		const owner = new DesktopWindowStateOwner(store, 'darwin');
		const window = new TestWindow();
		const attachment = owner.attach(window);

		window.emit('move');
		window.emit('resize');
		window.emit('moved');
		window.emit('resized');
		window.emit('close');
		attachment.dispose();
		window.emit('moved');
		await owner.flush();

		expect(store.save).toHaveBeenCalledTimes(3);
		expect(window.eventNames()).toEqual([]);
	});

	it('continues after a failed save and flushes the later capture', async () => {
		const store = createStore({ kind: 'missing' });
		vi.mocked(store.save).mockResolvedValueOnce(false).mockResolvedValueOnce(true);
		const owner = new DesktopWindowStateOwner(store, 'linux');
		const window = new TestWindow();
		owner.attach(window);

		window.emit('move');
		window.bounds = { height: 760, width: 1_200, x: 50, y: 60 };
		window.emit('resize');
		await owner.flush();

		expect(store.save).toHaveBeenCalledTimes(2);
		expect(store.save).toHaveBeenLastCalledWith(
			requiredState({ height: 760, width: 1_200, x: 50, y: 60 }, false),
		);
	});
});

class TestWindow extends EventEmitter implements DesktopWindowStatePort {
	bounds: DesktopWindowBounds = { height: 600, width: 900, x: 0, y: 0 };
	destroyed = false;
	fullScreen = false;
	maximized = false;

	getNormalBounds(): DesktopWindowBounds {
		return this.bounds;
	}

	isDestroyed(): boolean {
		return this.destroyed;
	}

	isFullScreen(): boolean {
		return this.fullScreen;
	}

	isMaximized(): boolean {
		return this.maximized;
	}
}

function createStore(loadResult: Awaited<ReturnType<DesktopWindowStateStore['load']>>) {
	return {
		flush: vi.fn<DesktopWindowStateStore['flush']>().mockResolvedValue(undefined),
		load: vi.fn<DesktopWindowStateStore['load']>().mockResolvedValue(loadResult),
		save: vi.fn<DesktopWindowStateStore['save']>().mockResolvedValue(true),
	} satisfies DesktopWindowStateStore;
}

function requiredState(bounds: DesktopWindowBounds, maximized: boolean): DesktopWindowState {
	const state = createDesktopWindowState(bounds, maximized);
	if (state === null) throw new Error('Test window state must be valid.');
	return state;
}
