import type {
	FileError,
	GitPort,
	GitRepositorySummary,
	GitStatusEntry,
	GitSummary,
} from '../../file-session';
import { scmResourceGroups, type ScmResourceGroups } from './scm-resource';

/** One repository with the groups of its latest read status. */
export interface ScmRepositoryState {
	/** The groups at `revision`, kept while a newer status loads; null before the first read. */
	readonly groups: ScmResourceGroups | null;
	/** Status pages are being read for this repository now. */
	readonly loading: boolean;
	readonly summary: GitRepositorySummary;
}

/** What Source Control knows of one Workspace. */
export interface ScmWorkspaceState {
	readonly repositories: readonly ScmRepositoryState[];
	/** The latest summary; null before the first. A gap keeps the last one until the next. */
	readonly summary: GitSummary | null;
}

interface StatusLoad {
	abort: AbortController | null;
	groups: ScmResourceGroups | null;
	/** The revision `groups` hold, or null when they must be read again whatever the summary says. */
	loaded: number | null;
	/** The revision being read or waiting for a turn. */
	pending: number | null;
}

interface Observer {
	readonly listener: () => void;
	readonly status: boolean;
}

/** How many repositories read status pages at once, as VS Code opens at most five at a time. */
const MAXIMUM_CONCURRENT_LOADS = 5;

const EMPTY_STATE: ScmWorkspaceState = { repositories: [], summary: null };

/**
 * Source Control for one Workspace's session. It follows the summary while anyone observes it,
 * and reads each displayed repository's status pages at the summary's revision while an observer
 * wants rows, so a Workspace whose Changes nobody shows reads only its summary. A newer revision
 * replaces a read in progress, a stale one waits for the summary that follows, and the previous
 * groups stay until the new ones are whole.
 */
export class ScmWorkspace {
	readonly #git: GitPort;
	readonly #loads = new Map<string, StatusLoad>();
	readonly #observers = new Set<Observer>();
	#queue: string[] = [];
	#running = 0;
	#state: ScmWorkspaceState = EMPTY_STATE;
	#unsubscribe: (() => void) | null = null;

	constructor(git: GitPort) {
		this.#git = git;
	}

	get state(): ScmWorkspaceState {
		return this.#state;
	}

	/**
	 * Calls `listener` after every change while observed. With `status`, the repositories' rows are
	 * read as well; otherwise only the summary is followed.
	 */
	observe(listener: () => void, options: { readonly status: boolean }): () => void {
		const observer: Observer = { listener, status: options.status };
		this.#observers.add(observer);
		this.#unsubscribe ??= this.#git.subscribe((event) => {
			if (event.kind === 'gap') {
				// A restarted service counts revisions afresh: reads in progress are void, and every
				// repository is read again at the next summary, its rows shown until then.
				this.#stopLoads();
				for (const load of this.#loads.values()) load.loaded = null;
				return;
			}
			this.#summarize(event.summary);
		});
		this.#schedule();
		return () => {
			if (!this.#observers.delete(observer)) return;
			if (this.#observers.size === 0) {
				this.#unsubscribe?.();
				this.#unsubscribe = null;
			}
			if (!this.#wantsStatus()) this.#stopLoads();
		};
	}

	#wantsStatus(): boolean {
		for (const observer of this.#observers) if (observer.status) return true;
		return false;
	}

	#summarize(summary: GitSummary): void {
		const displayed = new Set(summary.repositories.map(({ repository }) => repository));
		for (const [repository, load] of this.#loads) {
			if (displayed.has(repository)) continue;
			if (load.abort !== null) {
				load.abort.abort();
				this.#running -= 1;
			}
			this.#loads.delete(repository);
		}
		this.#queue = this.#queue.filter((repository) => displayed.has(repository));
		this.#publish(summary);
		this.#schedule();
	}

	/** Queues a read for each displayed repository whose status is newer than its rows. */
	#schedule(): void {
		const summary = this.#state.summary;
		if (summary === null || !this.#wantsStatus()) return;
		for (const repository of summary.repositories) {
			// A repository whose status was never read has no pages yet.
			if (!repository.warm) continue;
			let load = this.#loads.get(repository.repository);
			if (load === undefined) {
				load = { abort: null, groups: null, loaded: null, pending: null };
				this.#loads.set(repository.repository, load);
			}
			if (load.loaded === repository.revision || load.pending === repository.revision) continue;
			load.abort?.abort();
			if (load.abort !== null) this.#running -= 1;
			load.abort = null;
			load.pending = repository.revision;
			if (!this.#queue.includes(repository.repository)) this.#queue.push(repository.repository);
		}
		this.#drain();
	}

	#drain(): void {
		while (this.#running < MAXIMUM_CONCURRENT_LOADS) {
			const repository = this.#queue.shift();
			if (repository === undefined) break;
			const load = this.#loads.get(repository);
			if (load === undefined || load.pending === null) continue;
			const abort = new AbortController();
			load.abort = abort;
			this.#running += 1;
			void this.#read(repository, load.pending, abort.signal).then((result) => {
				if (abort.signal.aborted || load.abort !== abort) return;
				load.abort = null;
				this.#running -= 1;
				const revision = load.pending;
				load.pending = null;
				if (result.kind === 'ok') {
					load.groups = scmResourceGroups(result.entries);
					load.loaded = revision;
				}
				// A stale revision or a failure waits for the next summary to read again.
				this.#publish(this.#state.summary);
				this.#drain();
			});
		}
		this.#publish(this.#state.summary);
	}

	async #read(
		repository: string,
		revision: number,
		signal: AbortSignal,
	): Promise<
		| { readonly entries: readonly GitStatusEntry[]; readonly kind: 'ok' }
		| { readonly error: FileError; readonly kind: 'error' }
	> {
		const entries: GitStatusEntry[] = [];
		let cursor: string | undefined;
		for (;;) {
			const page = await this.#git.statusPage({ cursor, repository, revision }, signal);
			if (page.kind === 'error') return page;
			entries.push(...page.value.entries);
			if (page.value.cursor === null) return { entries, kind: 'ok' };
			cursor = page.value.cursor;
		}
	}

	#stopLoads(): void {
		for (const load of this.#loads.values()) {
			load.abort?.abort();
			load.abort = null;
			load.pending = null;
		}
		this.#queue = [];
		this.#running = 0;
		this.#publish(this.#state.summary);
	}

	#publish(summary: GitSummary | null): void {
		const repositories =
			summary?.repositories.map((repository) => {
				const load = this.#loads.get(repository.repository);
				return {
					groups: load?.groups ?? null,
					loading: load !== undefined && load.pending !== null,
					summary: repository,
				};
			}) ?? [];
		const previous = this.#state;
		const unchanged =
			previous.summary === summary &&
			previous.repositories.length === repositories.length &&
			previous.repositories.every(
				(state, index) =>
					state.groups === repositories[index]?.groups &&
					state.loading === repositories[index]?.loading &&
					state.summary === repositories[index]?.summary,
			);
		if (unchanged) return;
		this.#state = { repositories, summary };
		for (const observer of this.#observers) observer.listener();
	}
}
