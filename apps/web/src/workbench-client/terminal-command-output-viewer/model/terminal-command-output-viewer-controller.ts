import type { ProjectedCommandOutputReadResult } from '@orchbee/terminal';

const MAX_TERMINAL_COMMAND_OUTPUT_MATCHES = 1_000;
export const MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH = 1_024;

export type TerminalCommandOutputViewerDocument = Exclude<
	ProjectedCommandOutputReadResult,
	{ readonly kind: 'cancelled' }
>;

export interface TerminalCommandOutputMatch {
	readonly end: number;
	readonly start: number;
}

export interface TerminalCommandOutputViewerSnapshot {
	readonly activeMatchIndex: number;
	readonly document: TerminalCommandOutputViewerDocument | null;
	readonly focusRequest: number;
	readonly isOpen: boolean;
	readonly matchLimitReached: boolean;
	readonly matches: readonly TerminalCommandOutputMatch[];
	readonly query: string;
}

export type TerminalCommandOutputSegment =
	| { readonly kind: 'text'; readonly text: string }
	| {
			readonly active: boolean;
			readonly kind: 'match';
			readonly matchIndex: number;
			readonly text: string;
	  };

/** Own framework-independent state for one immutable command-output viewer document. */
export class TerminalCommandOutputViewerController {
	readonly #listeners = new Set<() => void>();
	#disposed = false;
	#snapshot: TerminalCommandOutputViewerSnapshot = Object.freeze({
		activeMatchIndex: -1,
		document: null,
		focusRequest: 0,
		isOpen: false,
		matchLimitReached: false,
		matches: Object.freeze([]),
		query: '',
	});

	readonly getSnapshot = (): TerminalCommandOutputViewerSnapshot => this.#snapshot;

	readonly subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	open(document: TerminalCommandOutputViewerDocument): void {
		if (this.#disposed) return;
		this.#update({
			activeMatchIndex: -1,
			document: copyDocument(document),
			focusRequest: this.#snapshot.focusRequest + 1,
			isOpen: true,
			matchLimitReached: false,
			matches: Object.freeze([]),
			query: '',
		});
	}

	close(): void {
		if (this.#disposed || !this.#snapshot.isOpen) return;
		this.#update({
			activeMatchIndex: -1,
			document: null,
			focusRequest: this.#snapshot.focusRequest,
			isOpen: false,
			matchLimitReached: false,
			matches: Object.freeze([]),
			query: '',
		});
	}

	setQuery(value: string): void {
		if (this.#disposed || this.#snapshot.document?.kind !== 'output') return;
		const query = value.slice(0, MAX_TERMINAL_COMMAND_OUTPUT_QUERY_LENGTH);
		if (query === this.#snapshot.query) return;
		const result = locateMatches(this.#snapshot.document.text, query);
		this.#update({
			...this.#snapshot,
			activeMatchIndex: result.matches.length === 0 ? -1 : 0,
			matchLimitReached: result.limitReached,
			matches: result.matches,
			query,
		});
	}

	moveMatch(direction: -1 | 1): boolean {
		if (this.#disposed || this.#snapshot.matches.length === 0) return false;
		const active = this.#snapshot.activeMatchIndex;
		let next: number;
		if (active < 0) {
			next = direction > 0 ? 0 : this.#snapshot.matches.length - 1;
		} else {
			next = (active + direction + this.#snapshot.matches.length) % this.#snapshot.matches.length;
		}
		this.#update({ ...this.#snapshot, activeMatchIndex: next });
		return true;
	}

	focusQuery(): void {
		if (this.#disposed || !this.#snapshot.isOpen) return;
		this.#update({ ...this.#snapshot, focusRequest: this.#snapshot.focusRequest + 1 });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.close();
		this.#disposed = true;
		this.#listeners.clear();
	}

	#update(snapshot: TerminalCommandOutputViewerSnapshot): void {
		this.#snapshot = Object.freeze(snapshot);
		for (const listener of this.#listeners) listener();
	}
}

export function projectTerminalCommandOutputSegments(
	text: string,
	matches: readonly TerminalCommandOutputMatch[],
	activeMatchIndex: number,
): readonly TerminalCommandOutputSegment[] {
	if (matches.length === 0) return Object.freeze([{ kind: 'text', text }]);
	const segments: TerminalCommandOutputSegment[] = [];
	let offset = 0;
	for (const [matchIndex, match] of matches.entries()) {
		if (match.start > offset) {
			segments.push({ kind: 'text', text: text.slice(offset, match.start) });
		}
		segments.push({
			active: matchIndex === activeMatchIndex,
			kind: 'match',
			matchIndex,
			text: text.slice(match.start, match.end),
		});
		offset = match.end;
	}
	if (offset < text.length) segments.push({ kind: 'text', text: text.slice(offset) });
	return Object.freeze(segments);
}

function locateMatches(
	text: string,
	query: string,
): { readonly limitReached: boolean; readonly matches: readonly TerminalCommandOutputMatch[] } {
	if (query === '') return { limitReached: false, matches: Object.freeze([]) };
	const expression = new RegExp(escapeRegExp(query), 'giu');
	const matches: TerminalCommandOutputMatch[] = [];
	let limitReached = false;
	for (const match of text.matchAll(expression)) {
		const start = match.index;
		if (matches.length === MAX_TERMINAL_COMMAND_OUTPUT_MATCHES) {
			limitReached = true;
			break;
		}
		matches.push(Object.freeze({ end: start + match[0].length, start }));
	}
	return { limitReached, matches: Object.freeze(matches) };
}

function escapeRegExp(value: string): string {
	return value.replace(/[\\^$.*+?()[\]{}|]/gu, '\\$&');
}

function copyDocument(
	document: TerminalCommandOutputViewerDocument,
): TerminalCommandOutputViewerDocument {
	return Object.freeze({ ...document });
}
