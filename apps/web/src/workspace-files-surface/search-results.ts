import type {
	WorkspaceReplaceFile,
	WorkspaceSearchFile,
	WorkspaceSearchState,
} from '@orchbee/workbench-client-protocol';

type SearchLine = WorkspaceSearchFile['lines'][number];

/** One line's matches, named within the results by its file, line and preview. */
export interface SearchResultLine extends SearchLine {
	readonly key: string;
}

/** A file's matches, gathered from every entry the search reported for it. */
export interface SearchResultFile {
	readonly path: string;
	readonly lines: readonly SearchResultLine[];
	readonly matches: number;
}

/**
 * How a search stands: the Server's own states, or one the window decided. A search the person
 * stopped is `cancelled`, a text past what a request carries is `too-long`, and a search of only
 * the changed files whose change list could not be read is `changed-unavailable`.
 */
export type SearchOutcome = WorkspaceSearchState | 'cancelled' | 'too-long' | 'changed-unavailable';

export interface SearchResults {
	readonly files: readonly SearchResultFile[];
	readonly matches: number;
	readonly state: SearchOutcome;
}

export const SEARCHING: SearchResults = { files: [], matches: 0, state: 'searching' };

/** A text span in the editor, counted from 1 in lines and UTF-16 columns, as Monaco takes it. */
export interface EditorRange {
	readonly startLineNumber: number;
	readonly startColumn: number;
	readonly endLineNumber: number;
	readonly endColumn: number;
}

/** A match to open: its file and where it is. */
export interface SearchMatchTarget {
	readonly path: string;
	readonly range: EditorRange;
}

/** A match the float window's editor shows and selects; each ask has its own serial. */
export interface MatchReveal extends SearchMatchTarget {
	/** Whether the editor takes the keyboard, or it stays where it was. */
	readonly focus: boolean;
	readonly serial: number;
}

/** Where the keyboard is in the results: a file, or one match of one of its lines. */
export interface SearchActive {
	readonly path: string;
	/** The line's key, or null for the file itself. */
	readonly key: string | null;
	readonly match: number;
}

/** One drawn row of the results. */
export type SearchRow =
	| { readonly kind: 'file'; readonly file: SearchResultFile; readonly collapsed: boolean }
	| { readonly kind: 'line'; readonly file: SearchResultFile; readonly line: SearchResultLine };

function lineKey(path: string, line: SearchLine): string {
	return `${path}\n${line.line}:${line.previewStart}`;
}

/**
 * Adds what a search reported to what it found before. An entry naming the path of the file
 * before it continues that file; the order is the order found.
 */
export function appendSearchFiles(
	results: SearchResults,
	entries: readonly WorkspaceSearchFile[],
	state: SearchOutcome,
): SearchResults {
	if (entries.length === 0) return results.state === state ? results : { ...results, state };
	const files = results.files.slice();
	let matches = results.matches;
	for (const entry of entries) {
		const lines = entry.lines.map((line) => ({ ...line, key: lineKey(entry.path, line) }));
		const count = lines.reduce((total, line) => total + line.matches.length, 0);
		matches += count;
		const last = files.at(-1);
		if (last?.path === entry.path) {
			files[files.length - 1] = {
				lines: [...last.lines, ...lines],
				matches: last.matches + count,
				path: last.path,
			};
		} else {
			files.push({ lines, matches: count, path: entry.path });
		}
	}
	return { files, matches, state };
}

/** The results without one file, or without one of its lines; a file left with none goes too. */
export function dismissSearchResult(
	results: SearchResults,
	path: string,
	key: string | null,
): SearchResults {
	let matches = results.matches;
	const files: SearchResultFile[] = [];
	for (const file of results.files) {
		if (file.path !== path) {
			files.push(file);
			continue;
		}
		if (key === null) {
			matches -= file.matches;
			continue;
		}
		const removed = file.lines.find((line) => line.key === key);
		if (removed === undefined) {
			files.push(file);
			continue;
		}
		matches -= removed.matches.length;
		const lines = file.lines.filter((line) => line !== removed);
		if (lines.length > 0) {
			files.push({ lines, matches: file.matches - removed.matches.length, path });
		}
	}
	return matches === results.matches ? results : { ...results, files, matches };
}

/** The rows to draw: each file, and its lines unless it is collapsed. */
export function searchRows(
	results: SearchResults | null,
	collapsed: ReadonlySet<string>,
): readonly SearchRow[] {
	if (results === null) return [];
	const rows: SearchRow[] = [];
	for (const file of results.files) {
		const folded = collapsed.has(file.path);
		rows.push({ collapsed: folded, file, kind: 'file' });
		if (folded) continue;
		for (const line of file.lines) rows.push({ file, kind: 'line', line });
	}
	return rows;
}

