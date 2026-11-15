// The text of a drawn terminal: rows of plain runs, colored runs, bold runs, and a cursor. The
// static page and the scene player both render rows from this model, so a row looks the same
// whether it was prerendered or written by script.

/** The terminal colors a run may take; each maps to a color of the window's terminal palette. */
type TerminalTone = 'dim' | 'yellow' | 'green' | 'cyan' | 'blue';

export type TerminalSegment =
	| string
	| { readonly text: string; readonly tone: TerminalTone | 'bold' }
	| { readonly cursor: 'blink' | 'steady' };

export interface TerminalRow {
	readonly segments: readonly TerminalSegment[];
	/** A prompt the agent was given, drawn on a band across the row. */
	readonly input?: true;
}

function segmentsOf(
	strings: TemplateStringsArray,
	parts: readonly TerminalSegment[],
): TerminalSegment[] {
	const segments: TerminalSegment[] = [];
	strings.forEach((text, index) => {
		if (text !== '') segments.push(text);
		const part = parts[index];
		if (part !== undefined) segments.push(part);
	});
	return segments;
}

/** A row written as a template: row`● ${bold('Read')} ${dim('src/app.ts')}`. */
export function row(
	strings: TemplateStringsArray,
	...parts: readonly TerminalSegment[]
): TerminalRow {
	return { segments: segmentsOf(strings, parts) };
}

/** A prompt row, drawn on a band. */
export function inputRow(
	strings: TemplateStringsArray,
	...parts: readonly TerminalSegment[]
): TerminalRow {
	return { segments: segmentsOf(strings, parts), input: true };
}

/** A row of uncolored text, as a program prints it. */
export function textRow(...segments: readonly TerminalSegment[]): TerminalRow {
	return { segments };
}

export function bold(text: string): TerminalSegment {
	return { text, tone: 'bold' };
}

export function dim(text: string): TerminalSegment {
	return { text, tone: 'dim' };
}

export function yellow(text: string): TerminalSegment {
	return { text, tone: 'yellow' };
}

export function green(text: string): TerminalSegment {
	return { text, tone: 'green' };
}

export function cyan(text: string): TerminalSegment {
	return { text, tone: 'cyan' };
}

export function blue(text: string): TerminalSegment {
	return { text, tone: 'blue' };
}

/** The row's characters without colors or cursor, as a program reading the screen gets them. */
export function plainText(terminalRow: TerminalRow): string {
	return terminalRow.segments
		.map((segment) => {
			if (typeof segment === 'string') return segment;
			return 'text' in segment ? segment.text : '';
		})
		.join('');
}
