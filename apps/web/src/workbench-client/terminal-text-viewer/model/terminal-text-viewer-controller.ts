import type {
	ProjectedCommandOutputReadResult,
	ProjectedScrollbackReadResult,
} from '@orchbee/terminal';

const MAX_TERMINAL_TEXT_VIEWER_MATCHES = 1_000;
export const MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH = 1_024;

export type TerminalTextViewerSource = 'command-output' | 'scrollback';

export type TerminalTextViewerDocument =
	| {
			readonly initialTopOffset: number | null;
			readonly kind: 'text';
			readonly notice: 'truncated-start' | null;
			readonly source: TerminalTextViewerSource;
			readonly text: string;
	  }
	| {
			readonly kind: 'empty';
			readonly reason: 'no-output' | 'no-scrollback' | 'no-shell-integration';
			readonly source: TerminalTextViewerSource;
	  }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'alternate-screen' | 'source-limit' | 'stale' | 'unavailable';
			readonly source: TerminalTextViewerSource;
	  };

export interface TerminalTextViewerMatch {
	readonly end: number;
	readonly start: number;
}

export interface TerminalTextViewerSnapshot {
	readonly activeMatchIndex: number;
	readonly document: TerminalTextViewerDocument | null;
	readonly focusRequest: number;
	readonly isOpen: boolean;
	readonly matchLimitReached: boolean;
	readonly matches: readonly TerminalTextViewerMatch[];
	readonly query: string;
}

export type TerminalTextViewerSegment =
	| { readonly kind: 'anchor' }
	| { readonly kind: 'text'; readonly text: string }
	| {
			readonly active: boolean;
			readonly kind: 'match';
			readonly matchIndex: number;
			readonly text: string;
	  };

/** Own framework-independent state for one immutable terminal-text viewer document. */
export class TerminalTextViewerController {
	readonly #listeners = new Set<() => void>();
	#disposed = false;
	#snapshot: TerminalTextViewerSnapshot = Object.freeze({
		activeMatchIndex: -1,
		document: null,
		focusRequest: 0,
		isOpen: false,
		matchLimitReached: false,
		matches: Object.freeze([]),
		query: '',
	});

	readonly getSnapshot = (): TerminalTextViewerSnapshot => this.#snapshot;

	readonly subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	open(document: TerminalTextViewerDocument): void {
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
		if (this.#disposed || this.#snapshot.document?.kind !== 'text') return;
		const query = value.slice(0, MAX_TERMINAL_TEXT_VIEWER_QUERY_LENGTH);
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

	#update(snapshot: TerminalTextViewerSnapshot): void {
		this.#snapshot = Object.freeze(snapshot);
		for (const listener of this.#listeners) listener();
	}
}

export function projectTerminalTextViewerSegments(
	text: string,
	matches: readonly TerminalTextViewerMatch[],
	activeMatchIndex: number,
	initialTopOffset: number | null,
): readonly TerminalTextViewerSegment[] {
	const anchor =
		initialTopOffset === null
			? null
			: Math.max(0, Math.min(Math.trunc(initialTopOffset), text.length));
	const boundaries = new Set<number>([0, text.length]);
	for (const match of matches) {
		boundaries.add(match.start);
		boundaries.add(match.end);
	}
	if (anchor !== null) boundaries.add(anchor);
	const orderedBoundaries = [...boundaries].sort((left, right) => left - right);
	const segments: TerminalTextViewerSegment[] = [];
	for (let index = 0; index < orderedBoundaries.length - 1; index += 1) {
		const start = orderedBoundaries[index];
		const end = orderedBoundaries[index + 1];
		if (start === undefined || end === undefined) continue;
		if (start === anchor) segments.push({ kind: 'anchor' });
		if (start === end) continue;
		const matchIndex = matches.findIndex((match) => start >= match.start && start < match.end);
		segments.push(
			matchIndex < 0
				? { kind: 'text', text: text.slice(start, end) }
				: {
						active: matchIndex === activeMatchIndex,
						kind: 'match',
						matchIndex,
						text: text.slice(start, end),
					},
		);
	}
	if (anchor === text.length) segments.push({ kind: 'anchor' });
	return Object.freeze(segments);
}

function locateMatches(
	text: string,
	query: string,
): { readonly limitReached: boolean; readonly matches: readonly TerminalTextViewerMatch[] } {
	if (query === '') return { limitReached: false, matches: Object.freeze([]) };
	const expression = new RegExp(escapeRegExp(query), 'giu');
	const matches: TerminalTextViewerMatch[] = [];
	let limitReached = false;
	for (const match of text.matchAll(expression)) {
		const start = match.index;
		if (matches.length === MAX_TERMINAL_TEXT_VIEWER_MATCHES) {
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

function copyDocument(document: TerminalTextViewerDocument): TerminalTextViewerDocument {
	return document.kind === 'text'
		? Object.freeze({
				...document,
				initialTopOffset:
					document.initialTopOffset === null
						? null
						: Math.max(0, Math.min(Math.trunc(document.initialTopOffset), document.text.length)),
			})
		: Object.freeze({ ...document });
}

export function commandOutputViewerDocument(
	result: Exclude<ProjectedCommandOutputReadResult, { readonly kind: 'cancelled' }>,
): TerminalTextViewerDocument {
	if (result.kind === 'output') {
		return {
			initialTopOffset: null,
			kind: 'text',
			notice: result.truncatedStart ? 'truncated-start' : null,
			source: 'command-output',
			text: result.text,
		};
	}
	return { ...result, source: 'command-output' };
}

export function scrollbackViewerDocument(
	result: Exclude<ProjectedScrollbackReadResult, { readonly kind: 'cancelled' }>,
): TerminalTextViewerDocument {
	if (result.kind === 'scrollback') {
		return {
			initialTopOffset: result.initialTopOffset,
			kind: 'text',
			notice: null,
			source: 'scrollback',
			text: result.text,
		};
	}
	return { ...result, source: 'scrollback' };
}
