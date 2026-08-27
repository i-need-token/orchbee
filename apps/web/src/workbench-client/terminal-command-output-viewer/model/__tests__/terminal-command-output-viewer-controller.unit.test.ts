import { describe, expect, it, vi } from 'vitest';

import {
	MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH,
	projectTerminalCommandOutputSegments,
	TerminalCommandOutputViewerController,
} from '../terminal-command-output-viewer-controller';

const maximumMatches = 1_000;

describe('TerminalCommandOutputViewerController', () => {
	it('opens one immutable output document and searches it case-insensitively', () => {
		const controller = new TerminalCommandOutputViewerController();
		const document = output('Alpha beta ALPHA');
		controller.open(document);
		controller.setQuery('alpha');

		expect(controller.getSnapshot()).toMatchObject({
			activeMatchIndex: 0,
			document,
			focusRequest: 1,
			isOpen: true,
			matchLimitReached: false,
			matches: [
				{ end: 5, start: 0 },
				{ end: 16, start: 11 },
			],
			query: 'alpha',
		});
	});

	it('cycles matches in both directions and can explicitly request query focus', () => {
		const controller = new TerminalCommandOutputViewerController();
		controller.open(output('one one'));
		controller.setQuery('one');

		expect(controller.moveMatch(1)).toBe(true);
		expect(controller.getSnapshot().activeMatchIndex).toBe(1);
		expect(controller.moveMatch(1)).toBe(true);
		expect(controller.getSnapshot().activeMatchIndex).toBe(0);
		expect(controller.moveMatch(-1)).toBe(true);
		expect(controller.getSnapshot().activeMatchIndex).toBe(1);
		controller.focusQuery();
		expect(controller.getSnapshot().focusRequest).toBe(2);
	});

	it('bounds the query and rendered matches without hiding that the cap was reached', () => {
		const controller = new TerminalCommandOutputViewerController();
		controller.open(output('x '.repeat(maximumMatches + 1)));
		controller.setQuery(`x${'y'.repeat(MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH)}`);
		expect(controller.getSnapshot().query).toHaveLength(MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH);

		controller.setQuery('x');
		expect(controller.getSnapshot()).toMatchObject({
			matchLimitReached: true,
			matches: expect.any(Array),
		});
		expect(controller.getSnapshot().matches).toHaveLength(maximumMatches);
	});

	it('keeps empty and unavailable results distinct and non-searchable', () => {
		const controller = new TerminalCommandOutputViewerController();
		controller.open({ kind: 'empty', reason: 'no-shell-integration' });
		controller.setQuery('ignored');
		expect(controller.getSnapshot()).toMatchObject({
			activeMatchIndex: -1,
			document: { kind: 'empty', reason: 'no-shell-integration' },
			matches: [],
			query: '',
		});

		controller.open({ kind: 'unavailable', reason: 'source-limit' });
		expect(controller.getSnapshot().document).toEqual({
			kind: 'unavailable',
			reason: 'source-limit',
		});
	});

	it('projects complete text segments and identifies only the active match', () => {
		expect(
			projectTerminalCommandOutputSegments(
				'a-one-b-one-c',
				[
					{ end: 5, start: 2 },
					{ end: 11, start: 8 },
				],
				1,
			),
		).toEqual([
			{ kind: 'text', text: 'a-' },
			{ active: false, kind: 'match', matchIndex: 0, text: 'one' },
			{ kind: 'text', text: '-b-' },
			{ active: true, kind: 'match', matchIndex: 1, text: 'one' },
			{ kind: 'text', text: '-c' },
		]);
	});

	it('notifies subscribers, closes idempotently, and rejects work after disposal', () => {
		const controller = new TerminalCommandOutputViewerController();
		const listener = vi.fn<() => void>();
		const unsubscribe = controller.subscribe(listener);
		controller.open(output('value'));
		controller.close();
		controller.close();
		expect(listener).toHaveBeenCalledTimes(2);
		expect(controller.getSnapshot()).toMatchObject({ document: null, isOpen: false });

		unsubscribe();
		controller.dispose();
		controller.open(output('ignored'));
		expect(controller.getSnapshot().isOpen).toBe(false);
	});
});

function output(text: string) {
	return {
		endRow: 4n,
		generation: 1n,
		kind: 'output' as const,
		revision: 2n,
		startRow: 3n,
		text,
		truncatedStart: false,
	};
}
