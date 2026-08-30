import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';

import { TerminalBufferRowDetail, type TerminalBufferRowReader } from '../buffer-rows';
import {
	TERMINAL_SEARCH_DEBOUNCE_MS,
	TERMINAL_SEARCH_RESULT_LIMIT,
	resolveTerminalSearchResultLimit,
	validateTerminalSearchQuery,
	type TerminalDirectionalSearchOptions,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
	type TerminalSearchPosition,
} from './terminal-search';
import {
	PROJECTED_SEARCH_SOURCE_BYTE_LIMIT,
	type ProjectedSearchWorkerFactory,
	type ProjectedSearchWorkerLike,
	type ProjectedSearchWorkerResponse,
} from './projected-search-protocol';

const SEARCH_WATCHDOG_MILLISECONDS = 10_000;
const SEARCH_REVISION_REFRESH_DEBOUNCE_MILLISECONDS = 200;
const SEARCH_INITIAL_PAGE_ROWS = 1400;
const SEARCH_PAGE_BYTE_TARGET = Math.floor(
	Math.min(PROTOCOL_LIMITS.maxRowWindowBytes, PROJECTED_SEARCH_SOURCE_BYTE_LIMIT) * 0.95,
);
const SEARCH_PAGE_ENTRY_TARGET = Math.floor(PROTOCOL_LIMITS.maxFrameCells * 0.95);

export interface ProjectedTerminalSearchEvent {
	readonly activeMatch: TerminalSearchMatch | null;
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
	readonly onActivate: (
		match: TerminalSearchMatch,
		source: { readonly generation: bigint; readonly revision: bigint },
		reveal: boolean,
	) => void;
	readonly onChange: (event: ProjectedTerminalSearchEvent) => void;
	readonly reader: TerminalBufferRowReader;
}

interface WorkerProgressWaiter {
	readonly nextRow: bigint;
	readonly resolve: (progressed: boolean) => void;
	readonly searchId: number;
}

interface SearchRestartPolicy {
	readonly debounceMilliseconds: number;
	readonly revealActiveMatch: boolean;
}

type SearchPageResult = Awaited<ReturnType<TerminalBufferRowReader['readBufferRows']>>;
type SettledSearchPage =
	| { readonly kind: 'cancelled' }
	| { readonly error: unknown; readonly kind: 'failed' }
	| { readonly kind: 'completed'; readonly value: SearchPageResult };

export class ProjectedSearchController {
	readonly #options: ProjectedSearchControllerOptions;
	#abortController: AbortController | undefined;
	#activeMatch: TerminalSearchMatch | null = null;
	#activeIndex = -1;
	#debounce: ReturnType<typeof setTimeout> | undefined;
	#disposed = false;
	#errorKind: ProjectedTerminalSearchEvent['errorKind'] = null;
	#generation = 1n;
	#inFlightNavigationStep = 0;
	#matches: readonly TerminalSearchMatch[] = [];
	#navigationAnchor: TerminalSearchPosition | undefined;
	#pendingNavigationOffset = 0;
	#query = '';
	#refreshPending = false;
	#regexError: string | null = null;
	#revision = 0n;
	#searchId = 0;
	#searchOptions: TerminalSearchOptions = {};
	#state: ProjectedTerminalSearchEvent['state'] = 'idle';
	#watchdog: ReturnType<typeof setTimeout> | undefined;
	#worker: ProjectedSearchWorkerLike | undefined;
	#workerProgress: WorkerProgressWaiter | undefined;

	constructor(options: ProjectedSearchControllerOptions) {
		this.#options = options;
	}

	get event(): ProjectedTerminalSearchEvent {
		return this.#event();
	}

