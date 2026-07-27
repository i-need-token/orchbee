// @vitest-environment jsdom

import { ActionRuntime, defineAction, type ActionScope } from '@orchbee/actions';
import { OrchbeeI18nProvider, workbenchMessages } from '@orchbee/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	WORKBENCH_ACTION_IDS,
	workbenchTabStripOrientationInput,
} from '../../../actions/workbench-action-ids';
import {
	createWorkbenchSearchState,
	transitionWorkbenchSearch,
} from '../model/workbench-search-state';
import { WorkbenchSearchSurface } from '../WorkbenchSearchSurface';

interface SearchHarnessProps {
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

function SearchHarness({ runtime, scope }: SearchHarnessProps): React.ReactNode {
	const [state, update] = React.useReducer(transitionWorkbenchSearch, undefined, () =>
		transitionWorkbenchSearch(createWorkbenchSearchState(), {
			filter: 'actions',
			type: 'open',
		}),
	);
	return state.kind === 'closed' ? null : (
		<WorkbenchSearchSurface
			actionRuntime={runtime}
			actionScope={scope}
			shortcutPlatform='apple'
			state={state}
			update={update}
		/>
	);
}

afterEach(() => cleanup());

describe('WorkbenchSearchSurface', () => {
	it('keeps pointer filter changes in the combobox and roves keyboard focus between filters', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<SearchHarness runtime={runtime} scope={scope} />
			</OrchbeeI18nProvider>,
		);

		const input = await screen.findByRole('combobox', { name: 'Search Orchbee' });
		const actionsFilter = screen.getByRole('tab', { name: 'Actions' });
		const settingsFilter = screen.getByRole('tab', { name: 'Settings' });

		expect(document.activeElement).toBe(input);
		fireEvent.pointerDown(settingsFilter);
		fireEvent.click(settingsFilter);
		expect(document.activeElement).toBe(input);
		expect(settingsFilter.getAttribute('aria-selected')).toBe('true');

		settingsFilter.focus();
		fireEvent.keyDown(settingsFilter, { key: 'ArrowLeft' });
		expect(document.activeElement).toBe(actionsFilter);
		expect(actionsFilter.getAttribute('aria-selected')).toBe('true');
		expect(actionsFilter.getAttribute('tabindex')).toBe('0');
		expect(settingsFilter.getAttribute('tabindex')).toBe('-1');
	});

	it('keeps focus in the combobox while filtering and running a nested action input', async () => {
		const execute = vi.fn<(orientation: string) => void>();
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		runtime.register(
			scope,
			defineAction({
				category: { id: 'view', title: workbenchMessages.actionCategoryView },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'orientation',
					mode: 'foreground',
				},
				getDiscoveryAvailability: () => ({ kind: 'enabled' }),
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (_context, input) => {
						execute(input.orientation);
						return { kind: 'completed' };
					},
				},
				id: WORKBENCH_ACTION_IDS.setTabStripOrientation,
				input: workbenchTabStripOrientationInput,
				title: workbenchMessages.tabStripOrientation,
			}),
		);
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<SearchHarness runtime={runtime} scope={scope} />
			</OrchbeeI18nProvider>,
		);

		const input = await screen.findByRole('combobox', { name: 'Search Orchbee' });
		expect(document.activeElement).toBe(input);
		expect(input.getAttribute('placeholder')).toBe('Search actions…');
		expect(screen.getByRole('tab', { name: 'Actions' }).getAttribute('aria-selected')).toBe('true');

		fireEvent.change(input, { target: { value: 'orientation' } });
		expect(screen.getByRole('option', { name: /Set tab strip orientation/u })).toBeDefined();
		fireEvent.keyDown(input, { key: 'Enter' });
		expect(input.getAttribute('placeholder')).toBe('Search options…');
		expect(screen.getByText('Choose an option for Set tab strip orientation')).toBeDefined();

		fireEvent.click(screen.getByRole('option', { name: /Vertical tabs/u }));
		await waitFor(() => expect(execute).toHaveBeenCalledWith('vertical'));
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});
