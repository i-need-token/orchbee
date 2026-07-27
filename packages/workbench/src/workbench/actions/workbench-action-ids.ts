import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';
import type { TabStripOrientation } from '@orchbee/tab-strip';

import type { WorkbenchSearchFilter } from '../types/workbench-search';

export const WORKBENCH_ACTION_IDS = {
	createTerminal: 'workbench.terminal.create',
	openSearch: 'workbench.search.open',
	openSettings: 'workbench.settings.open',
	setTabStripOrientation: 'workbench.tab-strip.orientation.set',
	toggleSidebar: 'workbench.sidebar.toggle',
} as const;

export interface WorkbenchTabStripOrientationInput {
	readonly orientation: TabStripOrientation;
}

export interface WorkbenchTerminalCreateInput {
	readonly groupId: string | null;
	readonly index?: number | undefined;
}

export interface WorkbenchSearchOpenInput {
	readonly filter: WorkbenchSearchFilter;
}

export interface WorkbenchActionCatalog {
	readonly [WORKBENCH_ACTION_IDS.createTerminal]: ActionSignature<WorkbenchTerminalCreateInput>;
	readonly [WORKBENCH_ACTION_IDS.openSearch]: ActionSignature<WorkbenchSearchOpenInput>;
	readonly [WORKBENCH_ACTION_IDS.openSettings]: ActionSignature<void>;
	readonly [WORKBENCH_ACTION_IDS.setTabStripOrientation]: ActionSignature<WorkbenchTabStripOrientationInput>;
	readonly [WORKBENCH_ACTION_IDS.toggleSidebar]: ActionSignature<void>;
}

function isRecord(input: unknown): input is Readonly<Record<string, unknown>> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isWorkbenchSearchFilter(input: unknown): input is WorkbenchSearchFilter {
	return input === 'all' || input === 'agents' || input === 'actions' || input === 'settings';
}

export const workbenchTerminalCreateInput: ActionInputValidation<WorkbenchTerminalCreateInput> = {
	name: 'WorkbenchTerminalCreateInput',
	parse: (input) => {
		if (input === undefined) {
			return { success: true, data: { groupId: null } };
		}
		if (
			!isRecord(input) ||
			!(input['groupId'] === null || typeof input['groupId'] === 'string') ||
			!(
				input['index'] === undefined ||
				(typeof input['index'] === 'number' &&
					Number.isInteger(input['index']) &&
					input['index'] >= 0)
			)
		) {
			return { success: false, code: 'expected-terminal-create-input' };
		}
		return {
			success: true,
			data: {
				groupId: input['groupId'],
				...(input['index'] === undefined ? {} : { index: input['index'] }),
			},
		};
	},
};

export const workbenchSearchOpenInput: ActionInputValidation<WorkbenchSearchOpenInput> = {
	name: 'WorkbenchSearchOpenInput',
	parse: (input) =>
		isRecord(input) && isWorkbenchSearchFilter(input['filter'])
			? {
					success: true,
					data: { filter: input['filter'] },
				}
			: { success: false, code: 'expected-workbench-search-filter' },
};

export const workbenchTabStripOrientationInput: ActionInputValidation<WorkbenchTabStripOrientationInput> =
	{
		name: 'WorkbenchTabStripOrientationInput',
		parse: (input) =>
			typeof input === 'object' &&
			input !== null &&
			'orientation' in input &&
			(input.orientation === 'horizontal' || input.orientation === 'vertical')
				? {
						success: true,
						data: { orientation: input.orientation },
					}
				: { success: false, code: 'expected-tab-strip-orientation' },
	};
