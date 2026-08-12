import type { TerminalBufferRow } from '../buffer-rows';
import {
	resolveTerminalSearchResultLimit,
	visitTerminalBufferMatches,
	type TerminalDirectionalSearchOptions,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from './terminal-search';

export interface ProjectedSearchAccumulatorResult {
	readonly activeMatch: TerminalSearchMatch | null;
	readonly matches: readonly TerminalSearchMatch[];
}

/** Incrementally scan completed logical lines while preserving cross-page soft wraps. */
export class ProjectedSearchAccumulator {
	readonly #navigation: TerminalDirectionalSearchOptions;
	readonly #options: TerminalSearchOptions;
	readonly #query: string;
	readonly #resultLimit: number;
	#activeMatch: TerminalSearchMatch | null = null;
	#activeMatchLocked = false;
	#firstMatch: TerminalSearchMatch | null = null;
	#lastMatch: TerminalSearchMatch | null = null;
	#matches: TerminalSearchMatch[] = [];
	#pendingRows: TerminalBufferRow[] = [];

	constructor(
		query: string,
		options: TerminalSearchOptions,
		navigation: TerminalDirectionalSearchOptions = {},
	) {
		this.#query = query;
		this.#options = { ...options };
		this.#navigation = {
			...(navigation.preserveStart === undefined
				? {}
				: { preserveStart: navigation.preserveStart }),
			...(navigation.reverse === undefined ? {} : { reverse: navigation.reverse }),
			...(navigation.start === undefined ? {} : { start: { ...navigation.start } }),
		};
		this.#resultLimit = resolveTerminalSearchResultLimit(options.maxResults);
	}

	push(
		rows: readonly TerminalBufferRow[],
		final: boolean,
	): ProjectedSearchAccumulatorResult | null {
		this.#pendingRows.push(...rows);
		const completedRows = this.#completedRowCount(final);
		if (completedRows > 0) {
			visitTerminalBufferMatches(
				this.#pendingRows.slice(0, completedRows),
				this.#query,
				this.#options,
				(match) => {
					this.#recordMatch(match);
					return false;
				},
			);
			this.#pendingRows.splice(0, completedRows);
		}
		if (!final) return null;
		this.#pendingRows = [];
		return {
			activeMatch:
				this.#activeMatch ?? (this.#navigation.reverse ? this.#lastMatch : this.#firstMatch),
			matches: this.#matches,
		};
	}

	#recordMatch(match: TerminalSearchMatch): void {
		if (this.#matches.length < this.#resultLimit) this.#matches.push(match);
		this.#firstMatch ??= match;
		this.#lastMatch = match;
		if (this.#activeMatchLocked) return;
		const start = this.#navigation.start;
		if (start === undefined) {
			if (this.#navigation.reverse) this.#activeMatch = match;
			else this.#activeMatch ??= match;
			return;
		}
		const comparison = compareMatchStart(match, start);
		if (this.#navigation.preserveStart && comparison === 0) {
			this.#activeMatch = match;
			this.#activeMatchLocked = true;
			return;
		}
		if (this.#navigation.reverse) {
			if (comparison < 0) this.#activeMatch = match;
			return;
		}
		if (comparison > 0) {
			this.#activeMatch = match;
			this.#activeMatchLocked = true;
		}
	}

	#completedRowCount(final: boolean): number {
		if (final) return this.#pendingRows.length;
		if (this.#options.spanLines) return 0;
		let completed = 0;
		for (const [index, row] of this.#pendingRows.entries()) {
			if (!row.wrap) completed = index + 1;
		}
		return completed;
	}
}

function compareMatchStart(
	match: TerminalSearchMatch,
	start: NonNullable<TerminalDirectionalSearchOptions['start']>,
): number {
	return match.start.row === start.row
		? match.start.column - start.column
		: match.start.row - start.row;
}
