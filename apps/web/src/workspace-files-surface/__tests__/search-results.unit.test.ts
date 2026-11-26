import type { WorkspaceSearchFile } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	appendSearchFiles,
	matchRange,
	previewParts,
	SEARCHING,
	splitGlobs,
	stepSearchMatch,
} from '../search-results';

// The Server reports previews in UTF-16 units from `previewStart`, and the editor takes columns
// from 1; an off-by-one here selects the wrong text without any visible error.

function entry(path: string, lines: WorkspaceSearchFile['lines']): WorkspaceSearchFile {
	return { lines, path };
}

describe('matchRange', () => {
	it('counts columns from where a cut preview starts', () => {
		const line = {
			line: 7,
			matches: [{ end: 9, start: 3 }],
			preview: 'xx needle yy',
			previewStart: 40,
		};
		expect(matchRange(line, 0)).toEqual({
			endColumn: 50,
			endLineNumber: 7,
			startColumn: 44,
			startLineNumber: 7,
		});
	});

	it('ends a match that spans lines on the later line, counting from its start', () => {
		const line = {
			line: 3,
			matches: [{ end: 11, start: 4 }],
			preview: 'let foo\nbar = 1',
			previewStart: 0,
		};
		expect(matchRange(line, 0)).toEqual({
			endColumn: 4,
			endLineNumber: 4,
			startColumn: 5,
			startLineNumber: 3,
		});
	});
});

describe('appendSearchFiles', () => {
	it('continues the file before when an entry names it again, and counts every match', () => {
		const first = appendSearchFiles(
			SEARCHING,
			[
				entry('a.ts', [
					{ line: 1, matches: [{ end: 1, start: 0 }], preview: 'a', previewStart: 0 },
				]),
			],
			'searching',
		);
		const second = appendSearchFiles(
			first,
			[
				entry('a.ts', [
					{
						line: 9,
						matches: [
							{ end: 1, start: 0 },
							{ end: 3, start: 2 },
						],
						preview: 'aba',
						previewStart: 0,
					},
				]),
				entry('b.ts', [
					{ line: 2, matches: [{ end: 1, start: 0 }], preview: 'a', previewStart: 0 },
				]),
			],
			'complete',
		);
		expect(second.files.map((file) => [file.path, file.lines.length, file.matches])).toEqual([
			['a.ts', 2, 3],
			['b.ts', 1, 1],
		]);
		expect(second.matches).toBe(4);
		expect(second.state).toBe('complete');
	});
});

describe('stepSearchMatch', () => {
	const results = appendSearchFiles(
		SEARCHING,
		[
			entry('a.ts', [
				{
					line: 1,
					matches: [
						{ end: 1, start: 0 },
						{ end: 3, start: 2 },
					],
					preview: 'aba',
					previewStart: 0,
				},
			]),
			entry('b.ts', [{ line: 5, matches: [{ end: 1, start: 0 }], preview: 'a', previewStart: 0 }]),
		],
		'complete',
	);
	const key = (path: string, line: number): string => `${path}\n${line}:0`;

	it('walks every match of a line, then the next file, and wraps at the end', () => {
		const steps: string[] = [];
		let active = null;
		for (let step = 0; step < 4; step += 1) {
			const next = stepSearchMatch(results, active, 1);
			if (next === null) break;
			steps.push(`${next.path}:${next.match}`);
			active = next;
		}
		expect(steps).toEqual(['a.ts:0', 'a.ts:1', 'b.ts:0', 'a.ts:0']);
	});

	it('steps from a file into its first match, and back to the last match above it', () => {
		expect(stepSearchMatch(results, { key: null, match: 0, path: 'b.ts' }, 1)).toMatchObject({
			key: key('b.ts', 5),
			match: 0,
		});
		expect(stepSearchMatch(results, { key: null, match: 0, path: 'b.ts' }, -1)).toMatchObject({
			key: key('a.ts', 1),
			match: 1,
		});
	});
});

describe('previewParts', () => {
	it('marks the matches on the first line, trims the indentation and notes what is cut', () => {
		const parts = previewParts({
			line: 1,
			matches: [{ end: 9, start: 6 }],
			preview: '\t\tfoo(bar)\r\nnext',
			previewStart: 3,
		});
		expect(parts).toEqual([
			{ match: false, text: '…' },
			{ match: false, text: 'foo(' },
			{ match: true, text: 'bar' },
			{ match: false, text: ')' },
			{ match: false, text: ' …' },
		]);
	});
});

describe('splitGlobs', () => {
	it('splits at commas outside braces and drops blank entries', () => {
		expect(splitGlobs(' *.{ts,tsx}, ,src/**/x ,')).toEqual(['*.{ts,tsx}', 'src/**/x']);
	});
});