	update(
		query: string,
		options: TerminalSearchOptions = {},
		anchor: TerminalSearchPosition | undefined = this.#navigationAnchor,
	): void {
		this.#restart(query, options, anchor, {
			debounceMilliseconds: TERMINAL_SEARCH_DEBOUNCE_MS,
			revealActiveMatch: true,
		});
	}

	invalidate(): void {
		if (this.#disposed || !this.#query) return;
		if (this.#debounce !== undefined) return;
		if (this.#scanActive()) {
			this.#refreshPending = true;
			return;
		}
		this.#scheduleRevisionRefresh();
	}

	next(reverse = false): boolean {
		if (
			this.#disposed ||
			!this.#query ||
			(this.#state !== 'complete' && this.#state !== 'searching') ||
			(this.#state === 'complete' && this.#activeMatch === null)
		) {
			return false;
		}
		this.#pendingNavigationOffset += reverse ? -1 : 1;
		this.#drainNavigation();
		return true;
	}

	setNavigationAnchor(anchor: TerminalSearchPosition | undefined): void {
		if (this.#disposed) return;
		this.#navigationAnchor = anchor === undefined ? undefined : { ...anchor };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#cancelScan();
		this.#inFlightNavigationStep = 0;
		this.#matches = [];
		this.#pendingNavigationOffset = 0;
		this.#activeMatch = null;
	}

	#restart(
		query: string,
		options: TerminalSearchOptions,
		anchor: TerminalSearchPosition | undefined,
		policy: SearchRestartPolicy,
	): void {
		if (this.#disposed) return;
		this.#query = query;
		this.#searchOptions = { ...options };
		this.#navigationAnchor = anchor === undefined ? undefined : { ...anchor };
		this.#cancelScan();
		this.#refreshPending = false;
		this.#inFlightNavigationStep = 0;
		this.#matches = [];
		this.#pendingNavigationOffset = 0;
		this.#activeMatch = null;
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
			void this.#run(this.#initialNavigation(), false, policy.revealActiveMatch);
		}, policy.debounceMilliseconds);
	}

	async #run(
		navigation: TerminalDirectionalSearchOptions,
		preserveResults = false,
		revealActiveMatch = true,
	): Promise<void> {
		if (this.#disposed || !this.#query) return;
		const snapshot = this.#options.reader.getBufferRowsSnapshot();
		const searchId = ++this.#searchId;
		this.#refreshPending = false;
		this.#generation = snapshot.generation;
		this.#revision = snapshot.revision;
		if (!preserveResults) {
			this.#matches = [];
			this.#activeMatch = null;
			this.#activeIndex = -1;
		}
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
		worker.addEventListener('message', (event) =>
			this.#receive(searchId, event.data, revealActiveMatch),
		);
		worker.addEventListener('error', (event) => {
			this.#completeError(searchId, event.message || 'Terminal search Worker failed.');
		});
		this.#refreshWatchdog(searchId);
		worker.postMessage({
			generation: snapshot.generation.toString(),
			kind: 'start',
			navigation,
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
			let pendingRead: Promise<SettledSearchPage> = settleSearchPage(
				this.#readSearchPage(snapshot, startRow, rowCount, abortController.signal),
				abortController.signal,
			);
			while (startRow < snapshot.totalRows && this.#searchId === searchId) {
				const read = await pendingRead;
				if (read.kind === 'cancelled') return;
				if (read.kind === 'failed') throw read.error;
				const result = read.value;
				if (this.#searchId !== searchId || abortController.signal.aborted) return;
				this.#refreshWatchdog(searchId);
				if (result.kind === 'too-large' && rowCount > 1) {
					rowCount = Math.max(1, Math.floor(rowCount / 2));
					pendingRead = settleSearchPage(
						this.#readSearchPage(snapshot, startRow, rowCount, abortController.signal),
						abortController.signal,
					);
					continue;
				}
				if (result.kind === 'evicted' || result.kind === 'stale') {
					this.#retireObsoleteScan(searchId);
					return;
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
				if (result.rows.length === 0) {
					this.#completeError(searchId, 'Terminal search source returned an empty page.');
					return;
				}
				const nextRow = result.startRow + BigInt(result.rows.length);
				const final = nextRow >= snapshot.totalRows;
				const progress = final ? undefined : this.#waitForWorkerProgress(searchId, nextRow);
				worker.postMessage({
					final,
					generation: snapshot.generation.toString(),
					kind: 'page',
					revision: snapshot.revision.toString(),
					rows: result.rows,
					searchId,
				});
				if (final) return;
				if (progress === undefined) return;
				startRow = nextRow;
				pendingRead = admitPrefetch(
					progress,
					settleSearchPage(
						this.#readSearchPage(snapshot, startRow, rowCount, abortController.signal),
						abortController.signal,
					),
				);
			}
		} catch (error) {
			this.#completeError(
				searchId,
				error instanceof Error ? error.message : 'Terminal search source failed.',
			);
		}
	}

	#receive(
		searchId: number,
		response: ProjectedSearchWorkerResponse,
		revealActiveMatch: boolean,
	): void {
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
		if (response.kind === 'progress') {
			this.#receiveWorkerProgress(searchId, response.nextRow);
			return;
		}
		const current = this.#options.reader.getBufferRowsSnapshot();
		if (
			this.#refreshPending ||
			current.generation !== this.#generation ||
			current.revision !== this.#revision
		) {
			this.#retireObsoleteScan(searchId);
			return;
		}
		if (this.#workerProgress !== undefined) {
			this.#completeError(searchId, 'Terminal search Worker completed before page admission.');
			return;
		}
		this.#matches = response.matches;
		this.#activeMatch = response.activeMatch;
		this.#activeIndex = findMatchIndex(response.matches, response.activeMatch);
		if (response.activeMatch !== null) {
			this.#navigationAnchor = { ...response.activeMatch.start };
		}
		this.#inFlightNavigationStep = 0;
		this.#refreshPending = false;
		this.#abortController = undefined;
		this.#cancelWorker();
		if (response.activeMatch !== null) {
			this.#options.onActivate(
				response.activeMatch,
				{
					generation: this.#generation,
					revision: this.#revision,
				},
				revealActiveMatch,
			);
		}
		this.#emit('complete', null);
		this.#drainNavigation();
	}

	#completeError(searchId: number, message: string): void {
		if (this.#searchId !== searchId) return;
		this.#matches = [];
		this.#activeMatch = null;
		this.#activeIndex = -1;
		this.#inFlightNavigationStep = 0;
		this.#pendingNavigationOffset = 0;
		this.#refreshPending = false;
		this.#retireScan(searchId);
		this.#emit('error', message);
	}

	#retireObsoleteScan(searchId: number): void {
		if (this.#searchId !== searchId) return;
		this.#pendingNavigationOffset += this.#inFlightNavigationStep;
		this.#inFlightNavigationStep = 0;
		this.#retireScan(searchId);
		this.#refreshPending = false;
		this.#scheduleRevisionRefresh();
	}

	#scheduleRevisionRefresh(): void {
		if (this.#disposed || !this.#query || this.#debounce !== undefined || this.#scanActive()) {
			return;
		}
		this.#debounce = setTimeout(() => {
			this.#debounce = undefined;
			void this.#run(this.#initialNavigation(), true, false);
		}, SEARCH_REVISION_REFRESH_DEBOUNCE_MILLISECONDS);
		this.#emit('searching', null);
	}

	#scanActive(): boolean {
		return this.#abortController !== undefined || this.#worker !== undefined;
	}

	#event(): ProjectedTerminalSearchEvent {
		return {
			activeMatch: this.#activeMatch,
			activeIndex: this.#activeIndex,
			errorKind: this.#errorKind,
			generation: this.#generation,
			matches: this.#matches,
			query: this.#query,
			regexError: this.#regexError,
			resultLimitReached: this.#resultLimitReached(),
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

	#activate(match: TerminalSearchMatch): void {
		this.#activeMatch = match;
		this.#activeIndex = findMatchIndex(this.#matches, match);
		this.#navigationAnchor = { ...match.start };
		this.#options.onActivate(
			match,
			{
				generation: this.#generation,
				revision: this.#revision,
			},
			true,
		);
		this.#emit('complete', null);
	}

	#drainNavigation(): void {
		while (
			this.#state === 'complete' &&
			this.#activeMatch !== null &&
			this.#pendingNavigationOffset !== 0
		) {
			const step = this.#pendingNavigationOffset < 0 ? -1 : 1;
			this.#pendingNavigationOffset -= step;
			const navigation = this.#navigationFromAnchor(step < 0, false);
			if (this.#activeIndex >= 0 && !this.#resultLimitReached()) {
				const match = findDirectionalMatch(this.#matches, navigation);
				if (match === null) {
					this.#pendingNavigationOffset = 0;
					return;
				}
				this.#activate(match);
				continue;
			}
			this.#inFlightNavigationStep = step;
			void this.#run(navigation, true);
			return;
		}
	}

	#initialNavigation(): TerminalDirectionalSearchOptions {
		return this.#navigationFromAnchor(true, true);
	}

	#navigationFromAnchor(
		reverse: boolean,
		preserveStart: boolean,
	): TerminalDirectionalSearchOptions {
		return this.#navigationAnchor === undefined
			? { preserveStart, reverse }
			: { preserveStart, reverse, start: { ...this.#navigationAnchor } };
	}

	#resultLimitReached(): boolean {
		return (
			this.#matches.length > 0 &&
			this.#matches.length >=
				Math.min(
					TERMINAL_SEARCH_RESULT_LIMIT,
					resolveTerminalSearchResultLimit(this.#searchOptions.maxResults),
				)
		);
	}

	#readSearchPage(
		snapshot: ReturnType<TerminalBufferRowReader['getBufferRowsSnapshot']>,
		startRow: bigint,
		rowCount: number,
		signal: AbortSignal,
	): ReturnType<TerminalBufferRowReader['readBufferRows']> {
		return this.#options.reader.readBufferRows(
			{
				detail: TerminalBufferRowDetail.text,
				generation: snapshot.generation,
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				purpose: 'search',
				revision: snapshot.revision,
				rowCount: Math.min(rowCount, Number(snapshot.totalRows - startRow)),
				startRow,
			},
			signal,
		);
	}

	#waitForWorkerProgress(searchId: number, nextRow: bigint): Promise<boolean> {
		return new Promise((resolve) => {
			this.#workerProgress = { nextRow, resolve, searchId };
		});
	}

	#receiveWorkerProgress(searchId: number, encodedNextRow: string): void {
		const waiter = this.#workerProgress;
		let nextRow: bigint;
		try {
			nextRow = BigInt(encodedNextRow);
		} catch {
			this.#completeError(searchId, 'Terminal search Worker returned invalid progress.');
			return;
		}
		if (nextRow < 0n || nextRow.toString() !== encodedNextRow) {
			this.#completeError(searchId, 'Terminal search Worker returned invalid progress.');
			return;
		}
		if (waiter === undefined || waiter.searchId !== searchId || waiter.nextRow !== nextRow) {
			this.#completeError(searchId, 'Terminal search Worker returned out-of-order progress.');
			return;
		}
		this.#workerProgress = undefined;
		this.#refreshWatchdog(searchId);
		waiter.resolve(true);
	}

	#refreshWatchdog(searchId: number): void {
		if (this.#searchId !== searchId) return;
		if (this.#watchdog !== undefined) clearTimeout(this.#watchdog);
		this.#watchdog = setTimeout(() => {
			if (this.#searchId !== searchId) return;
			this.#inFlightNavigationStep = 0;
			this.#pendingNavigationOffset = 0;
			this.#refreshPending = false;
			this.#retireScan(searchId);
			this.#emit('timeout', null);
		}, SEARCH_WATCHDOG_MILLISECONDS);
	}

	#retireScan(searchId: number): void {
		if (this.#searchId !== searchId) return;
		this.#abortController?.abort();
		this.#abortController = undefined;
		this.#releaseWorkerProgress(false);
		this.#cancelWorker();
		this.#searchId += 1;
	}

	#cancelScan(): void {
		if (this.#debounce !== undefined) clearTimeout(this.#debounce);
		this.#debounce = undefined;
		this.#abortController?.abort();
		this.#abortController = undefined;
		this.#releaseWorkerProgress(false);
		this.#cancelWorker();
		this.#refreshPending = false;
		this.#searchId += 1;
	}

	#cancelWorker(): void {
		if (this.#watchdog !== undefined) clearTimeout(this.#watchdog);
		this.#watchdog = undefined;
		this.#worker?.terminate();
		this.#worker = undefined;
	}

	#releaseWorkerProgress(progressed: boolean): void {
		const waiter = this.#workerProgress;
		this.#workerProgress = undefined;
		waiter?.resolve(progressed);
	}
}

