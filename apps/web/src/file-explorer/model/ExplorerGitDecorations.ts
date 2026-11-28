import {
	filePathKey,
	filePathName,
	isFilePathWithin,
	type FilePathRules,
	type FileRootEvent,
	type FileWorkspaceSession,
	type GitPort,
} from '../../file-session';
import {
	scmFileDecorations,
	scmWorkspaceOf,
	type ScmFileDecoration,
	type ScmRepositoryState,
} from '../../source-control';

/** What Git decorates on one row. */
export interface ExplorerGitDecoration {
	/** For a folder, the change below it that VS Code bubbles up first, whose color it takes. */
	readonly descendant: ScmFileDecoration | null;
	/** Git ignores the path, which grays its name. */
	readonly ignored: boolean;
	/** The row's own change. */
	readonly own: ScmFileDecoration | null;
}

const UNDECORATED: ExplorerGitDecoration = { descendant: null, ignored: false, own: null };

/** How long VS Code's Git extension gathers the paths it shows before it asks which are ignored. */
const IGNORE_CHECK_DELAY = 500;

/** The most paths one question to Git names, as the backend takes them. */
const IGNORE_CHECK_LIMIT = 1000;

/** The name of the files whose changes can change which paths Git ignores. */
const IGNORE_FILE = '.gitignore';

/**
 * A path key's place in VS Code's decoration tree, which orders paths segment by segment: the
 * separator turns into the lowest code unit, so comparing whole strings compares segments, and the
 * paths below a folder follow it in one run.
 */
function treeOrder(key: string, separator: string): string {
	return key.replaceAll(separator, '\0');
}

function compareTreeOrders(one: string, other: string): number {
	if (one === other) return 0;
	return one < other ? -1 : 1;
}

/** The index of the first of `orders` that does not sort before `order`. */
function firstAtOrAfter(orders: readonly string[], order: string): number {
	let low = 0;
	let high = orders.length;
	while (low < high) {
		const middle = Math.floor((low + high) / 2);
		const probe = orders[middle];
		if (probe !== undefined && probe < order) low = middle + 1;
		else high = middle;
	}
	return low;
}

/**
 * The Explorer's Git decorations over one session, as VS Code's Git extension gives them and its
 * decorations service bubbles them up: each changed path keeps its own decoration from Source
 * Control's status, which the Source Control view shares, and a folder takes the color of the first
 * change below it in VS Code's path order, apart from a deletion, which stays on its own path. Each
 * status builds the table once, and a folder finds its change by a binary search rather than a walk
 * over what lies below it.
 *
 * Which paths Git ignores, it asks only for the rows that show, as VS Code asks for the paths it
 * draws: it gathers them until none has come for 500 ms, then asks each path's innermost repository
 * about them together, and keeps the answers until a `.gitignore` changes, records are lost, or
 * the repositories change, when it asks again about the rows that show, which keep their color
 * until the new answer.
 */
export class ExplorerGitDecorations {
	readonly #listeners = new Set<() => void>();
	/** The changes that color their folders, in tree order, beside their decorations. */
	#bubbling: readonly string[] = [];
	#bubblingDecorations: readonly ScmFileDecoration[] = [];
	/** The questions about ignored paths that are on their way. */
	readonly #checks = new Set<AbortController>();
	/** Paths asked about in a question that is on its way. */
	readonly #checking = new Set<string>();
	/** Each changed path's decoration, as the status spells the path. */
	#decorations: ReadonlyMap<string, ScmFileDecoration> = new Map();
	#disposed = false;
	readonly #git: GitPort;
	/** The groups the table was last built from, so that it is built again only when they change. */
	#groups: readonly ScmRepositoryState['groups'][] = [];
	/** Git's answer for each path asked about, as the row spells it. */
	readonly #ignored = new Map<string, boolean>();
	/** Each changed path's decoration by its key. */
	#own: ReadonlyMap<string, ScmFileDecoration> = new Map();
	/** Shown paths waiting to be asked about. */
	readonly #pending = new Set<string>();
	/** The repositories' paths, innermost first, once the first summary names them. */
	#repositories: readonly string[] | null = null;
	#rules: FilePathRules | null = null;
	/** How many rows show each path. */
	readonly #shown = new Map<string, number>();
	#timer: ReturnType<typeof setTimeout> | null = null;
	readonly #unobserve: () => void;
	readonly #unsubscribeRoot: () => void;
	#version = 0;