/** The position of a preview's code unit: its line and its column, counted from 1. */
function positionIn(line: SearchLine, offset: number): { line: number; column: number } {
	const before = line.preview.slice(0, offset);
	const breaks = before.split('\n').length - 1;
	if (breaks === 0) return { column: line.previewStart + offset + 1, line: line.line };
	return { column: offset - before.lastIndexOf('\n'), line: line.line + breaks };
}

/** Where one match of a line is in its file; a match that spans lines ends on a later one. */
export function matchRange(line: SearchLine, match: number): EditorRange {
	const range = line.matches[match] ?? line.matches[0];
	const start = positionIn(line, range?.start ?? 0);
	const end = positionIn(line, range?.end ?? 0);
	return {
		endColumn: end.column,
		endLineNumber: end.line,
		startColumn: start.column,
		startLineNumber: start.line,
	};
}

/**
 * The match `direction` steps from `active`, wrapping at either end, as F4 and Shift+F4 do. From a
 * file the next step is its first match, and the step before it the last match of the file above.
 */
export function stepSearchMatch(
	results: SearchResults | null,
	active: SearchActive | null,
	direction: 1 | -1,
): (SearchActive & { readonly line: SearchResultLine }) | null {
	const all: { readonly path: string; readonly line: SearchResultLine; readonly match: number }[] =
		[];
	for (const file of results?.files ?? []) {
		for (const line of file.lines) {
			for (let match = 0; match < line.matches.length; match += 1) {
				all.push({ line, match, path: file.path });
			}
		}
	}
	if (all.length === 0) return null;
	let index = -1;
	if (active !== null) {
		index = all.findIndex(
			(entry) =>
				entry.path === active.path &&
				(active.key === null || (entry.line.key === active.key && entry.match === active.match)),
		);
		// On a file, the step back is from its first match.
		if (index >= 0 && active.key === null && direction === 1) index -= 1;
	}
	let next = index + direction;
	if (index < 0) next = direction === 1 ? 0 : all.length - 1;
	const entry = all[(next + all.length) % all.length];
	if (entry === undefined) return null;
	return { key: entry.line.key, line: entry.line, match: entry.match, path: entry.path };
}

/** A part of a drawn preview, a match or the text between. */
export interface PreviewPart {
	readonly text: string;
	readonly match: boolean;
	/** What a match becomes while replacing, where that is known without the file. */
	readonly replacement?: string | undefined;
}

/**
 * A line's preview as drawn: its first line only, without the indentation before its first match,
 * and an ellipsis where the preview starts within the line or goes on past what shows.
 */
export function previewParts(
	line: SearchLine,
	replacement: ((matched: string) => string) | null = null,
): readonly PreviewPart[] {
	const lineEnd = line.preview.indexOf('\n');
	const shown = (lineEnd === -1 ? line.preview : line.preview.slice(0, lineEnd)).replace(/\r$/, '');
	const firstMatch = line.matches[0]?.start ?? 0;
	const indent = Math.min(firstMatch, shown.length - shown.trimStart().length);
	const parts: PreviewPart[] = [];
	if (line.previewStart > 0) parts.push({ match: false, text: '…' });
	let at = indent;
	for (const range of line.matches) {
		if (range.start >= shown.length) break;
		if (range.start > at) parts.push({ match: false, text: shown.slice(at, range.start) });
		const end = Math.min(range.end, shown.length);
		const text = shown.slice(range.start, end);
		parts.push({
			match: true,
			replacement: replacement?.(line.preview.slice(range.start, range.end)),
			text,
		});
		at = end;
	}
	if (at < shown.length) parts.push({ match: false, text: shown.slice(at) });
	if (lineEnd !== -1) parts.push({ match: false, text: ' …' });
	return parts;
}

/**
 * Splits a list of globs at its commas, keeping a comma inside braces, as in `*.{ts,tsx}`; blank
 * entries are dropped.
 */
export function splitGlobs(text: string): readonly string[] {
	const globs: string[] = [];
	let depth = 0;
	let start = 0;
	for (let index = 0; index <= text.length; index += 1) {
		const character = text[index];
		if (character === '{') depth += 1;
		else if (character === '}') depth = Math.max(0, depth - 1);
		else if (character === undefined || (character === ',' && depth === 0)) {
			const glob = text.slice(start, index).trim();
			if (glob.length > 0) globs.push(glob);
			start = index + 1;
		}
	}
	return globs;
}

/** Results as text: each file's path, then its lines with their numbers, a blank line apart. */
export function searchResultsText(files: readonly SearchResultFile[]): string {
	return files
		.map((file) =>
			[file.path, ...file.lines.map((line) => `  ${line.line}: ${searchLineText(line)}`)].join(
				'\n',
			),
		)
		.join('\n\n');
}

