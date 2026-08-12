import type { ProjectedTerminalSearchEvent, TerminalSearchOptions } from '@orchbee/terminal';
import type { HistoryChanged, SelectionChanged } from '@orchbee/terminal-protocol';

const MAX_QUERY_HISTORY = 20;

export type TerminalFindNotice =
	| { readonly kind: 'history-evicted' }
	| { readonly kind: 'history-unavailable' }
	| { readonly kind: 'link-blocked' }
	| { readonly kind: 'link-failed' }
	| { readonly kind: 'selection-evicted' };

export interface TerminalFindSnapshot {
	readonly activeIndex: number;
	readonly caseSensitive: boolean;
	readonly focusRequest: number;
	readonly isOpen: boolean;
	readonly notice: TerminalFindNotice | null;
	readonly query: string;
	readonly regex: boolean;
	readonly resultCount: number;
	readonly resultLimitReached: boolean;
	readonly retainedRows: bigint;
	readonly searchErrorKind: ProjectedTerminalSearchEvent['errorKind'];
	readonly searchState: ProjectedTerminalSearchEvent['state'];
	readonly wholeWord: boolean;
}

export interface TerminalSearchDriver {
	clearSearch(): void;
	updateSearch(query: string, options: TerminalSearchOptions): void;
}

/** Own the framework-independent state transitions for one terminal find surface. */
export class TerminalFindController {
	readonly #driver: TerminalSearchDriver;
	readonly #listeners = new Set<() => void>();
	readonly #queryHistory: string[] = [];
	#disposed = false;
	#historyCursor: number | null = null;
	#historyDraft = '';
	#snapshot: TerminalFindSnapshot;

	constructor(driver: TerminalSearchDriver, retainedRows = 0n) {
		this.#driver = driver;
		this.#snapshot = {
			activeIndex: -1,
			caseSensitive: false,
			focusRequest: 0,
			isOpen: false,
			notice: null,
			query: '',
			regex: false,
			resultCount: 0,
			resultLimitReached: false,
			retainedRows,
			searchErrorKind: null,
			searchState: 'idle',
			wholeWord: false,
		};
	}

	readonly getSnapshot = (): TerminalFindSnapshot => this.#snapshot;

	readonly subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	open(): void {
		if (this.#disposed) return;
		const wasOpen = this.#snapshot.isOpen;
		this.#update({
			...this.#snapshot,
			focusRequest: this.#snapshot.focusRequest + 1,
			isOpen: true,
		});
		if (!wasOpen && this.#snapshot.query) this.#search();
	}

	close(): void {
		if (this.#disposed || !this.#snapshot.isOpen) return;
		this.recordCurrentQuery();
		this.#driver.clearSearch();
		this.#historyCursor = null;
		this.#update({
			...this.#snapshot,
			activeIndex: -1,
			isOpen: false,
			notice: null,
			resultCount: 0,
			resultLimitReached: false,
			searchErrorKind: null,
			searchState: 'idle',
		});
	}

	setQuery(query: string): void {
		if (this.#disposed || query === this.#snapshot.query) return;
		this.#historyCursor = null;
		this.#historyDraft = query;
		this.#applyQuery(query);
	}

	setCaseSensitive(caseSensitive: boolean): void {
		if (this.#disposed || caseSensitive === this.#snapshot.caseSensitive) return;
		this.#update({ ...this.#snapshot, caseSensitive, notice: null });
		this.#search();
	}

	setWholeWord(wholeWord: boolean): void {
		if (this.#disposed || wholeWord === this.#snapshot.wholeWord) return;
		this.#update({ ...this.#snapshot, notice: null, wholeWord });
		this.#search();
	}

	setRegex(regex: boolean): void {
		if (this.#disposed || regex === this.#snapshot.regex) return;
		this.#update({ ...this.#snapshot, notice: null, regex });
		this.#search();
	}

	moveInQueryHistory(direction: -1 | 1): boolean {
		if (this.#disposed || this.#queryHistory.length === 0) return false;
		if (this.#historyCursor === null) {
			this.#historyDraft = this.#snapshot.query;
			this.#historyCursor = this.#queryHistory.length;
		}
		const next = Math.max(0, Math.min(this.#queryHistory.length, this.#historyCursor + direction));
		if (next === this.#historyCursor) return false;
		this.#historyCursor = next;
		this.#applyQuery(
			next === this.#queryHistory.length ? this.#historyDraft : (this.#queryHistory[next] ?? ''),
		);
		return true;
	}

	recordCurrentQuery(): void {
		const query = this.#snapshot.query.trim();
		if (!query) return;
		const previous = this.#queryHistory.indexOf(query);
		if (previous >= 0) this.#queryHistory.splice(previous, 1);
		this.#queryHistory.push(query);
		if (this.#queryHistory.length > MAX_QUERY_HISTORY) this.#queryHistory.shift();
	}

	applySearchEvent(event: ProjectedTerminalSearchEvent): void {
		if (this.#disposed || event.query !== this.#snapshot.query) return;
		this.#update({
			...this.#snapshot,
			activeIndex: event.activeIndex,
			notice: null,
			resultCount: event.matches.length,
			resultLimitReached: event.resultLimitReached,
			searchErrorKind: event.errorKind,
			searchState: event.state,
		});
	}

	applyHistoryEvent(event: HistoryChanged): void {
		if (this.#disposed) return;
		this.#update({
			...this.#snapshot,
			notice:
				event.anchorStatus === 'evicted' ? { kind: 'history-evicted' } : this.#snapshot.notice,
			retainedRows: BigInt(event.totalRows),
		});
	}

	applySelectionEvent(event: SelectionChanged): void {
		if (this.#disposed || event.state !== 'evicted') return;
		this.#update({ ...this.#snapshot, notice: { kind: 'selection-evicted' } });
	}

	setNotice(notice: TerminalFindNotice): void {
		if (this.#disposed) return;
		this.#update({ ...this.#snapshot, notice });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#driver.clearSearch();
		this.#listeners.clear();
		this.#queryHistory.length = 0;
	}

	#applyQuery(query: string): void {
		this.#update({
			...this.#snapshot,
			activeIndex: -1,
			notice: null,
			query,
			resultCount: 0,
			resultLimitReached: false,
			searchErrorKind: null,
			searchState: query ? 'searching' : 'idle',
		});
		this.#search();
	}

	#search(): void {
		const snapshot = this.#snapshot;
		this.#driver.updateSearch(snapshot.query, {
			caseSensitive: snapshot.caseSensitive,
			regex: snapshot.regex,
			wholeWord: snapshot.wholeWord,
		});
	}

	#update(snapshot: TerminalFindSnapshot): void {
		if (Object.is(snapshot, this.#snapshot)) return;
		this.#snapshot = snapshot;
		for (const listener of this.#listeners) listener();
	}
}
