import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { fileOverlayCover, type FileOverlayRegions } from '../fileOverlayCover';

/** A window laid out as the Workbench lays it: Sidebar, divider, terminal area, File Bar. */
const regionsArbitrary = fc
	.record({
		divider: fc.integer({ max: 8, min: 0 }),
		fileBar: fc.integer({ max: 900, min: 0 }),
		height: fc.integer({ max: 2000, min: 1 }),
		row: fc.integer({ max: 80, min: 0 }),
		sidebar: fc.option(fc.integer({ max: 600, min: 1 }), { nil: null }),
		terminal: fc.integer({ max: 3000, min: 1 }),
	})
	.map(({ divider, fileBar, height, row, sidebar, terminal }): FileOverlayRegions => {
		const terminalLeft = sidebar === null ? 0 : sidebar + divider;
		return {
			sidebar: sidebar === null ? null : { left: 0, right: sidebar },
			terminal: { height, left: terminalLeft, top: row, width: terminal },
			width: terminalLeft + terminal + divider + fileBar,
		};
	});

const tierArbitrary = fc.constantFrom('default', 'full', 'left-expand');

describe('fileOverlayCover', () => {
	it('starts every tier below the 36px row and reaches the window bottom', () => {
		fc.assert(
			fc.property(regionsArbitrary, tierArbitrary, (regions, tier) => {
				const cover = fileOverlayCover(regions, tier);
				expect(cover.top).toBe(regions.terminal.top);
				expect(cover.height).toBe(regions.terminal.height);
			}),
		);
	});

	it('covers only what each tier names and never the File Bar below full', () => {
		fc.assert(
			fc.property(regionsArbitrary, (regions) => {
				const terminalRight = regions.terminal.left + regions.terminal.width;
				expect(fileOverlayCover(regions, 'default')).toEqual(regions.terminal);
				const leftExpand = fileOverlayCover(regions, 'left-expand');
				expect(leftExpand.left).toBe(regions.sidebar?.left ?? regions.terminal.left);
				expect(leftExpand.left + leftExpand.width).toBe(terminalRight);
				const full = fileOverlayCover(regions, 'full');
				expect(full.left).toBe(0);
				expect(full.width).toBe(regions.width);
			}),
		);
	});
});
