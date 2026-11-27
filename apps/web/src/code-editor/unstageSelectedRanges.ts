/**
 * A changed block as Monaco's diff editor reports it from `getLineChanges()`: 1-based lines, and an
 * end of 0 for the side the block is empty on, whose start is then the line before it.
 */
interface CodeEditorLineChange {
	readonly modifiedEndLineNumber: number;
	readonly modifiedStartLineNumber: number;
	readonly originalEndLineNumber: number;
	readonly originalStartLineNumber: number;
}

/** The lines a selection spans, 1-based as Monaco numbers them. */
interface LineSelection {
	readonly endLineNumber: number;
	readonly startLineNumber: number;
}

/** A position as VS Code's text documents count it: a 0-based line and character. */
interface Position {
	readonly character: number;
	readonly line: number;
}

interface Range {
	readonly end: Position;
	readonly start: Position;
}

function compare(left: Position, right: Position): number {
	return left.line - right.line || left.character - right.character;
}

/** A range between two positions in either order, as VS Code's `Range` orders them. */
function rangeOf(start: Position, end: Position): Range {
	return compare(start, end) <= 0 ? { end, start } : { end: start, start: end };
}

/** VS Code's `Range.intersection`: an empty intersection still counts. */
function intersection(left: Range, right: Range): Range | undefined {
	const start = compare(left.start, right.start) >= 0 ? left.start : right.start;
	const end = compare(left.end, right.end) <= 0 ? left.end : right.end;
	return compare(start, end) > 0 ? undefined : { end, start };
}

const LINE_BREAK = /\r\n|\r|\n/gu;

/** Text read as VS Code's text documents read it: lines, their lengths, and validated ranges. */
class TextLines {
	public readonly lineCount: number;
	/** Where each line's content ends, before its line break. */
	private readonly ends: number[] = [];
	private readonly source: string;
	private readonly starts: number[] = [0];

	public constructor(source: string) {
		this.source = source;
		for (const match of source.matchAll(LINE_BREAK)) {
			this.ends.push(match.index);
			this.starts.push(match.index + match[0].length);
		}
		this.ends.push(source.length);
		this.lineCount = this.starts.length;
	}

	public lineLength(line: number): number {
		return (this.ends[line] ?? 0) - (this.starts[line] ?? 0);
	}

	public text(range: Range): string {
		return this.source.slice(this.offset(range.start), this.offset(range.end));
	}

	/** A line past the last is the end of the text and a character past the line is its end. */
	private offset(position: Position): number {
		if (position.line >= this.lineCount) return this.source.length;
		const start = this.starts[Math.max(position.line, 0)] ?? 0;
		const end = this.ends[Math.max(position.line, 0)] ?? 0;
		return Math.min(start + Math.max(position.character, 0), end);
	}
}

/** The whole lines each selection touches, sorted, with touching selections joined. */
function toLineRanges(selections: readonly LineSelection[], document: TextLines): Range[] {
	const lineRanges = selections
		.map((selection) =>
			rangeOf(
				{ character: 0, line: selection.startLineNumber - 1 },
				{
					character: document.lineLength(selection.endLineNumber - 1),
					line: selection.endLineNumber - 1,
				},
			),
		)
		.sort((left, right) => left.start.line - right.start.line);
	const result: Range[] = [];
	for (const lineRange of lineRanges) {
		const last = result.at(-1);
		if (last === undefined) {
			result.push(lineRange);
			continue;
		}
		// VS Code keeps only the overlap of overlapping selections.
		const overlap = intersection(lineRange, last);
		if (overlap !== undefined) result[result.length - 1] = overlap;
		else if (lineRange.start.line === last.end.line + 1) {
			result[result.length - 1] = rangeOf(last.start, lineRange.end);
		} else result.push(lineRange);
	}
	return result;
}

/** The block's lines in the modified text; a deletion is the line break where its lines were. */
function modifiedRange(document: TextLines, change: CodeEditorLineChange): Range {
	const start = change.modifiedStartLineNumber;
	if (change.modifiedEndLineNumber !== 0) {
		return rangeOf(
			{ character: 0, line: start - 1 },
			{
				character: document.lineLength(change.modifiedEndLineNumber - 1),
				line: change.modifiedEndLineNumber - 1,
			},
		);
	}
	if (start === 0)
		return rangeOf({ character: 0, line: 0 }, { character: document.lineLength(0), line: 0 });
	const lineEnd = { character: document.lineLength(start - 1), line: start - 1 };
	return document.lineCount === start
		? rangeOf(lineEnd, lineEnd)
		: rangeOf(lineEnd, { character: 0, line: start });
}