	constructor(session: FileWorkspaceSession) {
		this.#git = session.git;
		const workspace = scmWorkspaceOf(session.git);
		const follow = (): void => {
			const { repositories, summary } = workspace.state;
			if (summary !== null) {
				this.#onRepositories(summary.repositories.map(({ repository }) => repository));
			}
			this.#onStatus(repositories);
		};
		this.#unobserve = workspace.observe(follow, { status: true });
		this.#unsubscribeRoot = session.subscribe((event) => this.#onRoot(event));
		follow();
	}

	/** Counts the changes of the decorations, which rows draw from. */
	get version(): number {
		return this.#version;
	}

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	/** What Git decorates on the row of `path`; only a folder takes a change from below. */
	decorationOf(path: string, directory: boolean): ExplorerGitDecoration {
		const rules = this.#rules;
		if (rules === null) return UNDECORATED;
		const key = filePathKey(path, rules);
		const own = this.#own.get(key) ?? null;
		const descendant = directory ? this.#descendantOf(key, rules) : null;
		const ignored = this.#ignored.get(path) === true;
		if (own === null && descendant === null && !ignored) return UNDECORATED;
		return { descendant, ignored, own };
	}

	/**
	 * Tells the decorations that a row of `path` shows, so Git is asked whether it ignores the path;
	 * the returned function says the row no longer shows it.
	 */
	show(path: string): () => void {
		this.#shown.set(path, (this.#shown.get(path) ?? 0) + 1);
		if (!this.#ignored.has(path)) this.#ask(path);
		let shown = true;
		return () => {
			if (!shown) return;
			shown = false;
			const count = (this.#shown.get(path) ?? 1) - 1;
			if (count > 0) this.#shown.set(path, count);
			else this.#shown.delete(path);
		};
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unobserve();
		this.#unsubscribeRoot();
		this.#abortChecks();
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = null;
		this.#listeners.clear();
	}

	#descendantOf(key: string, rules: FilePathRules): ScmFileDecoration | null {
		const order = treeOrder(key, rules.separator);
		// A root such as `C:\` already ends with its separator.
		const prefix = key.endsWith(rules.separator) ? order : `${order}\0`;
		const index = firstAtOrAfter(this.#bubbling, prefix);
		if (this.#bubbling[index]?.startsWith(prefix) !== true) return null;
		return this.#bubblingDecorations[index] ?? null;
	}

	#onStatus(repositories: readonly ScmRepositoryState[]): void {
		const groups = repositories.map((repository) => repository.groups);
		const unchanged =
			groups.length === this.#groups.length &&
			groups.every((group, index) => group === this.#groups[index]);
		if (unchanged) return;
		this.#groups = groups;
		this.#decorations = scmFileDecorations(repositories);
		this.#build();
	}

	/** A repository that opens or closes changes which repository answers for a path. */
	#onRepositories(paths: readonly string[]): void {
		const sorted = [...paths].sort((one, other) => other.length - one.length);
		const previous = this.#repositories;
		this.#repositories = sorted;
		if (previous === null) {
			this.#schedule();
			return;
		}
		const changed =
			previous.length !== sorted.length || sorted.some((path) => !previous.includes(path));
		if (changed) this.#askAgain();
	}

	#onRoot(event: FileRootEvent): void {
		if (event.kind === 'gap') {
			this.#askAgain();
			return;
		}
		if (event.kind === 'update') {
			const rules = this.#rules;
			const ignoreFileChanged =
				rules !== null &&
				event.changes.some(
					(change) => filePathKey(filePathName(change.path, rules), rules) === IGNORE_FILE,
				);
			// Changes dropped below a folder may have been to a `.gitignore`.
			if (ignoreFileChanged || event.rescan.length > 0) this.#askAgain();
			return;
		}
		const { rules } = event;
		const previous = this.#rules;
		if (previous?.separator === rules.separator && previous.caseSensitive === rules.caseSensitive) {
			return;
		}
		this.#rules = rules;
		this.#build();
		this.#schedule();
	}

	#build(): void {
		const rules = this.#rules;
		if (rules === null) return;
		const own = new Map<string, ScmFileDecoration>();
		for (const [path, decoration] of this.#decorations) {
			own.set(filePathKey(path, rules), decoration);
		}
		const bubbling: { readonly decoration: ScmFileDecoration; readonly order: string }[] = [];
		for (const [key, decoration] of own) {
			if (decoration.propagates) {
				bubbling.push({ decoration, order: treeOrder(key, rules.separator) });
			}
		}
		bubbling.sort((one, other) => compareTreeOrders(one.order, other.order));
		this.#own = own;
		this.#bubbling = bubbling.map(({ order }) => order);
		this.#bubblingDecorations = bubbling.map(({ decoration }) => decoration);
		this.#publish();
	}

