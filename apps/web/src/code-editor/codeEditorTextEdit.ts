/** A text model's lines as an edit reads them, numbered from 1 as Monaco numbers them. */
interface CodeEditorTextLines {
	getLineContent(lineNumber: number): string;
	getLineCount(): number;
}

/** One replacement in Monaco's terms: a range of the model and the text that takes its place. */
interface CodeEditorTextEdit {
	readonly range: {
		readonly endColumn: number;
		readonly endLineNumber: number;
		readonly startColumn: number;
		readonly startLineNumber: number;
	};
	/** Lines joined by `\n`, which the model writes in its own line ending. */
	readonly text: string;
}

/**
 * The one edit that turns a model's lines into `lines`: it replaces only what lies between the run
 * of lines the two share at the start and the run they share at the end. Unchanged lines keep their
 * place, and with them the cursor, the scroll position and the folds, as VS Code's reload keeps
 * them. Null when no line differs.
 */
export function codeEditorTextEdit(
	model: CodeEditorTextLines,
	lines: readonly string[],
): CodeEditorTextEdit | null {
	const before = model.getLineCount();
	const after = lines.length;
	const shorter = Math.min(before, after);
	let leading = 0;
	while (leading < shorter && model.getLineContent(leading + 1) === lines[leading]) leading += 1;
	if (leading === before && leading === after) return null;
	// The run at the end never reaches into the run at the start, so neither counts a line twice.
	let trailing = 0;
	while (
		trailing < shorter - leading &&
		model.getLineContent(before - trailing) === lines[after - 1 - trailing]
	) {
		trailing += 1;
	}
	const changed = lines.slice(leading, after - trailing);
	if (trailing > 0) {
		// Whole lines, each with its line break, up to the first line of the run at the end.
		return {
			range: {
				endColumn: 1,
				endLineNumber: before - trailing + 1,
				startColumn: 1,
				startLineNumber: leading + 1,
			},
			text: changed.map((line) => `${line}\n`).join(''),
		};
	}
	if (leading > 0) {
		// From the end of the last shared line, so the break before each changed line goes with it.
		return {
			range: {
				endColumn: model.getLineContent(before).length + 1,
				endLineNumber: before,
				startColumn: model.getLineContent(leading).length + 1,
				startLineNumber: leading,
			},
			text: changed.map((line) => `\n${line}`).join(''),
		};
	}
	return {
		range: {
			endColumn: model.getLineContent(before).length + 1,
			endLineNumber: before,
			startColumn: 1,
			startLineNumber: 1,
		},
		text: changed.join('\n'),
	};
}
