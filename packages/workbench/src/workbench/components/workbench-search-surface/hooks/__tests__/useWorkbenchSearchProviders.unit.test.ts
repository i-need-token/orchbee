// @vitest-environment jsdom

import { cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchSearchProvider } from '../../../../types/workbench-search-provider';
import type { WorkbenchSearchState } from '../../model/workbench-search-state';
import { useWorkbenchSearchProviders } from '../useWorkbenchSearchProviders';

const browsingState: Exclude<WorkbenchSearchState, { readonly kind: 'closed' }> = {
	filter: 'all',
	highlightedResultId: null,
	kind: 'browsing',
	nextSessionId: 2,
	query: '',
	queryRevision: 0,
	scrollTop: 0,
	sessionId: 1,
};

afterEach(() => cleanup());

describe('useWorkbenchSearchProviders', () => {
	it('connects the provider lifecycle and requests the current search snapshot', async () => {
		const getDefaultSections = vi.fn<WorkbenchSearchProvider['getDefaultSections']>(() => []);
		const search = vi.fn<WorkbenchSearchProvider['search']>(() => []);
		const provider: WorkbenchSearchProvider = {
			filters: ['all'],
			getDefaultSections,
			id: 'remote',
			label: 'Remote',
			priority: 20,
			resultLimit: 5,
			search,
		};
		const providers = [provider];
		const rendered = renderHook(
			({ state }: { readonly state: typeof browsingState }) =>
				useWorkbenchSearchProviders({ locale: 'en-US', providers, state }),
			{ initialProps: { state: browsingState } },
		);

		await waitFor(() => expect(rendered.result.current.runs[0]?.phase).toBe('ready'));
		expect(getDefaultSections).toHaveBeenCalledOnce();

		rendered.rerender({
			state: { ...browsingState, query: 'agent', queryRevision: 1 },
		});
		await waitFor(() => expect(search).toHaveBeenCalledOnce());
		expect(search).toHaveBeenCalledWith(
			'agent',
			{ filter: 'all', locale: 'en-US' },
			expect.any(AbortSignal),
		);
	});
});