function settleSearchPage(
	promise: Promise<SearchPageResult>,
	signal: AbortSignal,
): Promise<SettledSearchPage> {
	return new Promise((resolve) => {
		let finished = false;
		const finish = (result: SettledSearchPage): void => {
			if (finished) return;
			finished = true;
			signal.removeEventListener('abort', cancel);
			resolve(result);
		};
		const cancel = (): void => finish({ kind: 'cancelled' });
		if (signal.aborted) cancel();
		else signal.addEventListener('abort', cancel, { once: true });
		void promise.then(
			(value) => finish({ kind: 'completed', value }),
			(error: unknown) => finish({ error, kind: 'failed' }),
		);
	});
}

async function admitPrefetch(
	progress: Promise<boolean>,
	read: Promise<SettledSearchPage>,
): Promise<SettledSearchPage> {
	return (await progress) ? await read : { kind: 'cancelled' };
}

function findDirectionalMatch(
	matches: readonly TerminalSearchMatch[],
	navigation: TerminalDirectionalSearchOptions,
): TerminalSearchMatch | null {
	if (matches.length === 0) return null;
	const start = navigation.start;
	if (start === undefined)
		return navigation.reverse ? (matches.at(-1) ?? null) : (matches[0] ?? null);
	if (navigation.reverse) {
		for (let index = matches.length - 1; index >= 0; index--) {
			const match = matches[index];
			if (match === undefined) continue;
			const comparison = comparePosition(match.start, start);
			if (comparison < 0 || (navigation.preserveStart && comparison === 0)) return match;
		}
		return matches.at(-1) ?? null;
	}
	for (const match of matches) {
		const comparison = comparePosition(match.start, start);
		if (comparison > 0 || (navigation.preserveStart && comparison === 0)) return match;
	}
	return matches[0] ?? null;
}

function findMatchIndex(
	matches: readonly TerminalSearchMatch[],
	activeMatch: TerminalSearchMatch | null,
): number {
	if (activeMatch === null) return -1;
	return matches.findIndex(
		(match) =>
			comparePosition(match.start, activeMatch.start) === 0 &&
			comparePosition(match.end, activeMatch.end) === 0,
	);
}

function comparePosition(left: TerminalSearchPosition, right: TerminalSearchPosition): number {
	return left.row === right.row ? left.column - right.column : left.row - right.row;
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
