import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { codeEditorTextEdit } from '../codeEditorTextEdit';

/** A model holds at least one line, as Monaco's does for empty text. */
const linesArbitrary = fc.array(fc.constantFrom('', 'a', 'b', 'ab'), {
	maxLength: 8,
	minLength: 1,
});

function modelOf(lines: readonly string[]) {
	return {
		getLineContent: (lineNumber: number): string => {
			const line = lines[lineNumber - 1];
			if (line === undefined) throw new Error(`Line ${lineNumber} is outside the model.`);
			return line;
		},
		getLineCount: (): number => lines.length,
	};
}

/** The offset of a position in the lines joined by `\n`. */
function offsetOf(lines: readonly string[], lineNumber: number, column: number): number {
	let offset = 0;
	for (let index = 0; index < lineNumber - 1; index += 1) offset += (lines[index]?.length ?? 0) + 1;
	return offset + column - 1;
}

/** The lines as one text, with the edit applied when there is one. */
function edited(lines: readonly string[], edit: ReturnType<typeof codeEditorTextEdit>): string {
	const text = lines.join('\n');
	if (edit === null) return text;
	const { endColumn, endLineNumber, startColumn, startLineNumber } = edit.range;
	const start = offsetOf(lines, startLineNumber, startColumn);
	const end = offsetOf(lines, endLineNumber, endColumn);
	if (end < start) throw new Error('The edit ends before it starts.');
	return text.slice(0, start) + edit.text + text.slice(end);
}

describe('codeEditorTextEdit', () => {
	it('turns the old lines into the new ones, and is null exactly when none differs', () => {
		fc.assert(
			fc.property(linesArbitrary, linesArbitrary, (before, after) => {
				const edit = codeEditorTextEdit(modelOf(before), after);
				expect(edited(before, edit)).toBe(after.join('\n'));
				expect(edit === null).toBe(JSON.stringify(before) === JSON.stringify(after));
			}),
		);
	});
});
