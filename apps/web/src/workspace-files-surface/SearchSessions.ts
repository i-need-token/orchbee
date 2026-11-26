import type { WorkspaceSearchQuery } from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource, WorkspaceSearch } from '../workbench';
import {
	appendSearchFiles,
	dismissSearchResult,
	SEARCHING,
	type SearchActive,
	type SearchOutcome,
	type SearchResultLine,
	type SearchResults,
	stepSearchMatch,
} from './search-results';

/** What the search view's boxes and toggles hold. */
export interface SearchDraft {
	readonly pattern: string;
	readonly regex: boolean;
	readonly matchCase: boolean;
	readonly wholeWord: boolean;
	/** Comma-separated globs, as typed. */
	readonly includes: string;
	readonly excludes: string;
	readonly useIgnoreFiles: boolean;
	readonly onlyOpen: boolean;
	readonly onlyChanged: boolean;
	/** Whether the include and exclude boxes show. */
	readonly details: boolean;
}

/** A Workspace's search, kept while the window lives so its results outlive the view. */
export interface SearchSession {
	readonly draft: SearchDraft;
	/** Texts searched for, oldest first, which ↑ and ↓ bring back. */
	readonly history: readonly string[];
	/** What the last search found, null before any or after clearing. */
	readonly results: SearchResults | null;
	/** The draft the last search or clearing was for, so the same draft is not searched again. */
	readonly searchedKey: string | null;
	readonly active: SearchActive | null;
	/** Files whose matches are folded. */
	readonly collapsed: ReadonlySet<string>;
}

/** What a search runs with, or why it cannot run. */
export type PreparedSearch =
	| { readonly kind: 'query'; readonly query: WorkspaceSearchQuery }
	| { readonly kind: 'failed'; readonly state: SearchOutcome };

const DEFAULT_DRAFT: SearchDraft = {
	details: false,
	excludes: '',
	includes: '',
	matchCase: false,
	onlyChanged: false,
	onlyOpen: false,
	pattern: '',
	regex: false,
	useIgnoreFiles: true,
	wholeWord: false,
};

const EMPTY_SESSION: SearchSession = {
	active: null,
	collapsed: new Set(),
	draft: DEFAULT_DRAFT,
	history: [],
	results: null,
	searchedKey: null,
};

/** Texts ↑ and ↓ walk through, newest kept. */
const HISTORY_LIMIT = 50;

/** The search a Workspace has running, which a newer one or clearing stops. */
interface Running {
	readonly generation: number;
	search: WorkspaceSearch | null;
}

/**
 * Every Workspace's search in one window: what was typed, what was found and where the keyboard
 * was. One search runs per Workspace; a new one stops the last. Nothing is stored.
 */
export class SearchSessions {
	readonly #source: WorkbenchWorkspaceFilesSource;
	readonly #sessions = new Map<string, SearchSession>();
	readonly #running = new Map<string, Running>();
	readonly #listeners = new Set<() => void>();
	#version = 0;
	#generation = 0;
	#disposed = false;

	constructor(source: WorkbenchWorkspaceFilesSource) {
		this.#source = source;
	}

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	readonly getVersion = (): number => this.#version;

	session(workspaceId: string): SearchSession {
		return this.#sessions.get(workspaceId) ?? EMPTY_SESSION;
	}

