// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput, type ActionScope } from '@orchbee/actions';
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
import type {
	WorkbenchSearchProvider,
	WorkbenchSearchProviderSection,
} from '../../../types/workbench-search-provider';
import { WorkbenchSearchSurface } from '../WorkbenchSearchSurface';

interface SearchHarnessProps {
	readonly filter?: 'actions' | 'all';
	readonly providers?: readonly WorkbenchSearchProvider[] | undefined;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

function SearchHarness({
	filter = 'actions',
	providers,
	runtime,
	scope,
}: SearchHarnessProps): React.ReactNode {
	const [state, update] = React.useReducer(transitionWorkbenchSearch, undefined, () =>
		transitionWorkbenchSearch(createWorkbenchSearchState(), {
			filter,
			type: 'open',
		}),
	);
	return state.kind === 'closed' ? null : (
		<WorkbenchSearchSurface
			actionRuntime={runtime}
			actionScope={scope}
			providers={providers}
			shortcutLabelForAction={() => undefined}
			state={state}
			update={update}
		/>
	);
}

function createProvider(
	sections: () =>
		| readonly WorkbenchSearchProviderSection[]
		| Promise<readonly WorkbenchSearchProviderSection[]>,
): WorkbenchSearchProvider {
	return {
		filters: ['all'],
		getDefaultSections: sections,
		id: 'remote',
		label: 'Remote commands',
		priority: 20,
		resultLimit: 5,
		search: sections,
	};
}

function noop(): void {}

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

		fireEvent.click(screen.getByRole('button', { name: 'Back' }));
		await waitFor(() => expect(input.getAttribute('placeholder')).toBe('Search actions…'));
		expect((input as HTMLInputElement).value).toBe('orientation');
		expect(document.activeElement).toBe(input);

		fireEvent.keyDown(input, { key: 'Enter' });
		fireEvent.click(screen.getByRole('option', { name: /Vertical tabs/u }));
		await waitFor(() => expect(execute).toHaveBeenCalledWith('vertical'));
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('renders asynchronous provider progress and invokes the provider-owned Action token', async () => {
		const execute = vi.fn<() => { readonly kind: 'completed' }>(() => ({ kind: 'completed' }));
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		const registration = runtime.register(
			scope,
			defineAction({
				category: { id: 'navigation', title: workbenchMessages.actionCategoryNavigation },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'remote',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute },
				id: 'remote.open',
				input: noActionInput,
				title: { defaultMessage: 'Open remote item', id: 'test.remote.open' },
			}),
		);
		const discovered = runtime
			.discover({ scope, source: 'command-surface' })
			.find((result) => result.descriptor.id === 'remote.open');
		if (discovered?.kind !== 'ready') {
			throw new Error('Expected a ready remote Action');
		}
		let resolveSections: (sections: readonly WorkbenchSearchProviderSection[]) => void = noop;
		const sections = new Promise<readonly WorkbenchSearchProviderSection[]>((resolve) => {
			resolveSections = resolve;
		});
		const providers = [createProvider(() => sections)];

		render(
			<OrchbeeI18nProvider locale='en-US'>
				<SearchHarness filter='all' providers={providers} runtime={runtime} scope={scope} />
			</OrchbeeI18nProvider>,
		);

		const input = await screen.findByRole('combobox', { name: 'Search Orchbee' });
		expect(document.activeElement).toBe(input);
		expect((await screen.findByRole('status')).textContent).toContain('Searching Remote commands…');
		resolveSections([
			{
				id: 'remote',
				label: 'Remote',
				order: 80,
				results: [
					{
						availability: { kind: 'enabled' },
						canonicalLabel: 'Open remote item',
						icon: 'action',
						id: 'open',
						invocation: { kind: 'ready', prepared: discovered.invocation },
						keywords: ['remote'],
						label: 'Open remote item',
					},
				],
			},
		]);

		const option = await screen.findByRole('option', { name: 'Open remote item' });
		expect(screen.queryByRole('status')).toBeNull();
		expect(document.activeElement).toBe(input);
		fireEvent.click(option);
		await waitFor(() => expect(execute).toHaveBeenCalledOnce());
		expect(screen.queryByRole('dialog')).toBeNull();
		registration.dispose();
	});

	it('keeps a failed provider local and retries only after explicit user input', async () => {
		let attempt = 0;
		const provider = createProvider(() => {
			attempt += 1;
			return attempt === 1
				? Promise.reject(new Error('offline'))
				: [
						{
							id: 'remote',
							label: 'Remote',
							order: 80,
							results: [],
						},
					];
		});
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');

		render(
			<OrchbeeI18nProvider locale='en-US'>
				<SearchHarness filter='all' providers={[provider]} runtime={runtime} scope={scope} />
			</OrchbeeI18nProvider>,
		);

		expect((await screen.findByRole('status')).textContent).toContain(
			'Remote commands could not be searched.',
		);
		expect(attempt).toBe(1);
		fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
		await waitFor(() => expect(attempt).toBe(2));
		await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
	});
});
