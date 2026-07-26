import { describe, expect, it } from 'vitest';
import {
	calculateHorizontalAlertCloseGap,
	calculateHorizontalPinnedControlOffset,
	calculateHorizontalTabChildVisibility,
	calculateVerticalTabChildVisibility,
} from '../tab-content-layout';

describe('tab child visibility ports', () => {
	it('removes the extra alert padding on a narrow active tab', () => {
		expect(calculateHorizontalAlertCloseGap(68)).toBe(8);
		expect(calculateHorizontalAlertCloseGap(67)).toBe(4);
		expect(calculateHorizontalAlertCloseGap(100, true)).toBe(12);
		expect(calculateHorizontalAlertCloseGap(99, true)).toBe(4);
	});

	it('gives pinned alerts priority over favicons and never shows close', () => {
		expect(
			calculateHorizontalTabChildVisibility({
				width: 64,
				contentsWidth: 28,
				active: true,
				pinned: true,
				hasFavicon: true,
				hasAlert: true,
			}),
		).toEqual({
			showFavicon: false,
			showAlert: true,
			showClose: false,
			showTitle: false,
			centerSingleControl: true,
		});
	});

	it('keeps wide pinning tabs in normal content placement until the compact threshold', () => {
		expect(
			calculateHorizontalTabChildVisibility({
				width: 120,
				contentsWidth: 84,
				active: true,
				pinned: true,
				pinTransition: 'pinning',
				hasFavicon: true,
				hasAlert: false,
				pinnedWidth: 64,
			}),
		).toEqual({
			showFavicon: true,
			showAlert: false,
			showClose: false,
			showTitle: true,
			centerSingleControl: false,
		});
	});

	it('retains pinned child priority for the full unpin bounds animation', () => {
		expect(
			calculateHorizontalTabChildVisibility({
				width: 80,
				contentsWidth: 44,
				active: true,
				pinned: false,
				pinTransition: 'unpinning',
				hasFavicon: true,
				hasAlert: true,
				pinnedWidth: 64,
			}),
		).toEqual({
			showFavicon: false,
			showAlert: true,
			showClose: false,
			showTitle: true,
			centerSingleControl: false,
		});
	});

	it('moves the pinned control continuously over the final 30 DIP', () => {
		const input = {
			pinnedWidth: 64,
			controlWidth: 16,
			normalControlX: 18,
		};
		expect(calculateHorizontalPinnedControlOffset({ ...input, width: 94 })).toBe(0);
		expect(calculateHorizontalPinnedControlOffset({ ...input, width: 79 })).toBe(3);
		expect(calculateHorizontalPinnedControlOffset({ ...input, width: 64 })).toBe(6);
	});

	it('gives close priority on a narrow active tab without waiting for hover', () => {
		const base = {
			width: 72,
			contentsWidth: 36,
			active: true,
			pinned: false,
			declutterEnabled: true,
			hasFavicon: true,
			hasAlert: true,
		} as const;
		expect(calculateHorizontalTabChildVisibility(base)).toMatchObject({
			showFavicon: false,
			showAlert: true,
			showClose: true,
		});
		expect(calculateHorizontalTabChildVisibility({ ...base, hovered: true })).toMatchObject({
			showFavicon: false,
			showAlert: true,
			showClose: true,
		});
	});

	it('shows only a centered close control at the minimum active width', () => {
		expect(
			calculateHorizontalTabChildVisibility({
				width: 56,
				contentsWidth: 16,
				active: true,
				pinned: false,
				declutterEnabled: true,
				hasFavicon: true,
				hasAlert: false,
			}),
		).toEqual({
			showFavicon: false,
			showAlert: false,
			showClose: true,
			showTitle: false,
			centerSingleControl: true,
		});
	});

	it('keeps a compact active media alert ahead of close until interaction', () => {
		const base = {
			width: 56,
			contentsWidth: 16,
			active: true,
			pinned: false,
			declutterEnabled: true,
			hasFavicon: true,
			hasAlert: true,
		} as const;
		expect(calculateHorizontalTabChildVisibility(base)).toMatchObject({
			showAlert: true,
			showClose: false,
			showFavicon: false,
		});
		expect(calculateHorizontalTabChildVisibility({ ...base, hovered: true })).toMatchObject({
			showAlert: false,
			showClose: true,
			showFavicon: false,
		});
	});

	it('uses the touch close width when deciding whether a favicon still fits', () => {
		const base = {
			width: 80,
			contentsWidth: 40,
			active: true,
			pinned: false,
			declutterEnabled: true,
			hasFavicon: true,
			hasAlert: false,
		} as const;
		expect(calculateHorizontalTabChildVisibility(base).showFavicon).toBe(true);
		expect(calculateHorizontalTabChildVisibility({ ...base, touchUi: true }).showFavicon).toBe(
			false,
		);
	});

	it('hides inactive close in decluttered width and restores it on hover', () => {
		const base = {
			width: 120,
			contentsWidth: 84,
			active: false,
			pinned: false,
			declutterEnabled: true,
			hasFavicon: true,
			hasAlert: false,
		} as const;
		expect(calculateHorizontalTabChildVisibility(base).showClose).toBe(false);
		expect(calculateHorizontalTabChildVisibility({ ...base, hovered: true }).showClose).toBe(true);
	});

	it('suppresses close controls when the host locks the tab strip', () => {
		expect(
			calculateHorizontalTabChildVisibility({
				width: 120,
				contentsWidth: 84,
				active: true,
				pinned: false,
				hasFavicon: true,
				hasAlert: false,
				closeAllowed: false,
			}).showClose,
		).toBe(false);
		expect(
			calculateVerticalTabChildVisibility({
				width: 32,
				active: true,
				pinned: false,
				collapsed: true,
				hovered: true,
				hasFavicon: true,
				hasAlert: false,
				closeAllowed: false,
			}).showClose,
		).toBe(false);
	});

	it('hides all horizontal children below the minimum inactive width', () => {
		expect(
			calculateHorizontalTabChildVisibility({
				width: 31,
				contentsWidth: 0,
				active: true,
				pinned: false,
				hasFavicon: true,
				hasAlert: true,
			}),
		).toEqual({
			showFavicon: false,
			showAlert: false,
			showClose: false,
			showTitle: false,
			centerSingleControl: false,
		});
	});

	it('centers only the highest-priority visible child in a collapsed vertical tab', () => {
		const base = {
			width: 32,
			active: false,
			pinned: false,
			collapsed: true,
			hasFavicon: true,
			hasAlert: true,
		} as const;
		expect(calculateVerticalTabChildVisibility(base)).toEqual({
			showFavicon: false,
			showAlert: true,
			showClose: false,
			showTitle: false,
			centerSingleControl: true,
		});
		expect(
			calculateVerticalTabChildVisibility({ ...base, active: true, hovered: true }),
		).toMatchObject({
			showClose: true,
			showAlert: false,
			showFavicon: false,
		});
	});

	it('lays out all applicable children in an expanded vertical tab', () => {
		expect(
			calculateVerticalTabChildVisibility({
				width: 216,
				active: true,
				pinned: false,
				collapsed: false,
				hasFavicon: true,
				hasAlert: true,
			}),
		).toEqual({
			showFavicon: true,
			showAlert: true,
			showClose: true,
			showTitle: true,
			centerSingleControl: false,
		});
	});
});
