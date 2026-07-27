// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { OrchbeeI18nProvider, workbenchMessages } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchSearchEvent,
	WorkbenchSearchState,
} from '../../model/workbench-search-state';
import { useWorkbenchSearchSurface } from '../useWorkbenchSearchSurface';

const browsingState: Exclude<WorkbenchSearchState, { readonly kind: 'closed' }> = {
	filter: 'actions',
	highlightedResultId: null,
	kind: 'browsing',
	nextSessionId: 2,
	query: '',
	queryRevision: 0,
	sessionId: 1,
};

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useWorkbenchSearchSurface', () => {
	it('prepares localized filters, action rows, and one runtime-bound invocation', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		const execute = vi.fn<() => { readonly kind: 'completed' }>(() => ({ kind: 'completed' }));
		runtime.register(
			scope,
			defineAction({
				category: { id: 'navigation', title: workbenchMessages.actionCategoryNavigation },
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'settings',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: { execute },
				id: 'workbench.settings.open',
				input: noActionInput,
				title: workbenchMessages.settings,
			}),
		);
		const update = vi.fn<(event: WorkbenchSearchEvent) => void>();
		const rendered = renderHook(
			() =>
				useWorkbenchSearchSurface({
					platform: 'apple',
					runtime,
					scope,
					state: browsingState,
					update,
				}),
			{ wrapper: I18nWrapper },
		);

		expect(rendered.result.current.dialogTitle).toBe('Search Orchbee');
		expect(rendered.result.current.filters.find((filter) => filter.id === 'actions')).toMatchObject(
			{ label: 'Actions', selected: true },
		);
		const settingsRow = rendered.result.current.sections
			.flatMap((section) => section.rows)
			.find((row) => row.label === 'Settings');
		expect(settingsRow).toBeDefined();
		act(() => settingsRow?.onInvoke());
		expect(update).toHaveBeenCalledWith({ type: 'close' });
		expect(execute).toHaveBeenCalledOnce();
	});
});
