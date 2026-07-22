import { describe, expect, it } from 'vitest';

import {
	createWorkbenchTabWidthLock,
	getWorkbenchTabLockedWidth,
	isWorkbenchTabWidthLockApplicable,
	resolveWorkbenchTabLayoutMode,
} from '../workbench-tab-layout';

const TABS = [
	{ id: 'first', label: 'First' },
	{ groupId: 'group-a', id: 'second', label: 'Second' },
	{ groupId: 'group-a', id: 'third', label: 'Third' },
] as const;

describe('workbench tab layout', () => {
	it('classifies standard, compact, and icon-only widths at stable boundaries', () => {
		const thresholds = { closeVisibilityWidth: 96, iconOnlyWidth: 56 };

		expect(resolveWorkbenchTabLayoutMode(120, thresholds)).toBe('standard');
		expect(resolveWorkbenchTabLayoutMode(96, thresholds)).toBe('compact');
		expect(resolveWorkbenchTabLayoutMode(56, thresholds)).toBe('icon-only');
	});

	it('keeps the captured widths while the remaining topology is an ordered subset', () => {
		const lock = createWorkbenchTabWidthLock(
			TABS,
			[
				{ id: 'first', width: 80 },
				{ id: 'second', width: 80 },
				{ id: 'third', width: 80 },
			],
			0,
		);

		expect(lock).toBeDefined();
		if (lock === undefined) throw new Error('Expected the measured tab layout to create a lock.');
		expect(isWorkbenchTabWidthLockApplicable(lock, TABS.slice(1), 0)).toBe(true);
		expect(getWorkbenchTabLockedWidth(lock, 'second')).toBe(80);
	});

	it('releases the lock for keyboard reset, insertion, reordering, pinning, or regrouping', () => {
		const lock = createWorkbenchTabWidthLock(TABS, [{ id: 'first', width: 80 }], 2);
		if (lock === undefined) throw new Error('Expected the measured tab layout to create a lock.');

		expect(isWorkbenchTabWidthLockApplicable(lock, TABS, 3)).toBe(false);
		expect(
			isWorkbenchTabWidthLockApplicable(lock, [...TABS, { id: 'fourth', label: 'Fourth' }], 2),
		).toBe(false);
		expect(isWorkbenchTabWidthLockApplicable(lock, [TABS[1], TABS[0], TABS[2]], 2)).toBe(false);
		expect(
			isWorkbenchTabWidthLockApplicable(
				lock,
				[{ ...TABS[0], isPinned: true }, ...TABS.slice(1)],
				2,
			),
		).toBe(false);
		expect(
			isWorkbenchTabWidthLockApplicable(
				lock,
				[TABS[0], { ...TABS[1], groupId: undefined }, TABS[2]],
				2,
			),
		).toBe(false);
	});
});
