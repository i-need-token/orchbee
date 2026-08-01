import type { TerminalBufferRow } from '../buffer-rows';
import {
	findTerminalBufferMatches,
	resolveTerminalSearchResultLimit,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from './terminal-search';

/** Incrementally scan completed logical lines while preserving cross-page soft wraps. */
export class ProjectedSearchAccumulator {
	readonly #options: TerminalSearchOptions;
	readonly #query: string;
	readonly #resultLimit: number;
	#matches: TerminalSearchMatch[] = [];
	#pendingRows: TerminalBufferRow[] = [];

	constructor(query: string, options: TerminalSearchOptions) {
		this.#query = query;
		this.#options = { ...options };
		this.#resultLimit = resolveTerminalSearchResultLimit(options.maxResults);
	}

	push(rows: readonly TerminalBufferRow[], final: boolean): readonly TerminalSearchMatch[] | null {
		this.#pendingRows.push(...rows);
		if (this.#matches.length < this.#resultLimit) {
			const completedRows = this.#completedRowCount(final);
			if (completedRows > 0) {
				const matches = findTerminalBufferMatches(
					this.#pendingRows.slice(0, completedRows),
					this.#query,
					{
						...this.#options,
						maxResults: this.#resultLimit - this.#matches.length,
					},
				);
				this.#matches.push(...matches);
				this.#pendingRows.splice(0, completedRows);
			}
		} else {
			this.#pendingRows = [];
		}
		if (!final) return null;
		this.#pendingRows = [];
		return this.#matches;
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
