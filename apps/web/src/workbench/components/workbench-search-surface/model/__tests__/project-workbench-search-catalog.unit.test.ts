import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { createOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_ACTION_IDS,
	workbenchTabStripOrientationInput,
} from '../../../../actions/workbench-action-ids';
import { projectWorkbenchSearchCatalog } from '../project-workbench-search-catalog';

describe('Workbench search catalog projection', () => {
	it('projects ready, disabled, settings, and nested-input results from runtime discovery', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		const terminalScope = runtime.createScope('terminal', scope);
		const execution = {
			concurrency: 'parallel' as const,
			concurrencyKey: () => 'test',
			mode: 'foreground' as const,
		};
		runtime.register(
			scope,
			defineAction({
				category: { id: 'navigation', title: workbenchMessages.actionCategoryNavigation },
				execution,
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: () => ({ kind: 'completed' }) },
				id: WORKBENCH_ACTION_IDS.openSettings,
				input: noActionInput,
				title: workbenchMessages.settings,
			}),
		);
		runtime.register(
			terminalScope,
			defineAction({
				category: { id: 'terminal', title: workbenchMessages.actionCategoryTerminal },
				execution,
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: () => ({ kind: 'completed' }) },
				id: 'terminal.clipboard.paste',
				input: noActionInput,
				title: { defaultMessage: 'Paste', id: 'test.terminal.paste' },
			}),
		);
		runtime.register(
			scope,
			defineAction({
				category: { id: 'view', title: workbenchMessages.actionCategoryView },
				execution,
				getDiscoveryAvailability: () => ({ kind: 'enabled' }),
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute: () => ({ kind: 'completed' }) },
				id: WORKBENCH_ACTION_IDS.setTabStripOrientation,
				input: workbenchTabStripOrientationInput,
				title: workbenchMessages.tabStripOrientation,
			}),
		);

		const catalog = projectWorkbenchSearchCatalog({
			argumentPickerForAction: () => undefined,
			intl: createOrchbeeIntl('en-US'),
			runtime,
			scope,
			shortcutLabelForAction: (actionId) => {
				if (actionId === WORKBENCH_ACTION_IDS.openSettings) return '⌘,';
				return actionId === 'terminal.clipboard.paste' ? '⌘V' : undefined;
			},
		});

		expect(catalog.settingsResults).toMatchObject([
			{
				actionId: WORKBENCH_ACTION_IDS.openSettings,
				groupLabel: 'Settings',
				icon: 'settings',
				label: 'Settings',
				providerId: 'settings',
				shortcut: '⌘,',
			},
		]);
		expect(
			catalog.actionResults.find(
				(result) => result.actionId === WORKBENCH_ACTION_IDS.setTabStripOrientation,
			),
		).toMatchObject({
			groupLabel: 'View',
			invocation: {
				kind: 'requires-input',
				picker: {
					kind: 'options',
					options: [{ id: 'horizontal' }, { id: 'vertical' }],
				},
			},
		});
		expect(
			catalog.actionResults.find((result) => result.actionId === 'terminal.clipboard.paste'),
		).toMatchObject({ shortcut: '⌘V' });
	});
});
