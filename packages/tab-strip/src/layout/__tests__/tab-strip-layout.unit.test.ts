import { requireDefined } from '../../common';
import { describe, expect, it } from 'vitest';
import { rectRight, type Rect } from '../../geometry';
import { TabActive, TabOpen, TabPinned, type TabGroupId } from '../../model';
import { TabLayoutState } from '../TabLayoutState';
import { calculateTabBounds } from '../tab-strip-layout';
import type { TabSizeInfo } from '../tab-strip-layout-types';
import { TabWidthConstraints } from '../TabWidthConstraints';
interface TestCase {
	numPinnedTabs?: number;
	numTabs?: number;
	activeIndex?: number;
	tabstripWidth?: number;
	tabGroups?: readonly (TabGroupId | null)[];
	focusedGroup?: TabGroupId | null;
	showPinnedTabsInFocusedGroups?: boolean;
}
const STANDARD_WIDTH = 256;
const TAB_HEIGHT = 41;
const MIN_ACTIVE_WIDTH = 56;
const MIN_INACTIVE_WIDTH = 32;
const PINNED_WIDTH = 64;
const normalSize: TabSizeInfo = {
	pinnedTabWidth: PINNED_WIDTH,
	minActiveWidth: MIN_ACTIVE_WIDTH,
	minInactiveWidth: MIN_INACTIVE_WIDTH,
	standardWidth: STANDARD_WIDTH,
};
function runTestCase(testCase: TestCase): Rect[] {
	const numPinnedTabs = testCase.numPinnedTabs ?? 0;
	const numTabs = testCase.numTabs ?? 0;
	const activeIndex = testCase.activeIndex ?? 0;
	const focusedGroup = testCase.focusedGroup ?? null;
	const tabGroups = testCase.tabGroups ?? [];
	const constraints = Array.from(
		{
			length: numTabs,
		},
		(_, tabIndex) => {
			let collapsed = false;
			if (focusedGroup !== null) {
				if (testCase.showPinnedTabsInFocusedGroups && tabIndex < numPinnedTabs) {
					collapsed = false;
				} else {
					collapsed = (tabGroups[tabIndex] ?? null) !== focusedGroup;
				}
			}
			return new TabWidthConstraints(
				new TabLayoutState(
					collapsed ? TabOpen.Closed : TabOpen.Open,
					tabIndex < numPinnedTabs ? TabPinned.Pinned : TabPinned.Unpinned,
					tabIndex === activeIndex ? TabActive.Active : TabActive.Inactive,
				),
				normalSize,
			);
		},
	);
	return calculateTabBounds(constraints, testCase.tabstripWidth ?? 0).bounds;
}
const widths = (bounds: readonly Rect[]) => bounds.map((rect) => rect.width);
const xs = (bounds: readonly Rect[]) => bounds.map((rect) => rect.x);
function expectFill(bounds: readonly Rect[], width: number): void {
	expect(rectRight(requireDefined(bounds.at(-1)))).toBe(width);
}
describe('calculateTabBounds (Chromium tab_strip_layout_unittest.cc)', () => {
	it('uses standard dimensions when space is available', () => {
		const bounds = runTestCase({
			tabstripWidth: 1000,
			numTabs: 3,
		});
		expect(widths(bounds)).toEqual([256, 256, 256]);
		expect(xs(bounds)).toEqual([0, 238, 476]);
		expect(bounds.every((rect) => rect.y === 0 && rect.height === TAB_HEIGHT)).toBe(true);
		expect(rectRight(requireDefined(bounds.at(-1)))).toBeLessThan(1000);
	});
	it.each([
		[
			'all pinned',
			{
				tabstripWidth: 1000,
				numTabs: 3,
				numPinnedTabs: 3,
			},
			[64, 64, 64],
			[0, 46, 92],
		],
		[
			'mixed pinned and normal',
			{
				tabstripWidth: 1000,
				numTabs: 3,
				numPinnedTabs: 1,
			},
			[64, 256, 256],
			[0, 46, 284],
		],
	] as const)('lays out %s tabs', (_name, testCase, expectedWidths, expectedXs) => {
		const bounds = runTestCase(testCase);
		expect(widths(bounds)).toEqual(expectedWidths);
		expect(xs(bounds)).toEqual(expectedXs);
		expect(rectRight(requireDefined(bounds.at(-1)))).toBeLessThan(testCase.tabstripWidth);
	});
	it.each([
		[
			'middle width',
			{
				tabstripWidth: 598,
				numTabs: 4,
			},
			[163, 163, 163, 163],
			[0, 145, 290, 435],
		],
		[
			'middle width with pinned',
			{
				tabstripWidth: 400,
				numTabs: 3,
				numPinnedTabs: 1,
			},
			[64, 186, 186],
			[0, 46, 214],
		],
		[
			'rounded middle width',
			{
				tabstripWidth: 600,
				numTabs: 4,
			},
			[164, 164, 163, 163],
			[0, 146, 292, 437],
		],
		[
			'rounded middle width with pinned',
			{
				tabstripWidth: 401,
				numTabs: 3,
				numPinnedTabs: 1,
			},
			[64, 187, 186],
			[0, 46, 215],
		],
	] as const)('fills the strip at %s', (_name, testCase, expectedWidths, expectedXs) => {
		const bounds = runTestCase(testCase);
		expect(widths(bounds)).toEqual(expectedWidths);
		expect(xs(bounds)).toEqual(expectedXs);
		expectFill(bounds, testCase.tabstripWidth);
	});
	it.each([
		[
			'below active minimum',
			{
				tabstripWidth: 196,
				numTabs: 6,
				activeIndex: 3,
			},
			[46, 46, 46, 56, 46, 46],
		],
		[
			'rounded below active minimum',
			{
				tabstripWidth: 200,
				numTabs: 6,
				activeIndex: 3,
			},
			[47, 47, 47, 56, 47, 46],
		],
		[
			'active pinned below minimum',
			{
				tabstripWidth: 249,
				numTabs: 6,
				numPinnedTabs: 1,
			},
			[64, 55, 55, 55, 55, 55],
		],
		[
			'inactive pinned below minimum',
			{
				tabstripWidth: 250,
				numTabs: 6,
				numPinnedTabs: 1,
				activeIndex: 2,
			},
			[64, 55, 56, 55, 55, 55],
		],
		[
			'rounded active pinned below minimum',
			{
				tabstripWidth: 250,
				numTabs: 6,
				numPinnedTabs: 1,
			},
			[64, 56, 55, 55, 55, 55],
		],
	] as const)('matches Chromium pixel allocation for %s', (_name, testCase, expectedWidths) => {
		const bounds = runTestCase(testCase);
		expect(widths(bounds)).toEqual(expectedWidths);
		expectFill(bounds, testCase.tabstripWidth);
	});
	it.each([
		[
			'normal tabs',
			{
				tabstripWidth: 10,
				numTabs: 3,
			},
			[56, 32, 32],
		],
		[
			'one tab',
			{
				tabstripWidth: 15,
				numTabs: 1,
			},
			[56],
		],
		[
			'all pinned tabs',
			{
				tabstripWidth: 10,
				numTabs: 3,
				numPinnedTabs: 3,
			},
			[64, 64, 64],
		],
		[
			'mixed pinned and normal',
			{
				tabstripWidth: 10,
				numTabs: 3,
				numPinnedTabs: 1,
			},
			[64, 32, 32],
		],
	] as const)('never shrinks %s below minimum', (_name, testCase, expectedWidths) => {
		const bounds = runTestCase(testCase);
		expect(widths(bounds)).toEqual(expectedWidths);
		expect(rectRight(requireDefined(bounds.at(-1)))).toBeGreaterThan(testCase.tabstripWidth);
	});
	it('exactly fits fixed-width pinned tabs', () => {
		const width = 2 * PINNED_WIDTH - 18;
		const bounds = runTestCase({
			tabstripWidth: width,
			numTabs: 2,
			numPinnedTabs: 2,
		});
		expect(widths(bounds)).toEqual([64, 64]);
		expectFill(bounds, width);
	});
	it.each([
		[
			'trailing groups',
			['group-1', 'group-1', 'group-2', 'group-2'],
			'group-1',
			0,
			[256, 256, 18, 18],
			[0, 238, 476, 476],
		],
		[
			'leading groups',
			['group-1', 'group-1', 'group-2', 'group-2'],
			'group-2',
			2,
			[18, 18, 256, 256],
			[0, 0, 0, 238],
		],
	] as const)(
		'collapses non-focused %s',
		(_name, tabGroups, focusedGroup, activeIndex, expectedWidths, expectedXs) => {
			const bounds = runTestCase({
				tabstripWidth: 1000,
				numTabs: 4,
				activeIndex,
				tabGroups,
				focusedGroup,
			});
			expect(widths(bounds)).toEqual(expectedWidths);
			expect(xs(bounds)).toEqual(expectedXs);
		},
	);
	it('does not collapse grouped tabs without a focused group', () => {
		const bounds = runTestCase({
			tabstripWidth: 1000,
			numTabs: 4,
			tabGroups: ['group-1', 'group-1', 'group-2', 'group-2'],
		});
		expect(widths(bounds)).toEqual([256, 256, 256, 256]);
		expect(xs(bounds)).toEqual([0, 238, 476, 714]);
	});
	it.each([
		[false, [18, 18, 256, 18], [0, 0, 0, 238]],
		[true, [64, 64, 256, 18], [0, 46, 92, 330]],
	] as const)(
		'honors the show-pinned-tabs-in-focused-groups feature when %s',
		(showPinnedTabsInFocusedGroups, expectedWidths, expectedXs) => {
			const bounds = runTestCase({
				tabstripWidth: 1000,
				numTabs: 4,
				numPinnedTabs: 2,
				tabGroups: [null, null, 'group-1', 'group-2'],
				focusedGroup: 'group-1',
				showPinnedTabsInFocusedGroups,
			});
			expect(widths(bounds)).toEqual(expectedWidths);
			expect(xs(bounds)).toEqual(expectedXs);
		},
	);
});
