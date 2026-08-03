import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';

import { TerminalBufferRowDetail, type TerminalBufferRowReader } from '../buffer-rows';
import {
	TERMINAL_SEARCH_DEBOUNCE_MS,
	TERMINAL_SEARCH_RESULT_LIMIT,
	resolveTerminalSearchResultLimit,
	validateTerminalSearchQuery,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from './terminal-search';
import type {
	ProjectedSearchWorkerFactory,
	ProjectedSearchWorkerLike,
	ProjectedSearchWorkerResponse,
} from './projected-search-protocol';

const SEARCH_WATCHDOG_MILLISECONDS = 10_000;
const SEARCH_INITIAL_PAGE_ROWS = 1400;
const SEARCH_PAGE_BYTE_TARGET = Math.floor(PROTOCOL_LIMITS.maxRowWindowBytes * 0.95);
const SEARCH_PAGE_ENTRY_TARGET = Math.floor(PROTOCOL_LIMITS.maxFrameCells * 0.95);

export interface ProjectedTerminalSearchEvent {
	readonly activeIndex: number;
	readonly errorKind: 'invalid-query' | 'unavailable' | null;
	readonly generation: bigint;
	readonly matches: readonly TerminalSearchMatch[];
	readonly query: string;
	readonly regexError: string | null;
	readonly resultLimitReached: boolean;
	readonly revision: bigint;
	readonly state: 'complete' | 'error' | 'idle' | 'searching' | 'timeout';
}

export interface ProjectedSearchControllerOptions {
	readonly createWorker: ProjectedSearchWorkerFactory;
	readonly onChange: (event: ProjectedTerminalSearchEvent) => void;
	readonly onReveal: (match: TerminalSearchMatch) => void;
	readonly reader: TerminalBufferRowReader;
}

export class ProjectedSearchController {
	readonly #options: ProjectedSearchControllerOptions;
	#abortController: AbortController | undefined;
	#activeIndex = -1;
	#debounce: ReturnType<typeof setTimeout> | undefined;
	#disposed = false;
	#errorKind: ProjectedTerminalSearchEvent['errorKind'] = null;
	#generation = 1n;
	#matches: readonly TerminalSearchMatch[] = [];
	#query = '';
	#regexError: string | null = null;
	#revision = 0n;
	#searchId = 0;
	#searchOptions: TerminalSearchOptions = {};
	#state: ProjectedTerminalSearchEvent['state'] = 'idle';
	#watchdog: ReturnType<typeof setTimeout> | undefined;
	#worker: ProjectedSearchWorkerLike | undefined;

	constructor(options: ProjectedSearchControllerOptions) {
		this.#options = options;
	}

	get event(): ProjectedTerminalSearchEvent {
		return this.#event();
	}

	update(query: string, options: TerminalSearchOptions = {}): void {
		if (this.#disposed) return;
		this.#query = query;
		this.#searchOptions = { ...options };
		this.#cancelScan();
		this.#matches = [];
		this.#activeIndex = -1;
		if (!query) {
			this.#emit('idle', null);
			return;
		}
		const regexError = validateTerminalSearchQuery(query, options);
		if (regexError !== null) {
			this.#emit('error', regexError, 'invalid-query');
			return;
		}
		if (new TextEncoder().encode(query).byteLength > PROTOCOL_LIMITS.maxSearchQueryBytes) {
			this.#emit('error', 'Search query exceeds its bounded limit.', 'invalid-query');
			return;
		}
		this.#emit('searching', null);
		this.#debounce = setTimeout(() => {
			this.#debounce = undefined;
			void this.#run();
		}, TERMINAL_SEARCH_DEBOUNCE_MS);
	}

	invalidate(): void {
		if (this.#query) this.update(this.#query, this.#searchOptions);
	}

	next(reverse = false): TerminalSearchMatch | null {
		if (this.#matches.length === 0) return null;
		this.#activeIndex = reverse
			? (this.#activeIndex - 1 + this.#matches.length) % this.#matches.length
			: (this.#activeIndex + 1) % this.#matches.length;
		const match = this.#matches[this.#activeIndex] ?? null;
		if (match !== null) this.#options.onReveal(match);
		this.#emit('complete', null);
		return match;
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#cancelScan();
		this.#matches = [];
	}

	async #run(): Promise<void> {
		if (this.#disposed || !this.#query) return;
		const snapshot = this.#options.reader.getBufferRowsSnapshot();
		const searchId = ++this.#searchId;
		this.#generation = snapshot.generation;
		this.#revision = snapshot.revision;
		this.#matches = [];
		this.#activeIndex = -1;
		this.#emit('searching', null);
		const abortController = new AbortController();
		this.#abortController = abortController;
		let worker: ProjectedSearchWorkerLike;
		try {
			worker = this.#options.createWorker();
		} catch (error) {
			this.#completeError(
				searchId,
				error instanceof Error ? error.message : 'Terminal search Worker is unavailable.',
			);
			return;
		}
		this.#worker = worker;
		worker.addEventListener('message', (event) => this.#receive(searchId, event.data));
		worker.addEventListener('error', (event) => {
			this.#completeError(searchId, event.message || 'Terminal search Worker failed.');
		});
		this.#watchdog = setTimeout(() => {
			if (this.#searchId !== searchId) return;
			this.#abortController?.abort();
			this.#abortController = undefined;
			this.#cancelWorker();
			this.#emit('timeout', null);
		}, SEARCH_WATCHDOG_MILLISECONDS);
		worker.postMessage({
			generation: snapshot.generation.toString(),
			kind: 'start',
			options: this.#searchOptions,
			query: this.#query,
			revision: snapshot.revision.toString(),
			searchId,
		});

		let startRow = 0n;
		let rowCount = SEARCH_INITIAL_PAGE_ROWS;
		try {
			if (snapshot.totalRows === 0n) {
				worker.postMessage({
					final: true,
					generation: snapshot.generation.toString(),
					kind: 'page',
					revision: snapshot.revision.toString(),
					rows: [],
					searchId,
				});
				return;
			}
			while (startRow < snapshot.totalRows && this.#searchId === searchId) {
				const result = await this.#options.reader.readBufferRows(
					{
						detail: TerminalBufferRowDetail.text,
						generation: snapshot.generation,
						maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
						purpose: 'search',
						revision: snapshot.revision,
						rowCount: Math.min(rowCount, Number(snapshot.totalRows - startRow)),
						startRow,
					},
					abortController.signal,
				);
				if (this.#searchId !== searchId || abortController.signal.aborted) return;
				if (result.kind === 'too-large' && rowCount > 1) {
					rowCount = Math.max(1, Math.floor(rowCount / 2));
					continue;
				}
				if (result.kind !== 'rows') {
					this.#completeError(searchId, `Terminal search source became ${result.kind}.`);
					return;
				}
				if (result.rows.length > 0 && result.byteLength > 0) {
					const entryCount = result.rows.reduce(
						(total, row) => total + (row.identityAscii ? row.text.length : row.mappings.length),
						0,
					);
					const rowsByEntries =
						entryCount === 0
							? PROTOCOL_LIMITS.maxRowWindowRows
							: Math.floor((SEARCH_PAGE_ENTRY_TARGET * result.rows.length) / entryCount);
					rowCount = Math.max(
						1,
						Math.min(
							PROTOCOL_LIMITS.maxRowWindowRows,
							Math.floor((SEARCH_PAGE_BYTE_TARGET * result.rows.length) / result.byteLength),
							rowsByEntries,
						),
					);
				}
				const nextRow = result.startRow + BigInt(result.rows.length);
				worker.postMessage({
					final: nextRow >= snapshot.totalRows,
					generation: snapshot.generation.toString(),
					kind: 'page',
					revision: snapshot.revision.toString(),
					rows: result.rows,
					searchId,
				});
				if (result.rows.length === 0) {
					this.#completeError(searchId, 'Terminal search source returned an empty page.');
					return;
				}
				startRow = nextRow;
			}
		} catch (error) {
			this.#completeError(
				searchId,
				error instanceof Error ? error.message : 'Terminal search source failed.',
			);
		}
	}

	#receive(searchId: number, response: ProjectedSearchWorkerResponse): void {
		if (this.#disposed || this.#searchId !== searchId || response.searchId !== searchId) return;
		if (response.kind === 'error') {
			this.#completeError(searchId, response.message);
			return;
		}
		if (
			response.revision !== this.#revision.toString() ||
			response.generation !== this.#generation.toString()
		) {
			this.#completeError(searchId, 'Terminal search result revision is stale.');
			return;
		}
		this.#matches = response.matches;
		this.#activeIndex = response.matches.length === 0 ? -1 : 0;
		this.#cancelWorker();
		const first = response.matches[0];
		if (first !== undefined) this.#options.onReveal(first);
		this.#emit('complete', null);
	}

	#completeError(searchId: number, message: string): void {
		if (this.#searchId !== searchId) return;
		this.#matches = [];
		this.#activeIndex = -1;
		this.#abortController?.abort();
		this.#abortController = undefined;
		this.#cancelWorker();
		this.#emit('error', message);
	}

	#event(): ProjectedTerminalSearchEvent {
		return {
			activeIndex: this.#activeIndex,
			errorKind: this.#errorKind,
			generation: this.#generation,
			matches: this.#matches,
			query: this.#query,
			regexError: this.#regexError,
			resultLimitReached:
				this.#matches.length > 0 &&
				this.#matches.length >=
					Math.min(
						TERMINAL_SEARCH_RESULT_LIMIT,
						resolveTerminalSearchResultLimit(this.#searchOptions.maxResults),
					),
			revision: this.#revision,
			state: this.#state,
		};
	}

	#emit(
		state: ProjectedTerminalSearchEvent['state'],
		regexError: string | null,
		errorKind: ProjectedTerminalSearchEvent['errorKind'] = state === 'error' ? 'unavailable' : null,
	): void {
		this.#state = state;
		this.#regexError = regexError;
		this.#errorKind = errorKind;
		this.#options.onChange(this.#event());
	}

	#cancelScan(): void {
		if (this.#debounce !== undefined) clearTimeout(this.#debounce);
		this.#debounce = undefined;
		this.#abortController?.abort();
		this.#abortController = undefined;
		this.#cancelWorker();
		this.#searchId += 1;
	}

	#cancelWorker(): void {
		if (this.#watchdog !== undefined) clearTimeout(this.#watchdog);
		this.#watchdog = undefined;
		this.#worker?.terminate();
		this.#worker = undefined;
	}
}

export function createProjectedSearchWorker(): ProjectedSearchWorkerLike {
	const worker: ProjectedSearchWorkerLike = new Worker(
		new URL('./projected-search-worker', import.meta.url),
		{
			type: 'module',
		},
	);
	return worker;
}
