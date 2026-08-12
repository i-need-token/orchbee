import { describe, expect, it } from 'vitest';

import {
	beginHistoryScrollbarDrag,
	historyScrollbarContainsPoint,
	projectHistoryScrollbar,
	resolveHistoryScrollbarDrag,
} from '../history-scrollbar';

describe('projected history scrollbar', () => {
	it('projects the mature hit area and proportional thumb geometry', () => {
		const scrollbar = projectHistoryScrollbar({
			opacity: 1,
			totalRows: 100n,
			viewportHeight: 100,
			viewportWidth: 100,
			visibleRows: 20,
			visibleStartRow: 40n,
		});

		expect(scrollbar).toEqual({
			hitArea: { height: 100, width: 16, x: 84, y: 0 },
			maximumStartRow: 80n,
			thumb: { height: 20, opacity: 0.5, width: 8, x: 88, y: 40 },
			track: { height: 92, travel: 72, y: 4 },
			visibleStartRow: 40n,
		});
		expect(scrollbar && historyScrollbarContainsPoint(scrollbar, 84, 0)).toBe(true);
		expect(scrollbar && historyScrollbarContainsPoint(scrollbar, 83.999, 50)).toBe(false);
		expect(scrollbar && historyScrollbarContainsPoint(scrollbar, 100.001, 50)).toBe(false);
	});

	it('clamps start rows, opacity, and the minimum thumb to the usable track', () => {
		const bottom = projectHistoryScrollbar({
			opacity: 2,
			totalRows: 100n,
			viewportHeight: 100,
			viewportWidth: 100,
			visibleRows: 20,
			visibleStartRow: 500n,
		});
		const short = projectHistoryScrollbar({
			opacity: -1,
			totalRows: 10n,
			viewportHeight: 16,
			viewportWidth: 10,
			visibleRows: 1,
			visibleStartRow: -1n,
		});

		expect(bottom).toMatchObject({
			thumb: { height: 20, opacity: 0.3, y: 76 },
			visibleStartRow: 80n,
		});
		expect(short).toEqual({
			hitArea: { height: 16, width: 10, x: 0, y: 0 },
			maximumStartRow: 9n,
			thumb: { height: 8, opacity: 0, width: 8, x: 0, y: 4 },
			track: { height: 8, travel: 0, y: 4 },
			visibleStartRow: 0n,
		});
	});

	it('centers proportional track jumps and drags from the jumped intent', () => {
		const scrollbar = projectHistoryScrollbar({
			opacity: 0,
			totalRows: 100n,
			viewportHeight: 100,
			viewportWidth: 100,
			visibleRows: 20,
			visibleStartRow: 80n,
		});
		if (scrollbar === null) throw new Error('scrollbar projection is unavailable');

		const jump = beginHistoryScrollbarDrag(scrollbar, 50);
		expect(jump.targetStartRow).toBe(40n);
		expect(resolveHistoryScrollbarDrag(scrollbar, jump.drag, 68)).toBe(60n);
		expect(resolveHistoryScrollbarDrag(scrollbar, jump.drag, -500)).toBe(0n);
		expect(resolveHistoryScrollbarDrag(scrollbar, jump.drag, 500)).toBe(80n);

		const thumbPress = beginHistoryScrollbarDrag(scrollbar, 80);
		expect(thumbPress.targetStartRow).toBe(80n);
	});

	it('returns no interactive projection without retained history or usable geometry', () => {
		expect(
			projectHistoryScrollbar({
				opacity: 1,
				totalRows: 20n,
				viewportHeight: 100,
				viewportWidth: 100,
				visibleRows: 20,
				visibleStartRow: 0n,
			}),
		).toBeNull();
		expect(
			projectHistoryScrollbar({
				opacity: 1,
				totalRows: 21n,
				viewportHeight: 8,
				viewportWidth: 100,
				visibleRows: 20,
				visibleStartRow: 0n,
			}),
		).toBeNull();
		expect(
			projectHistoryScrollbar({
				opacity: 1,
				totalRows: 21n,
				viewportHeight: Number.NaN,
				viewportWidth: 100,
				visibleRows: 20,
				visibleStartRow: 0n,
			}),
		).toBeNull();
		expect(
			projectHistoryScrollbar({
				opacity: 1,
				totalRows: 21n,
				viewportHeight: 100,
				viewportWidth: 100,
				visibleRows: Number.NaN,
				visibleStartRow: 0n,
			}),
		).toBeNull();
	});
});
