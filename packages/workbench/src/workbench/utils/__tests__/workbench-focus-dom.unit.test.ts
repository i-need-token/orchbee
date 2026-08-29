// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

import {
	isWorkbenchExclusiveFocusTarget,
	resolveWorkbenchFocusRegionFromTarget,
} from '../workbench-focus-dom';

describe('Workbench focus DOM boundaries', () => {
	it('accepts only descendants of a known semantic region marker', () => {
		const region = document.createElement('section');
		const child = document.createElement('button');
		region.dataset['workbenchFocusRegion'] = 'tab-strip';
		region.append(child);

		expect(resolveWorkbenchFocusRegionFromTarget(child)).toBe('tab-strip');
		region.dataset['workbenchFocusRegion'] = 'unknown';
		expect(resolveWorkbenchFocusRegionFromTarget(child)).toBeNull();
		expect(resolveWorkbenchFocusRegionFromTarget(null)).toBeNull();
	});

	it('recognizes product keyboard captures and shared exclusive surfaces', () => {
		const dialog = document.createElement('div');
		const dialogChild = document.createElement('input');
		dialog.role = 'dialog';
		dialog.append(dialogChild);
		const capture = document.createElement('div');
		capture.dataset['workbenchKeyboardCapture'] = 'true';
		const ordinary = document.createElement('button');

		expect(isWorkbenchExclusiveFocusTarget(dialogChild)).toBe(true);
		expect(isWorkbenchExclusiveFocusTarget(capture)).toBe(true);
		expect(isWorkbenchExclusiveFocusTarget(ordinary)).toBe(false);
		expect(isWorkbenchExclusiveFocusTarget(null)).toBe(false);
	});
});