/**
 * The part of a block a selection touches. A block with as many lines on both sides is taken line
 * by line; any other keeps its whole original side.
 */
function intersectChange(
	document: TextLines,
	change: CodeEditorLineChange,
	lineRange: Range,
): CodeEditorLineChange | null {
	const overlap = intersection(lineRange, modifiedRange(document, change));
	if (overlap === undefined) return null;
	if (change.modifiedEndLineNumber === 0) return change;
	const modifiedStartLineNumber = overlap.start.line + 1;
	const modifiedEndLineNumber = overlap.end.line + 1;
	if (
		change.originalEndLineNumber - change.originalStartLineNumber ===
		change.modifiedEndLineNumber - change.modifiedStartLineNumber
	) {
		const delta = modifiedStartLineNumber - change.modifiedStartLineNumber;
		const length = modifiedEndLineNumber - modifiedStartLineNumber;
		return {
			modifiedEndLineNumber,
			modifiedStartLineNumber,
			originalEndLineNumber: change.originalStartLineNumber + delta + length,
			originalStartLineNumber: change.originalStartLineNumber + delta,
		};
	}
	return {
		modifiedEndLineNumber,
		modifiedStartLineNumber,
		originalEndLineNumber: change.originalEndLineNumber,
		originalStartLineNumber: change.originalStartLineNumber,
	};
}

function invert(change: CodeEditorLineChange): CodeEditorLineChange {
	return {
		modifiedEndLineNumber: change.originalEndLineNumber,
		modifiedStartLineNumber: change.originalStartLineNumber,
		originalEndLineNumber: change.modifiedEndLineNumber,
		originalStartLineNumber: change.modifiedStartLineNumber,
	};
}

/** The original text with the blocks replaced by their modified lines, as VS Code's Git applies them. */
function applyLineChanges(
	original: TextLines,
	modified: TextLines,
	changes: readonly CodeEditorLineChange[],
): string {
	const result: string[] = [];
	let currentLine = 0;
	for (const change of changes) {
		const insertion = change.originalEndLineNumber === 0;
		const deletion = change.modifiedEndLineNumber === 0;
		let endLine = insertion ? change.originalStartLineNumber : change.originalStartLineNumber - 1;
		let endCharacter = 0;
		// A deletion at the very end also takes the line break before it.
		if (deletion && change.originalEndLineNumber === original.lineCount) {
			endLine -= 1;
			endCharacter = original.lineLength(endLine);
		}
		result.push(
			original.text(
				rangeOf({ character: 0, line: currentLine }, { character: endCharacter, line: endLine }),
			),
		);
		if (!deletion) {
			let fromLine = change.modifiedStartLineNumber - 1;
			let fromCharacter = 0;
			// An insertion at the very end starts after the last line, to take its line break.
			if (insertion && change.originalStartLineNumber === original.lineCount) {
				fromLine -= 1;
				fromCharacter = modified.lineLength(fromLine);
			}
			result.push(
				modified.text(
					rangeOf(
						{ character: fromCharacter, line: fromLine },
						{ character: 0, line: change.modifiedEndLineNumber },
					),
				),
			);
		}
		currentLine = insertion ? change.originalStartLineNumber : change.originalEndLineNumber;
	}
	result.push(
		original.text(
			rangeOf({ character: 0, line: currentLine }, { character: 0, line: original.lineCount }),
		),
	);
	return result.join('');
}

/**
 * VS Code Git's Unstage Selected Ranges on a staged diff: the text the index takes when the staged
 * changes the selections touch return to `HEAD`, or `null` when they touch none. `changes` are the
 * diff editor's, `HEAD` against the index.
 */
export function unstageSelectedRanges(request: {
	readonly changes: readonly CodeEditorLineChange[];
	readonly head: string;
	readonly index: string;
	readonly selections: readonly LineSelection[];
}): string | null {
	const index = new TextLines(request.index);
	const selectedLines = toLineRanges(request.selections, index);
	const selected = request.changes
		.map((change) =>
			selectedLines.reduce<CodeEditorLineChange | null>(
				(found, lineRange) => found ?? intersectChange(index, change, lineRange),
				null,
			),
		)
		.filter((change) => change !== null);
	if (selected.length === 0) return null;
	return applyLineChanges(index, new TextLines(request.head), selected.map(invert));
}