	/** Queues a shown path for the next question; each new one puts the question off again. */
	#ask(path: string): void {
		if (this.#disposed || this.#pending.has(path) || this.#checking.has(path)) return;
		this.#pending.add(path);
		this.#schedule();
	}

	/** Forgets the answers, and asks again about the rows that show, which keep theirs till then. */
	#askAgain(): void {
		this.#abortChecks();
		for (const path of this.#ignored.keys()) {
			if (!this.#shown.has(path)) this.#ignored.delete(path);
		}
		for (const path of this.#shown.keys()) this.#ask(path);
	}

	#schedule(): void {
		if (this.#disposed || this.#pending.size === 0) return;
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = setTimeout(() => {
			this.#timer = null;
			this.#check();
		}, IGNORE_CHECK_DELAY);
	}

	/** Asks each repository about the shown paths it holds; a path in none is not ignored. */
	#check(): void {
		const rules = this.#rules;
		const repositories = this.#repositories;
		// The first snapshot and summary ask again for what is pending.
		if (rules === null || repositories === null) return;
		const asked = new Map<string, string[]>();
		let answered = false;
		for (const path of this.#pending) {
			if (!this.#shown.has(path)) continue;
			const repository = repositories.find((candidate) => isFilePathWithin(path, candidate, rules));
			// Git ignores nothing outside a repository, nor a repository's own folder.
			if (repository === undefined || filePathKey(path, rules) === filePathKey(repository, rules)) {
				answered = this.#answer(path, false) || answered;
				continue;
			}
			const paths = asked.get(repository) ?? [];
			paths.push(path);
			asked.set(repository, paths);
		}
		this.#pending.clear();
		for (const [repository, paths] of asked) {
			for (let start = 0; start < paths.length; start += IGNORE_CHECK_LIMIT) {
				this.#checkPaths(repository, paths.slice(start, start + IGNORE_CHECK_LIMIT));
			}
		}
		if (answered) this.#publish();
	}

	#checkPaths(repository: string, paths: readonly string[]): void {
		const controller = new AbortController();
		this.#checks.add(controller);
		for (const path of paths) this.#checking.add(path);
		void this.#git.checkIgnore({ paths, repository }, controller.signal).then(
			(result) => {
				if (controller.signal.aborted) return;
				this.#checks.delete(controller);
				const ignored = new Set(result.kind === 'ok' ? result.value : []);
				let changed = false;
				for (const path of paths) {
					this.#checking.delete(path);
					// A question that fails counts as an answer that ignores nothing.
					changed = this.#answer(path, ignored.has(path)) || changed;
				}
				if (changed) this.#publish();
			},
			() => {
				if (controller.signal.aborted) return;
				this.#checks.delete(controller);
				let changed = false;
				for (const path of paths) {
					this.#checking.delete(path);
					changed = this.#answer(path, false) || changed;
				}
				if (changed) this.#publish();
			},
		);
	}

	/** Keeps Git's answer for a path; true when it changes what the row draws. */
	#answer(path: string, ignored: boolean): boolean {
		const previous = this.#ignored.get(path) === true;
		this.#ignored.set(path, ignored);
		return previous !== ignored;
	}

	#abortChecks(): void {
		for (const controller of this.#checks) controller.abort();
		this.#checks.clear();
		this.#checking.clear();
	}

	#publish(): void {
		if (this.#disposed) return;
		this.#version += 1;
		for (const listener of this.#listeners) listener();
	}
}