/** A line's text as it shows: its first line, trimmed. */
export function searchLineText(line: SearchLine): string {
	const lineEnd = line.preview.indexOf('\n');
	return (lineEnd === -1 ? line.preview : line.preview.slice(0, lineEnd)).trim();
}

/** What a replacement names: a file, one of its lines, or everything shown. */
export interface ReplaceTarget {
	readonly path: string;
	/** The line's key, or null for every line of the file. */
	readonly key: string | null;
}

/** The files and lines a replacement names, in the order shown. */
export function replacedLines(
	results: SearchResults,
	target: ReplaceTarget | null,
): readonly SearchResultFile[] {
	if (target === null) return results.files;
	const file = results.files.find((candidate) => candidate.path === target.path);
	if (file === undefined) return [];
	if (target.key === null) return [file];
	const lines = file.lines.filter((line) => line.key === target.key);
	if (lines.length === 0) return [];
	return [
		{
			lines,
			matches: lines.reduce((total, line) => total + line.matches.length, 0),
			path: file.path,
		},
	];
}

/**
 * Where each match of `file` starts, as a replacement names it: its line and the UTF-16 code units
 * of that line before it, in order and once each.
 */
export function replaceFileOf(file: SearchResultFile): WorkspaceReplaceFile {
	const starts = new Map<string, { readonly line: number; readonly column: number }>();
	for (const line of file.lines) {
		for (const range of line.matches) {
			const column = line.previewStart + range.start;
			starts.set(`${line.line}:${column}`, { column, line: line.line });
		}
	}
	const matches = [...starts.values()].sort(
		(left, right) => left.line - right.line || left.column - right.column,
	);
	return { matches, path: file.path };
}

/**
 * The results with the files a later search of `paths` found in place of what they held; a file
 * it found nothing in goes, and one it found that was not shown is left out.
 */
export function researchedFiles(
	results: SearchResults,
	paths: ReadonlySet<string>,
	found: readonly SearchResultFile[],
): SearchResults {
	const byPath = new Map(found.map((file) => [file.path, file]));
	const files: SearchResultFile[] = [];
	let matches = 0;
	for (const file of results.files) {
		const next = paths.has(file.path) ? byPath.get(file.path) : file;
		if (next === undefined) continue;
		files.push(next);
		matches += next.matches;
	}
	return { ...results, files, matches };
}

/**
 * What a replacement inserts for every match when that is known without the file: the text as it
 * stands, or, for a regular expression, with its escapes read, unless it inserts a group. Null when
 * it inserts one. Read as the Server reads it.
 */
export function fixedReplacement(replacement: string, regex: boolean): string | null {
	if (!regex) return replacement;
	let text = '';
	for (let index = 0; index < replacement.length; index += 1) {
		const character = replacement[index] ?? '';
		const next = replacement[index + 1];
		if (character === '\\') {
			if (next === undefined) {
				text += '\\';
				continue;
			}
			index += 1;
			if (next === '\\') text += '\\';
			else if (next === 'n') text += '\n';
			else if (next === 't') text += '\t';
			else if (!'ulUL'.includes(next)) text += `\\${next}`;
		} else if (character === '$') {
			const rest = replacement.slice(index + 1);
			if (/^(?:[&0-9]|\{[\p{L}\p{N}_]+\}|<[\p{L}\p{N}_]+>)/u.test(rest)) return null;
			text += '$';
			if (next === '$') index += 1;
		} else {
			text += character;
		}
	}
	return text;
}

/** Whether both texts hold `separator` and as many parts around it. */
function sameParts(matched: string, replacement: string, separator: string): boolean {
	return (
		matched.includes(separator) &&
		replacement.includes(separator) &&
		matched.split(separator).length === replacement.split(separator).length
	);
}

/**
 * `replacement` in the case of `matched`, as the Server keeps it: all upper or all lower where it
 * is, a capital or small first letter where it starts so, and part by part where both hold the
 * same number of hyphens, or of underscores, and not the other.
 */
export function withCaseOf(matched: string, replacement: string): string {
	if (matched.length === 0) return replacement;
	const hyphens = sameParts(matched, replacement, '-');
	const underscores = sameParts(matched, replacement, '_');
	if (hyphens !== underscores) {
		const separator = hyphens ? '-' : '_';
		const wanted = replacement.split(separator);
		return matched
			.split(separator)
			.map((part, index) => withCaseOf(part, wanted[index] ?? ''))
			.join(separator);
	}
	if (matched.toUpperCase() === matched) return replacement.toUpperCase();
	if (matched.toLowerCase() === matched) return replacement.toLowerCase();
	const [first = ''] = matched;
	const [head = '', ...tail] = replacement;
	if (first !== first.toLowerCase()) return head.toUpperCase() + tail.join('');
	if (first !== first.toUpperCase()) return head.toLowerCase() + tail.join('');
	return replacement;
}
