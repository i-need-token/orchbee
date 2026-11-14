import type { ProjectedTerminalSearchEvent } from '@orchbee/terminal';
import { AttachmentIdSchema, CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { TerminalFindController, type TerminalSearchDriver } from '../TerminalFindController';

describe('TerminalFindController', () => {
	it('keeps search client-local and exposes immutable result status', () => {
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController(
			{ clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(), updateSearch },
			12n,
		);
		controller.open();
		controller.setQuery('orchbee');
		controller.setCaseSensitive(true);
		controller.applySearchEvent(searchEvent('orchbee', 3, 1));

		expect(updateSearch).toHaveBeenLastCalledWith('orchbee', {
			caseSensitive: true,
			regex: false,
			wholeWord: false,
		});
		expect(controller.getSnapshot()).toMatchObject({
			activeIndex: 1,
			isOpen: true,
			resultCount: 3,
			retainedRows: 12n,
			searchState: 'complete',
		});
	});

	it('restarts a retained query when the find surface reopens', () => {
		const clearSearch = vi.fn<TerminalSearchDriver['clearSearch']>();
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({ clearSearch, updateSearch });
		controller.open();
		controller.setQuery('retained');
		controller.close();
		updateSearch.mockClear();

		controller.open();

		expect(clearSearch).toHaveBeenCalledOnce();
		expect(updateSearch).toHaveBeenCalledOnce();
		expect(updateSearch).toHaveBeenCalledWith('retained', {
			caseSensitive: false,
			regex: false,
			wholeWord: false,
		});
	});

	it('seeds a newly opened find surface from the bounded Host selection preview', () => {
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch,
		});

		controller.open('selected text');

		expect(controller.getSnapshot()).toMatchObject({
			isOpen: true,
			query: 'selected text',
			searchState: 'searching',
		});
		expect(updateSearch).toHaveBeenCalledWith('selected text', {
			caseSensitive: false,
			regex: false,
			wholeWord: false,
		});
	});

	it.each([
		{ currentQuery: '', isOpen: false },
		{ currentQuery: 'retained query', isOpen: false },
		{ currentQuery: '', isOpen: true },
		{ currentQuery: 'retained query', isOpen: true },
	])(
		'replaces query "$currentQuery" from the current Host selection when isOpen=$isOpen',
		({ currentQuery, isOpen }) => {
			const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
			const controller = new TerminalFindController({
				clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
				updateSearch,
			});
			controller.open();
			if (currentQuery) controller.setQuery(currentQuery);
			if (!isOpen) controller.close();
			updateSearch.mockClear();
			const focusRequest = controller.getSnapshot().focusRequest;

			controller.open('current selection');

			expect(controller.getSnapshot()).toMatchObject({
				focusRequest: focusRequest + 1,
				isOpen: true,
				query: 'current selection',
				searchState: 'searching',
			});
			expect(updateSearch).toHaveBeenCalledOnce();
			expect(updateSearch).toHaveBeenCalledWith('current selection', {
				caseSensitive: false,
				regex: false,
				wholeWord: false,
			});
		},
	);

	it('reports history and selection eviction without retaining content', () => {
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
		});
		controller.applyHistoryEvent({
			anchorStatus: 'evicted',
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			generation: CanonicalUint64TextSchema.parse('2'),
			revision: CanonicalUint64TextSchema.parse('4'),
			sequence: CanonicalUint64TextSchema.parse('1'),
			totalRows: CanonicalUint64TextSchema.parse('100'),
		});
		expect(controller.getSnapshot()).toMatchObject({
			notice: { kind: 'history-evicted' },
			retainedRows: 100n,
		});

		controller.applySelectionEvent({
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			state: 'evicted',
		});
		expect(controller.getSnapshot().notice).toEqual({ kind: 'selection-evicted' });
	});

	it('preserves unavailable search failures as a distinct product state', () => {
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
		});
		controller.setQuery('query');
		controller.applySearchEvent({
			...searchEvent('query', 0, -1),
			errorKind: 'unavailable',
			regexError: 'Terminal search source became unavailable.',
			state: 'error',
		});

		expect(controller.getSnapshot()).toMatchObject({
			searchErrorKind: 'unavailable',
			searchState: 'error',
		});
	});

	describe.each(['close', 'dispose'] as const)('subscriber %s', (retire) => {
		it.each([
			{ name: 'query', apply: (owner: TerminalFindController) => owner.setQuery('next') },
			{ name: 'case', apply: (owner: TerminalFindController) => owner.setCaseSensitive(true) },
			{ name: 'word', apply: (owner: TerminalFindController) => owner.setWholeWord(true) },
			{ name: 'regex', apply: (owner: TerminalFindController) => owner.setRegex(true) },
			{ name: 'opening', apply: (owner: TerminalFindController) => owner.open('selection') },
		])('prevents a retired $name transition from starting search', ({ apply }) => {
			const clearSearch = vi.fn<TerminalSearchDriver['clearSearch']>();
			const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
			const controller = new TerminalFindController({ clearSearch, updateSearch });
			controller.open('initial');
			updateSearch.mockClear();
			let retired = false;
			controller.subscribe(() => {
				if (retired) return;
				retired = true;
				controller[retire]();
			});

			apply(controller);

			expect(clearSearch).toHaveBeenCalledOnce();
			expect(updateSearch).not.toHaveBeenCalled();
			controller.dispose();
		});
	});

	it('preserves a newer query chosen by an opening subscriber', () => {
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch,
		});
		controller.open('retained');
		updateSearch.mockClear();
		let replaced = false;
		controller.subscribe(() => {
			if (replaced) return;
			replaced = true;
			controller.setQuery('newer query');
		});

		controller.open('selection');

		expect(controller.getSnapshot().query).toBe('newer query');
		expect(updateSearch.mock.calls).toEqual([
			['newer query', { caseSensitive: false, regex: false, wholeWord: false }],
		]);
		controller.dispose();
	});

	it('does not restart a replacement query after its subscriber returns', () => {
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch,
		});
		controller.open();
		let replaced = false;
		controller.subscribe(() => {
			if (replaced) return;
			replaced = true;
			controller.setQuery('latest');
		});

		controller.setQuery('superseded');

		expect(updateSearch.mock.calls).toEqual([
			['latest', { caseSensitive: false, regex: false, wholeWord: false }],
		]);
		controller.dispose();
	});

	it('keeps search admission when a subscriber publishes an independent notice', () => {
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch,
		});
		controller.open();
		let notified = false;
		controller.subscribe(() => {
			if (notified) return;
			notified = true;
			controller.setNotice({ kind: 'history-unavailable' });
		});

		controller.setQuery('selected query');

		expect(controller.getSnapshot()).toMatchObject({
			notice: { kind: 'history-unavailable' },
			query: 'selected query',
		});
		expect(updateSearch.mock.calls).toEqual([
			['selected query', { caseSensitive: false, regex: false, wholeWord: false }],
		]);
		controller.dispose();
	});

	it('keeps pending query work when a subscriber only refocuses the open surface', () => {
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch,
		});
		controller.open();
		let refocused = false;
		controller.subscribe(() => {
			if (refocused) return;
			refocused = true;
			controller.open();
		});

		controller.setQuery('current query');

		expect(controller.getSnapshot().focusRequest).toBe(2);
		expect(updateSearch.mock.calls).toEqual([
			['current query', { caseSensitive: false, regex: false, wholeWord: false }],
		]);
		controller.dispose();
	});

	it('does not close a search reopened by the driver clear callback', () => {
		const clearSearch = vi.fn<TerminalSearchDriver['clearSearch']>();
		const updateSearch = vi.fn<TerminalSearchDriver['updateSearch']>();
		const controller = new TerminalFindController({ clearSearch, updateSearch });
		controller.open('retained');
		updateSearch.mockClear();
		clearSearch.mockImplementationOnce(() => controller.open('latest'));

		controller.close();

		expect(controller.getSnapshot()).toMatchObject({ isOpen: true, query: 'latest' });
		expect(updateSearch.mock.calls).toEqual([
			['latest', { caseSensitive: false, regex: false, wholeWord: false }],
		]);
		controller.dispose();
	});

	it('leaves the latest search active when a closed-state subscriber reopens it', () => {
		const effects: string[] = [];
		const controller = new TerminalFindController({
			clearSearch: () => effects.push('clear'),
			updateSearch: (query) => effects.push(`search:${query}`),
		});
		controller.open('retained');
		effects.length = 0;
		controller.subscribe(() => {
			if (!controller.getSnapshot().isOpen) controller.open('latest');
		});

		controller.close();

		expect(controller.getSnapshot()).toMatchObject({ isOpen: true, query: 'latest' });
		expect(effects.at(-1)).toBe('search:latest');
		controller.dispose();
	});
});

function searchEvent(
	query: string,
	matchCount: number,
	activeIndex: number,
): ProjectedTerminalSearchEvent {
	return {
		activeMatch:
			activeIndex < 0
				? null
				: {
						end: { column: activeIndex + 1, row: activeIndex },
						start: { column: activeIndex, row: activeIndex },
						text: query,
					},
		activeIndex,
		errorKind: null,
		generation: 1n,
		matches: Array.from({ length: matchCount }, (_, index) => ({
			end: { column: index + 1, row: index },
			start: { column: index, row: index },
			text: query,
		})),
		query,
		regexError: null,
		resultLimitReached: false,
		revision: 1n,
		state: 'complete',
	};
}
