import {
	filePathKey,
	type FilePathRules,
	type FileRootEvent,
	type FileWorkspaceSession,
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
	/** The row's own change. */
	readonly own: ScmFileDecoration | null;
}

const UNDECORATED: ExplorerGitDecoration = { descendant: null, own: null };

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
 */
export class ExplorerGitDecorations {
	readonly #listeners = new Set<() => void>();
	/** The changes that color their folders, in tree order, beside their decorations. */
	#bubbling: readonly string[] = [];
	#bubblingDecorations: readonly ScmFileDecoration[] = [];
	/** Each changed path's decoration, as the status spells the path. */
	#decorations: ReadonlyMap<string, ScmFileDecoration> = new Map();
	#disposed = false;
	/** The groups the table was last built from, so that it is built again only when they change. */
	#groups: readonly ScmRepositoryState['groups'][] = [];
	/** Each changed path's decoration by its key. */
	#own: ReadonlyMap<string, ScmFileDecoration> = new Map();
	#rules: FilePathRules | null = null;
	readonly #unobserve: () => void;
	readonly #unsubscribeRoot: () => void;
	#version = 0;

	constructor(session: FileWorkspaceSession) {
		const workspace = scmWorkspaceOf(session.git);
		this.#unobserve = workspace.observe(() => this.#onStatus(workspace.state.repositories), {
			status: true,
		});
		this.#unsubscribeRoot = session.subscribe((event) => this.#onRoot(event));
		this.#onStatus(workspace.state.repositories);
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
		return own === null && descendant === null ? UNDECORATED : { descendant, own };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unobserve();
		this.#unsubscribeRoot();
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

	#onRoot(event: FileRootEvent): void {
		if (event.kind !== 'snapshot') return;
		const { rules } = event;
		const previous = this.#rules;
		if (previous?.separator === rules.separator && previous.caseSensitive === rules.caseSensitive) {
			return;
		}
		this.#rules = rules;
		this.#build();
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

	#publish(): void {
		if (this.#disposed) return;
		this.#version += 1;
		for (const listener of this.#listeners) listener();
	}
}
