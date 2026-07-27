import { describe, expect, it } from 'vitest';

import { createWorkbenchSearchState, transitionWorkbenchSearch } from '../workbench-search-state';

describe('Workbench search state', () => {
	it('opens a fresh preset and closes without retaining browsing state', () => {
		const opened = transitionWorkbenchSearch(createWorkbenchSearchState(), {
			filter: 'actions',
			type: 'open',
		});

		expect(opened).toMatchObject({
			filter: 'actions',
			highlightedResultId: null,
			kind: 'browsing',
			query: '',
			queryRevision: 0,
			sessionId: 1,
		});
		const closed = transitionWorkbenchSearch(opened, { type: 'close' });
		const reopened = transitionWorkbenchSearch(closed, { filter: 'all', type: 'open' });
		expect(reopened).toMatchObject({
			filter: 'all',
			kind: 'browsing',
			query: '',
			sessionId: 2,
		});
	});

	it('preserves a query across filters and retains a still-visible result', () => {
		let state = transitionWorkbenchSearch(createWorkbenchSearchState(), {
			filter: 'all',
			type: 'open',
		});
		state = transitionWorkbenchSearch(state, { query: 'term', type: 'query-changed' });
		state = transitionWorkbenchSearch(state, {
			enabledResultIds: ['terminal'],
			resultIds: ['terminal', 'settings'],
			type: 'results-changed',
		});
		state = transitionWorkbenchSearch(state, { filter: 'actions', type: 'filter-changed' });
		state = transitionWorkbenchSearch(state, {
			enabledResultIds: ['terminal'],
			resultIds: ['terminal'],
			type: 'results-changed',
		});

		expect(state).toMatchObject({
			filter: 'actions',
			highlightedResultId: 'terminal',
			query: 'term',
			queryRevision: 2,
		});
	});

	it('chooses the first enabled result and moves through inspectable disabled rows', () => {
		let state = transitionWorkbenchSearch(createWorkbenchSearchState(), {
			filter: 'actions',
			type: 'open',
		});
		state = transitionWorkbenchSearch(state, {
			enabledResultIds: ['enabled'],
			resultIds: ['disabled', 'enabled'],
			type: 'results-changed',
		});
		expect(state).toMatchObject({ highlightedResultId: 'enabled' });

		state = transitionWorkbenchSearch(state, {
			direction: 1,
			resultIds: ['disabled', 'enabled'],
			type: 'move',
		});
		expect(state).toMatchObject({ highlightedResultId: 'disabled' });
	});

	it('restores the parent filter, query, revision, and highlight after a nested picker', () => {
		let state = transitionWorkbenchSearch(createWorkbenchSearchState(), {
			filter: 'actions',
			type: 'open',
		});
		state = transitionWorkbenchSearch(state, { query: 'tabs', type: 'query-changed' });
		state = transitionWorkbenchSearch(state, {
			enabledResultIds: ['orientation'],
			resultIds: ['orientation'],
			type: 'results-changed',
		});
		state = transitionWorkbenchSearch(state, {
			actionResultId: 'orientation',
			type: 'start-argument-picker',
		});
		state = transitionWorkbenchSearch(state, {
			query: 'vertical',
			type: 'query-changed',
		});
		state = transitionWorkbenchSearch(state, { type: 'back' });

		expect(state).toMatchObject({
			filter: 'actions',
			highlightedResultId: 'orientation',
			kind: 'browsing',
			query: 'tabs',
			queryRevision: 1,
		});
	});
});
