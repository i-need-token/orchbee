import {
	MAXIMUM_WORKSPACE_SEARCH_PATHS,
	type WorkspaceSearchQuery,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource, WorkspaceSearch } from '../workbench';
import {
	appendSearchFiles,
	dismissSearchResult,
	type ReplaceTarget,
	replacedLines,
	replaceFileOf,
	researchedFiles,
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
	/** Whether the replace box shows. */
	readonly replacing: boolean;
	/** What each match becomes. */
	readonly replacement: string;
	readonly preserveCase: boolean;
}

/** Where a replacement stands, or what it did. */
export type ReplaceNote =
	| { readonly kind: 'replacing' }
	| {
			readonly kind: 'replaced';
			/** Matches replaced, and the files they were in. */
			readonly matches: number;
			readonly files: number;
			/** Files left out because they are open with edits not saved. */
			readonly unsaved: number;
			/** Files that changed since the search, which are searched again. */
			readonly changed: number;
			/** Binary files, files not in UTF-8 and files too large. */
			readonly notEditable: number;
			/** Files not written, or not heard back about. */
			readonly failed: number;
	  };

/** A Workspace's search, kept while the window lives so its results outlive the view. */
export interface SearchSession {
	readonly draft: SearchDraft;
	/** Texts searched for, oldest first, which ↑ and ↓ bring back. */
	readonly history: readonly string[];
	/** What the last search found, null before any or after clearing. */
	readonly results: SearchResults | null;
	/** The draft the last search or clearing was for, so the same draft is not searched again. */
	readonly searchedKey: string | null;
	/** What the results were searched for, which a replacement looks for again. */
	readonly searched: WorkspaceSearchQuery | null;
	readonly replace: ReplaceNote | null;
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
	preserveCase: false,
	regex: false,
	replacement: '',
	replacing: false,
	useIgnoreFiles: true,
	wholeWord: false,
};

const EMPTY_SESSION: SearchSession = {
	active: null,
	collapsed: new Set(),
	draft: DEFAULT_DRAFT,
	history: [],
	replace: null,
	results: null,
	searched: null,
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
				replace: null,
				results: SEARCHING,
				searched: null,
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
			this.#update(workspaceId, (session) => ({ ...session, searched: ready.query }));
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
			replace: null,
			results: null,
			searched: null,
			searchedKey: key,
		}));
	}

	/** Forgets the results without touching the draft, as an emptied box does. */
	discard(workspaceId: string, key: string): void {
		this.#stop(workspaceId);
		this.#update(workspaceId, (session) => ({
			...session,
			active: null,
			replace: null,
			results: null,
			searched: null,
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
	 * Replaces what `target` names, or every match shown, with the draft's replacement, leaving out
	 * files in `unsaved`. Replaced lines leave the results; a file that changed since the search, or
	 * keeps other lines whose places a replacement may have moved, is searched again. Answers
	 * whether a file may have been written. Nothing runs while the results are still coming, or
	 * while another replacement of the Workspace runs.
	 */
	async replace(
		workspaceId: string,
		target: ReplaceTarget | null,
		unsaved: ReadonlySet<string>,
	): Promise<boolean> {
		const session = this.session(workspaceId);
		const { results, searched, draft } = session;
		if (
			results === null ||
			results.state === 'searching' ||
			searched === null ||
			session.replace?.kind === 'replacing'
		) {
			return false;
		}
		const named = replacedLines(results, target);
		const kept = named.filter((file) => !unsaved.has(file.path));
		if (named.length === 0) return false;
		const generation = this.#generation;
		this.#update(workspaceId, (current) => ({ ...current, replace: { kind: 'replacing' } }));
		const files = kept.map(replaceFileOf);
		const reports =
			files.length === 0
				? []
				: await this.#source.replace(
						workspaceId,
						{
							matchCase: searched.matchCase,
							pattern: searched.pattern,
							preserveCase: draft.preserveCase,
							regex: searched.regex,
							replacement: draft.replacement,
							wholeWord: searched.wholeWord,
						},
						files,
					);
		const note = {
			changed: 0,
			failed: files.length - reports.length,
			files: 0,
			kind: 'replaced',
			matches: 0,
			notEditable: 0,
			unsaved: named.length - kept.length,
		} satisfies ReplaceNote;
		const again = new Set<string>();
		const replaced = new Map<string, readonly string[]>();
		let written = false;
		reports.forEach((report, index) => {
			const kind = report.result?.kind ?? null;
			if (kind === 'replaced') {
				note.files += 1;
				note.matches += files[index]?.matches.length ?? 0;
				replaced.set(report.path, kept[index]?.lines.map((line) => line.key) ?? []);
				written = true;
			} else if (kind === 'changed') {
				note.changed += 1;
				again.add(report.path);
			} else if (kind === 'not-editable') {
				note.notEditable += 1;
			} else {
				note.failed += 1;
				// No answer leaves unknown whether the file was written.
				if (kind === null) written = true;
			}
		});
		if (this.#disposed) return written;
		// A search started meanwhile shows the files as they are now.
		const current = this.#generation === generation;
		this.#update(workspaceId, (latest) => {
			let next = latest.results;
			if (current && next !== null) {
				for (const [path, keys] of replaced) {
					for (const key of keys) next = dismissSearchResult(next, path, key);
					if (next.files.some((file) => file.path === path)) again.add(path);
				}
			}
			return { ...latest, replace: note, results: next };
		});
		if (current && again.size > 0) this.#research(workspaceId, searched, [...again]);
		return written;
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

	/**
	 * Searches `paths` again with the query the results were for and shows what it finds in their
	 * place. A new search stops it, and one that does not complete changes nothing.
	 */
	#research(workspaceId: string, query: WorkspaceSearchQuery, paths: readonly string[]): void {
		this.#stop(workspaceId);
		this.#generation += 1;
		const running: Running = { generation: this.#generation, search: null };
		this.#running.set(workspaceId, running);
		const searched = paths.slice(0, MAXIMUM_WORKSPACE_SEARCH_PATHS);
		let found = SEARCHING;
		running.search = this.#source.search(
			workspaceId,
			{ ...query, excludes: [], includes: [], paths: searched },
			(report) => {
				if (this.#disposed || this.#running.get(workspaceId)?.generation !== running.generation) {
					return;
				}
				found = appendSearchFiles(found, report.files, report.state);
				if (report.state === 'searching') return;
				this.#running.delete(workspaceId);
				if (report.state !== 'complete') return;
				const replaced = new Set(searched);
				this.#update(workspaceId, (session) =>
					session.results === null
						? session
						: { ...session, results: researchedFiles(session.results, replaced, found.files) },
				);
			},
		);
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
