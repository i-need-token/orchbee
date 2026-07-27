import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_ACTION_IDS,
	workbenchTabStripOrientationInput,
	workbenchTerminalCreateInput,
} from '../workbench-action-ids';

describe('workbench action contracts', () => {
	it('owns stable semantic action ids', () => {
		expect(WORKBENCH_ACTION_IDS).toEqual({
			createTerminal: 'workbench.terminal.create',
			openSearch: 'workbench.search.open',
			openSettings: 'workbench.settings.open',
			setTabStripOrientation: 'workbench.tab-strip.orientation.set',
			toggleSidebar: 'workbench.sidebar.toggle',
		});
	});

	it('accepts only supported tab-strip orientations', () => {
		expect(workbenchTabStripOrientationInput.parse({ orientation: 'horizontal' })).toEqual({
			success: true,
			data: { orientation: 'horizontal' },
		});
		expect(workbenchTabStripOrientationInput.parse({ orientation: 'diagonal' })).toEqual({
			success: false,
			code: 'expected-tab-strip-orientation',
		});
	});

	it('defaults terminal creation to the ungrouped trailing position and validates placement', () => {
		expect(workbenchTerminalCreateInput.parse(undefined)).toEqual({
			success: true,
			data: { groupId: null },
		});
		expect(workbenchTerminalCreateInput.parse({ groupId: 'group-1', index: 2 })).toMatchObject({
			success: true,
		});
		expect(workbenchTerminalCreateInput.parse({ groupId: null, index: -1 })).toMatchObject({
			success: false,
		});
	});
});
