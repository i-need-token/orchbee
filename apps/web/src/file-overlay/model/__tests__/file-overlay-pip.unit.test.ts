import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { FileOverlayBox, FileOverlayRegions } from '../fileOverlayCover';
import {
	FILE_OVERLAY_PIP_CORNERS,
	fileOverlayPipBox,
	fileOverlayPipPosition,
	moveFileOverlayPip,
	type FileOverlayPipPosition,
} from '../file-overlay-pip';

/** A window laid out as the Workbench lays it: Sidebar, terminal area, File Bar, below the row. */
const regionsArbitrary = fc
	.record({
		fileBar: fc.integer({ max: 900, min: 0 }),
		height: fc.integer({ max: 2000, min: 1 }),
		row: fc.integer({ max: 80, min: 0 }),
		sidebar: fc.integer({ max: 600, min: 0 }),
		terminal: fc.integer({ max: 3000, min: 1 }),
	})
	.map(
		({ fileBar, height, row, sidebar, terminal }): FileOverlayRegions => ({
			sidebar: sidebar === 0 ? null : { left: 0, right: sidebar },
			terminal: { height, left: sidebar, top: row, width: terminal },
			width: sidebar + terminal + fileBar,
		}),
	);

const positionArbitrary: fc.Arbitrary<FileOverlayPipPosition | null> = fc.option(
	fc.record({
		corner: fc.constantFrom(...FILE_OVERLAY_PIP_CORNERS),
		x: fc.integer({ max: 5000, min: 0 }),
		y: fc.integer({ max: 5000, min: 0 }),
	}),
	{ nil: null },
);

/** Whether `box` lies inside the window below the 36px row. */
function insideWindow(regions: FileOverlayRegions, box: FileOverlayBox): boolean {
	const { terminal } = regions;
	return (
		box.left >= 0 &&
		box.top >= terminal.top &&
		box.left + box.width <= regions.width + 1e-9 &&
		box.top + box.height <= terminal.top + terminal.height + 1e-9
	);
}

const WIDE: FileOverlayRegions = {
	sidebar: { left: 0, right: 300 },
	terminal: { height: 864, left: 300, top: 36, width: 1000 },
	width: 1600,
};

describe('picture-in-picture geometry', () => {
	it('stays inside the window below the 36px row at 40% of the terminal area, at least 200 × 120', () => {
		fc.assert(
			fc.property(regionsArbitrary, positionArbitrary, (regions, position) => {
				const box = fileOverlayPipBox(regions, position);
				expect(insideWindow(regions, box)).toBe(true);
				const { terminal } = regions;
				expect(box.width).toBe(Math.min(regions.width, Math.max(200, terminal.width * 0.4)));
				expect(box.height).toBe(Math.min(terminal.height, Math.max(120, terminal.height * 0.4)));
			}),
		);
	});

	it('first appears 24 px inside the terminal area’s bottom-right corner', () => {
		expect(fileOverlayPipBox(WIDE, null)).toEqual({
			height: 345.6,
			left: 300 + 1000 - 24 - 400,
			top: 36 + 864 - 24 - 345.6,
			width: 400,
		});
	});

	it('keeps a drag inside the window below the 36px row', () => {
		fc.assert(
			fc.property(
				regionsArbitrary,
				positionArbitrary,
				fc.integer({ max: 10_000, min: -10_000 }),
				fc.integer({ max: 10_000, min: -10_000 }),
				(regions, position, deltaX, deltaY) => {
					const origin = fileOverlayPipBox(regions, position);
					const moved = moveFileOverlayPip(regions, origin, deltaX, deltaY);
					expect(insideWindow(regions, moved)).toBe(true);
					expect(moved.width).toBe(origin.width);
					expect(moved.height).toBe(origin.height);
				},
			),
		);
		const origin = fileOverlayPipBox(WIDE, null);
		expect(moveFileOverlayPip(WIDE, origin, -5000, -5000)).toMatchObject({ left: 0, top: 36 });
	});

	it('puts picture-in-picture back where a drag left it, within half a pixel', () => {
		fc.assert(
			fc.property(
				regionsArbitrary,
				fc.integer({ max: 10_000, min: -10_000 }),
				fc.integer({ max: 10_000, min: -10_000 }),
				(regions, deltaX, deltaY) => {
					const left = moveFileOverlayPip(
						regions,
						fileOverlayPipBox(regions, null),
						deltaX,
						deltaY,
					);
					const restored = fileOverlayPipBox(regions, fileOverlayPipPosition(regions, left));
					expect(Math.abs(restored.left - left.left)).toBeLessThanOrEqual(0.5);
					expect(Math.abs(restored.top - left.top)).toBeLessThanOrEqual(0.5);
				},
			),
		);
	});

	it('keeps to its nearest corner as the window changes', () => {
		const nearTopLeft = moveFileOverlayPip(WIDE, fileOverlayPipBox(WIDE, null), -1000, -800);
		const moved = moveFileOverlayPip(WIDE, nearTopLeft, 40, 30);
		const position = fileOverlayPipPosition(WIDE, moved);
		expect(position).toEqual({ corner: 'top-left', x: 40, y: 30 });

		const nearBottomRight = fileOverlayPipPosition(WIDE, fileOverlayPipBox(WIDE, null));
		expect(nearBottomRight.corner).toBe('bottom-right');
		const narrower: FileOverlayRegions = {
			sidebar: null,
			terminal: { height: 600, left: 0, top: 36, width: 900 },
			width: 1200,
		};
		const box = fileOverlayPipBox(narrower, nearBottomRight);
		expect(1200 - box.left - box.width).toBe(nearBottomRight.x);
		expect(36 + 600 - box.top - box.height).toBe(nearBottomRight.y);
		expect(fileOverlayPipBox(narrower, position)).toMatchObject({ left: 40, top: 66 });
	});
});
