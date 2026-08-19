import { describe, expect, it } from 'vitest';

import { measureMountedTerminalGeometry } from '../measureMountedTerminalGeometry';
import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '../presentation-configuration';

describe('measureMountedTerminalGeometry', () => {
	it('uses renderer font metrics, mounted padding, and device pixels for initial geometry', async () => {
		const root = createMountedRoot({
			clientHeight: 600,
			clientWidth: 900,
			devicePixelRatio: 2,
			padding: { bottom: '18px', left: '9px', right: '9px', top: '18px' },
		});

		await expect(
			measureMountedTerminalGeometry(root, DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION),
		).resolves.toEqual({
			cellPixelHeight: 36,
			cellPixelWidth: 18,
			columns: 98,
			rows: 31,
		});
	});

	it('does not invent geometry for an unmounted viewport', async () => {
		const root = createMountedRoot({
			clientHeight: 0,
			clientWidth: 900,
			devicePixelRatio: 2,
			padding: { bottom: '0px', left: '0px', right: '0px', top: '0px' },
		});

		await expect(
			measureMountedTerminalGeometry(root, DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION),
		).resolves.toBeUndefined();
	});
});

function createMountedRoot(options: {
	readonly clientHeight: number;
	readonly clientWidth: number;
	readonly devicePixelRatio: number;
	readonly padding: {
		readonly bottom: string;
		readonly left: string;
		readonly right: string;
		readonly top: string;
	};
}): HTMLElement {
	const context = {
		direction: 'ltr',
		font: '',
		measureText: () =>
			({
				actualBoundingBoxAscent: 14,
				actualBoundingBoxDescent: 4,
				fontBoundingBoxAscent: 14,
				fontBoundingBoxDescent: 4,
				width: 9,
			}) as TextMetrics,
		textRendering: 'auto',
	};
	const document = {
		createElement: () => ({ getContext: () => context }),
		defaultView: {
			devicePixelRatio: options.devicePixelRatio,
			getComputedStyle: () => ({
				paddingBottom: options.padding.bottom,
				paddingLeft: options.padding.left,
				paddingRight: options.padding.right,
				paddingTop: options.padding.top,
			}),
		},
	} as unknown as Document;
	return {
		clientHeight: options.clientHeight,
		clientWidth: options.clientWidth,
		ownerDocument: document,
	} as unknown as HTMLElement;
}
