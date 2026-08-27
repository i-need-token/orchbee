import { describe, expect, it, vi } from 'vitest';

import {
	commandOutputViewerDocument,
	MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH,
	projectTerminalTextViewerSegments,
	scrollbackViewerDocument,
	TerminalTextViewerController,
} from '../terminal-text-viewer-controller';

const maximumMatches = 1_000;

describe('TerminalTextViewerController', () => {
	it('opens one immutable output document and searches it case-insensitively', () => {
		const controller = new TerminalTextViewerController();
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
		const controller = new TerminalTextViewerController();
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
		const controller = new TerminalTextViewerController();
		controller.open(output('x '.repeat(maximumMatches + 1)));
		controller.setQuery(`x${'y'.repeat(MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH)}`);
		expect(controller.getSnapshot().query).toHaveLength(MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH);

		controller.setQuery('x');
		expect(controller.getSnapshot()).toMatchObject({
			matchLimitReached: true,
			matches: expect.any(Array),
		});
		expect(controller.getSnapshot().matches).toHaveLength(maximumMatches);
	});

	it('keeps empty and unavailable results distinct and non-searchable', () => {
		const controller = new TerminalTextViewerController();
		controller.open({ kind: 'empty', reason: 'no-shell-integration', source: 'command-output' });
		controller.setQuery('ignored');
		expect(controller.getSnapshot()).toMatchObject({
			activeMatchIndex: -1,
			document: { kind: 'empty', reason: 'no-shell-integration', source: 'command-output' },
			matches: [],
			query: '',
		});

		controller.open({ kind: 'unavailable', reason: 'source-limit', source: 'scrollback' });
		expect(controller.getSnapshot().document).toEqual({
			kind: 'unavailable',
			reason: 'source-limit',
			source: 'scrollback',
		});
	});

	it('projects complete text segments and identifies only the active match', () => {
		expect(
			projectTerminalTextViewerSegments(
				'a-one-b-one-c',
				[
					{ end: 5, start: 2 },
					{ end: 11, start: 8 },
				],
				1,
				null,
			),
		).toEqual([
			{ kind: 'text', text: 'a-' },
			{ active: false, kind: 'match', matchIndex: 0, text: 'one' },
			{ kind: 'text', text: '-b-' },
			{ active: true, kind: 'match', matchIndex: 1, text: 'one' },
			{ kind: 'text', text: '-c' },
		]);
	});

	it('splits text at an exact scrollback anchor without changing match ownership', () => {
		expect(
			projectTerminalTextViewerSegments('prefix-match-tail', [{ end: 12, start: 7 }], 0, 9),
		).toEqual([
			{ kind: 'text', text: 'prefix-' },
			{ active: true, kind: 'match', matchIndex: 0, text: 'ma' },
			{ kind: 'anchor' },
			{ active: true, kind: 'match', matchIndex: 0, text: 'tch' },
			{ kind: 'text', text: '-tail' },
		]);
	});

	it('adapts command output and scrollback into tagged immutable viewer documents', () => {
		expect(commandOutputViewerDocument(commandOutput('result'))).toEqual(output('result'));
		expect(
			scrollbackViewerDocument({
				endRow: 4n,
				generation: 1n,
				initialTopOffset: 6,
				kind: 'scrollback',
				revision: 2n,
				startRow: 0n,
				text: 'older\nvisible',
			}),
		).toEqual({
			initialTopOffset: 6,
			kind: 'text',
			notice: null,
			source: 'scrollback',
			text: 'older\nvisible',
		});
	});

	it('notifies subscribers, closes idempotently, and rejects work after disposal', () => {
		const controller = new TerminalTextViewerController();
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
		initialTopOffset: null,
		kind: 'text' as const,
		notice: null,
		source: 'command-output' as const,
		text,
	};
}

function commandOutput(text: string) {
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
