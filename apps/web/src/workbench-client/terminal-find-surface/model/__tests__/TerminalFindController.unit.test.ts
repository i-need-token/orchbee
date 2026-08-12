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

	it('bounds in-memory query history to the latest twenty entries', () => {
		const controller = new TerminalFindController({
			clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
			updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
		});
		for (let index = 1; index <= 21; index++) {
			controller.setQuery(`query-${index}`);
			controller.recordCurrentQuery();
		}
		controller.setQuery('draft');

		for (let index = 0; index < 20; index++) {
			expect(controller.moveInQueryHistory(-1)).toBe(true);
		}
		expect(controller.getSnapshot().query).toBe('query-2');
		expect(controller.moveInQueryHistory(-1)).toBe(false);
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
});

function searchEvent(
	query: string,
	matchCount: number,
	activeIndex: number,
): ProjectedTerminalSearchEvent {
	return {
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