	edit(workspaceId: string, change: Partial<SearchDraft>): void {
		this.#update(workspaceId, (session) => ({
			...session,
			draft: { ...session.draft, ...change },
		}));
	}

	/**
	 * Starts a search for the draft named `key`, stopping the one running; the query may wait on a
	 * read, such as the changed files, and a newer search that starts meanwhile wins.
	 */
	start(workspaceId: string, key: string, prepared: Promise<PreparedSearch>): void {
		this.#stop(workspaceId);
		this.#generation += 1;
		const running: Running = { generation: this.#generation, search: null };
		this.#running.set(workspaceId, running);
		this.#update(workspaceId, (session) => {
			const pattern = session.draft.pattern;
			const history = session.history.filter((earlier) => earlier !== pattern);
			return {
				...session,
				active: null,
				history: [...history, pattern].slice(-HISTORY_LIMIT),
				results: SEARCHING,
				searchedKey: key,
			};
		});
		const current = (): boolean =>
			!this.#disposed && this.#running.get(workspaceId)?.generation === running.generation;
		void prepared.then((ready) => {
			if (!current()) return;
			if (ready.kind === 'failed') {
				this.#finish(workspaceId, running, ready.state);
				return;
			}
			running.search = this.#source.search(workspaceId, ready.query, (report) => {
				if (!current()) return;
				if (report.state !== 'searching') this.#running.delete(workspaceId);
				this.#update(workspaceId, (session) => ({
					...session,
					results: appendSearchFiles(session.results ?? SEARCHING, report.files, report.state),
				}));
			});
			// A search that ended before it returned is closed again, which does nothing.
			if (!current()) running.search.close();
		});
	}

	/** Stops the running search, keeping what it found. */
	cancel(workspaceId: string): void {
		const running = this.#running.get(workspaceId);
		if (running === undefined) return;
		this.#finish(workspaceId, running, 'cancelled');
	}

	/** Stops the search and empties the box and the results; `key` names the empty draft. */
	clear(workspaceId: string, key: string): void {
		this.#stop(workspaceId);
		this.#update(workspaceId, (session) => ({
			...session,
			active: null,
			collapsed: new Set(),
			draft: { ...session.draft, pattern: '' },
			results: null,
			searchedKey: key,
		}));
	}

	/** Forgets the results without touching the draft, as an emptied box does. */
	discard(workspaceId: string, key: string): void {
		this.#stop(workspaceId);
		this.#update(workspaceId, (session) => ({
			...session,
			active: null,
			results: null,
			searchedKey: key,
		}));
	}

	clearHistory(workspaceId: string): void {
		this.#update(workspaceId, (session) => ({ ...session, history: [] }));
	}

	setActive(workspaceId: string, active: SearchActive | null): void {
		this.#update(workspaceId, (session) => ({ ...session, active }));
	}

	setCollapsed(workspaceId: string, collapsed: ReadonlySet<string>): void {
		this.#update(workspaceId, (session) => ({ ...session, collapsed }));
	}

	/** Removes a file, or one of its lines, from the results; the files stay as they are. */
	dismiss(workspaceId: string, path: string, key: string | null): void {
		this.#update(workspaceId, (session) =>
			session.results === null
				? session
				: { ...session, results: dismissSearchResult(session.results, path, key) },
		);
	}

	/**
	 * Moves to the next or previous match, unfolding its file, and names it so it can be opened;
	 * null when there is none.
	 */
	step(
		workspaceId: string,
		direction: 1 | -1,
	): (SearchActive & { readonly line: SearchResultLine }) | null {
		const session = this.session(workspaceId);
		const next = stepSearchMatch(session.results, session.active, direction);
		if (next === null) return null;
		const collapsed = new Set(session.collapsed);
		collapsed.delete(next.path);
		this.#update(workspaceId, (current) => ({
			...current,
			active: { key: next.key, match: next.match, path: next.path },
			collapsed: collapsed.size === current.collapsed.size ? current.collapsed : collapsed,
		}));
		return next;
	}

	dispose(): void {
		this.#disposed = true;
		for (const running of this.#running.values()) running.search?.close();
		this.#running.clear();
		this.#listeners.clear();
	}

	#finish(workspaceId: string, running: Running, state: SearchOutcome): void {
		running.search?.close();
		this.#running.delete(workspaceId);
		this.#update(workspaceId, (session) => ({
			...session,
			results: { ...(session.results ?? SEARCHING), state },
		}));
	}

	#stop(workspaceId: string): void {
		this.#running.get(workspaceId)?.search?.close();
		this.#running.delete(workspaceId);
	}

	#update(workspaceId: string, change: (session: SearchSession) => SearchSession): void {
		if (this.#disposed) return;
		const current = this.session(workspaceId);
		const next = change(current);
		if (next === current) return;
		this.#sessions.set(workspaceId, next);
		this.#version += 1;
		for (const listener of this.#listeners) listener();
	}
}
