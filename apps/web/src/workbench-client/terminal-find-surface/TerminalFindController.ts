import type { ProjectedTerminalSearchEvent, TerminalSearchOptions } from '@orchbee/terminal';
import type { HistoryChanged, SelectionChanged } from '@orchbee/terminal-protocol';

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
	#disposed = false;
	#searchRevision = 0;
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

	open(selectionPreview?: string): void {
		if (this.#disposed) return;
		const wasOpen = this.#snapshot.isOpen;
		const query = selectionPreview || this.#snapshot.query;
		const queryChanged = query !== this.#snapshot.query;
		const shouldSearch = queryChanged || (!wasOpen && query.length > 0);
		// Refocusing the same query must preserve search work admitted by an enclosing transition.
		const revision = shouldSearch ? ++this.#searchRevision : this.#searchRevision;
		const snapshot = queryChanged ? this.#snapshotWithQuery(query) : this.#snapshot;
		this.#update({
			...snapshot,
			focusRequest: snapshot.focusRequest + 1,
			isOpen: true,
		});
		if (shouldSearch) this.#search(revision);
	}

	close(): void {
		if (this.#disposed || !this.#snapshot.isOpen) return;
		const revision = ++this.#searchRevision;
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
		if (this.#isCurrentSearch(revision)) this.#driver.clearSearch();
	}

	setQuery(query: string): void {
		if (this.#disposed || query === this.#snapshot.query) return;
		const revision = ++this.#searchRevision;
		this.#update(this.#snapshotWithQuery(query));
		this.#search(revision);
	}

	setCaseSensitive(caseSensitive: boolean): void {
		if (this.#disposed || caseSensitive === this.#snapshot.caseSensitive) return;
		const revision = ++this.#searchRevision;
		this.#update({ ...this.#snapshot, caseSensitive, notice: null });
		this.#search(revision);
	}

	setWholeWord(wholeWord: boolean): void {
		if (this.#disposed || wholeWord === this.#snapshot.wholeWord) return;
		const revision = ++this.#searchRevision;
		this.#update({ ...this.#snapshot, notice: null, wholeWord });
		this.#search(revision);
	}

	setRegex(regex: boolean): void {
		if (this.#disposed || regex === this.#snapshot.regex) return;
		const revision = ++this.#searchRevision;
		this.#update({ ...this.#snapshot, notice: null, regex });
		this.#search(revision);
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
	}

	#snapshotWithQuery(query: string): TerminalFindSnapshot {
		return {
			...this.#snapshot,
			activeIndex: -1,
			notice: null,
			query,
			resultCount: 0,
			resultLimitReached: false,
			searchErrorKind: null,
			searchState: query ? 'searching' : 'idle',
		};
	}

	#isCurrentSearch(revision: number): boolean {
		return !this.#disposed && revision === this.#searchRevision;
	}

	#search(revision: number): void {
		if (!this.#isCurrentSearch(revision)) return;
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
