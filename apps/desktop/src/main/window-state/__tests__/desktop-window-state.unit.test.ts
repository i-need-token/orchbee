import { describe, expect, it } from 'vitest';

import {
	createDesktopWindowState,
	parseDesktopWindowState,
	resolveDesktopWindowLaunchState,
} from '../desktop-window-state';

describe('desktop window state', () => {
	it('uses the unpositioned product default when no valid state exists', () => {
		expect(
			resolveDesktopWindowLaunchState(null, {
				primaryWorkArea: { height: 900, width: 1_440, x: 0, y: 0 },
				workAreas: [{ height: 900, width: 1_440, x: 0, y: 0 }],
			}),
		).toEqual({ bounds: { height: 720, width: 1_024 }, maximized: false });
	});

	it('preserves a visible normal rectangle and its maximized intent', () => {
		const state = createDesktopWindowState({ height: 700, width: 1_100, x: 120, y: 80 }, true);

		expect(
			resolveDesktopWindowLaunchState(state, {
				primaryWorkArea: { height: 900, width: 1_440, x: 0, y: 0 },
				workAreas: [{ height: 900, width: 1_440, x: 0, y: 0 }],
			}),
		).toEqual({ bounds: { height: 700, width: 1_100, x: 120, y: 80 }, maximized: true });
	});

	it('chooses the display with greatest overlap including negative coordinates', () => {
		const state = createDesktopWindowState({ height: 700, width: 1_000, x: -900, y: 100 }, false);

		expect(
			resolveDesktopWindowLaunchState(state, {
				primaryWorkArea: { height: 900, width: 1_440, x: 0, y: 0 },
				workAreas: [
					{ height: 1_080, width: 1_920, x: -1_920, y: 0 },
					{ height: 900, width: 1_440, x: 0, y: 0 },
				],
			}),
		).toEqual({ bounds: { height: 700, width: 1_000, x: -1_000, y: 100 }, maximized: false });
	});

	it('rehomes a removed-display rectangle and keeps it fully visible', () => {
		const state = createDesktopWindowState({ height: 800, width: 1_200, x: 3_000, y: -400 }, false);

		expect(
			resolveDesktopWindowLaunchState(state, {
				primaryWorkArea: { height: 860, width: 1_400, x: 20, y: 40 },
				workAreas: [{ height: 860, width: 1_400, x: 20, y: 40 }],
			}),
		).toEqual({ bounds: { height: 800, width: 1_200, x: 220, y: 40 }, maximized: false });
	});

	it('shrinks oversize geometry at every work-area edge', () => {
		const state = createDesktopWindowState(
			{ height: 2_000, width: 3_000, x: -100, y: -200 },
			false,
		);

		expect(
			resolveDesktopWindowLaunchState(state, {
				primaryWorkArea: { height: 700, width: 900, x: -50, y: 30 },
				workAreas: [{ height: 700, width: 900, x: -50, y: 30 }],
			}),
		).toEqual({ bounds: { height: 700, width: 900, x: -50, y: 30 }, maximized: false });
	});

	it('strictly rejects invalid, unknown, and full-screen persistence fields', () => {
		expect(parseDesktopWindowState({ maximized: false, version: 1 })).toBeNull();
		expect(
			parseDesktopWindowState({
				bounds: { height: 720, width: 1_024, x: 0, y: 0 },
				fullScreen: true,
				maximized: false,
				version: 1,
			}),
		).toBeNull();
		expect(
			parseDesktopWindowState({
				bounds: { height: 100, width: 100, x: 0, y: 0 },
				maximized: false,
				version: 1,
			}),
		).toBeNull();
		expect(createDesktopWindowState({ height: 720, width: 1_024, x: 1.5, y: 0 }, false)).toBeNull();
	});
});
