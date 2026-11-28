import type {
	FilePathRules,
	FileRoot,
	FileRootEvent,
	FileWorkspaceSession,
	GitRemote,
	GitRepositorySummary,
} from '../../file-session';
import { VirtualTreeModel } from '../../virtual-tree';
import type { ScmGroupId, ScmResourceGroups, ScmResourceStatus } from './scm-resource';
import {
	SCM_TREE_INPUT,
	scmTreeSource,
	type ScmTreeItem,
	type ScmTreeSnapshot,
	type ScmTreeSource,
} from './scm-tree';
import type { ScmViewMode, ScmViewSortKey, ScmViewState } from './scm-view-state';
import type { ScmRepositoryState, ScmWorkspace, ScmWorkspaceState } from './ScmWorkspace';
import { scmWorkspaceOf } from './scmWorkspaceOf';
import {
	readSourceControlViewState,
	writeSourceControlViewState,
	type SourceControlViewStateStorage,
} from './source-control-view-state-storage';

type ChangesViewState = ReturnType<VirtualTreeModel<string, ScmTreeItem>['viewState']>;

/** What one Workspace's Source Control keeps while the view is away, for the run. */
export interface ScmViewMemory {
	/** Where the Changes tree was: expansion, focus, selection and scroll. */
	changes: ChangesViewState | null;
	/** Repositories this window closed in the Workspace, which discovery no longer displays. */
	readonly closed: Set<string>;
}

/** Everything the Source Control view draws from, apart from its two trees. */
export interface SourceControlState {
	/** How many repositories this window closed in the Workspace. */
	readonly closed: number;
	/** Each displayed repository's remotes, once read; a repository not read yet has none. */
	readonly remotes: ReadonlyMap<string, readonly GitRemote[]>;
	/** The repositories the view shows: those discovery found, apart from the ones closed here. */
	readonly repositories: readonly ScmRepositoryState[];
	/** The Workspace's root; null before the root stream describes it. */
	readonly root: FileRoot | null;
	readonly rules: FilePathRules | null;
	readonly view: ScmViewState;
	readonly workspace: ScmWorkspaceState;
}

interface SourceControlModelOptions {
	/** VS Code's `compareFileNames` for the locale. */
	readonly compareNames: (one: string, other: string) => number;
	readonly memory: ScmViewMemory;
	readonly session: FileWorkspaceSession;
	/** A status's localized hover text, which Sort by Status compares. */
	readonly statusText: (status: ScmResourceStatus) => string;
	readonly storage: SourceControlViewStateStorage | undefined;
}

/** The repositories list's input; repository paths are never empty. */
const REPOSITORIES_INPUT = '';

/** What a repository's remotes were read for: a new branch or upstream reads them again. */
function remotesKey({ head, upstream }: GitRepositorySummary): string {
	return `${head.branch ?? ''}\n${upstream?.name ?? ''}`;
}

/**
 * Source Control for one Workspace's session while its view shows: the root and its path rules,
 * the summary with each displayed repository's rows, each repository's remotes for its sync
 * control, the view mode and sort the Workspace remembers, and the Changes and Repositories trees,
 * which refresh in the task their data changes.
 */
export class SourceControlModel {
	readonly changes: VirtualTreeModel<string, ScmTreeItem>;
	/** VS Code's `compareFileNames` for the locale, which the view's pickers sort by as well. */
	readonly compareNames: SourceControlModelOptions['compareNames'];
	readonly repositories: VirtualTreeModel<string, string>;
	/** A status's localized hover text, the one Sort by Status compares. */
	readonly statusText: SourceControlModelOptions['statusText'];
	readonly #listeners = new Set<() => void>();
	readonly #memory: ScmViewMemory;
	readonly #remoteReads = new Map<
		string,
		{ readonly abort: AbortController; readonly key: string }
	>();
	readonly #session: FileWorkspaceSession;
	readonly #source: ScmTreeSource;
	readonly #storage: SourceControlViewStateStorage | undefined;
	readonly #workspace: ScmWorkspace;
	#disposed = false;
	/** The groups the Changes tree last refreshed with, to refresh only when they change. */
	#shownGroups: readonly (ScmResourceGroups | null)[] = [];
	#shownRepositories: readonly string[] = [];
	#state: SourceControlState;
	readonly #unobserve: () => void;
	readonly #unsubscribeRoot: () => void;

	constructor(options: SourceControlModelOptions) {
		this.compareNames = options.compareNames;
		this.#memory = options.memory;
		this.#session = options.session;
		this.statusText = options.statusText;
		this.#storage = options.storage;
		this.#workspace = scmWorkspaceOf(options.session.git);
		this.#state = {
			closed: this.#memory.closed.size,
			remotes: new Map(),
			repositories: [],
			root: null,
			rules: null,
			view: readSourceControlViewState(options.storage, options.session.workspaceId),
			workspace: this.#workspace.state,
		};
		this.#source = scmTreeSource(() => this.#snapshot());
		this.changes = new VirtualTreeModel({
			expandedByDefault: () => true,
			input: SCM_TREE_INPUT,
			source: this.#source,
		});
		this.repositories = new VirtualTreeModel({
			input: REPOSITORIES_INPUT,
			source: {
				children: (item) =>
					item === REPOSITORIES_INPUT
						? this.#state.repositories.map(({ summary }) => summary.repository)
						: [],
				hasChildren: (item) => item === REPOSITORIES_INPUT,
				key: (item) => item,
			},
		});
		const remembered = this.#memory.changes;
		if (remembered !== null) void this.changes.restoreViewState(remembered);
		this.#unobserve = this.#workspace.observe(() => this.#onWorkspace(), { status: true });
		this.#unsubscribeRoot = options.session.subscribe((event) => this.#onRoot(event));
	}

	get state(): SourceControlState {
		return this.#state;
	}

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	/** The repositories closed here, which Reopen Closed Repositories offers, in closing order. */
	get closedRepositories(): readonly string[] {
		return [...this.#memory.closed];
	}

	/**
	 * Close Repository: the view stops showing these repositories for the rest of the run, as VS
	 * Code's closed repositories stay closed, until Reopen Closed Repositories.
	 */
	close(repositories: readonly string[]): void {
		for (const repository of repositories) this.#memory.closed.add(repository);
		this.#onWorkspace();
	}

	/** Reopen Closed Repositories: the view shows these again once discovery finds them. */
	reopen(repositories: readonly string[]): void {
		for (const repository of repositories) this.#memory.closed.delete(repository);
		this.#onWorkspace();
	}

	/** VS Code's Collapse All on a group: each row below it collapses with everything under it. */
	collapseGroup(repository: string, group: ScmGroupId): void {
		const groups = this.#state.repositories.find(
			({ summary }) => summary.repository === repository,
		)?.groups;
		if (groups === null || groups === undefined) return;
		const item: ScmTreeItem = { group, kind: 'group', repository, resources: groups[group] };
		for (const child of this.#source.children(item)) {
			this.changes.collapse(this.#source.key(child), { recursive: true });
		}
	}

	/** View as List or View as Tree, which the Workspace remembers. */
	setViewMode(mode: ScmViewMode): void {
		this.#setView({ ...this.#state.view, mode });
	}

	/** Sort Changes by Name, Path or Status, which the Workspace remembers. */
	setSortKey(sortKey: ScmViewSortKey): void {
		this.#setView({ ...this.#state.view, sortKey });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#memory.changes = this.changes.viewState();
		this.#unobserve();
		this.#unsubscribeRoot();
		for (const { abort } of this.#remoteReads.values()) abort.abort();
		this.#remoteReads.clear();
		this.changes.dispose();
		this.repositories.dispose();
		this.#listeners.clear();
	}

	#setView(view: ScmViewState): void {
		const previous = this.#state.view;
		if (previous.mode === view.mode && previous.sortKey === view.sortKey) return;
		writeSourceControlViewState(this.#storage, this.#session.workspaceId, view);
		this.#publish({ ...this.#state, view });
		void this.changes.refresh(null, { recursive: true });
	}

	#snapshot(): ScmTreeSnapshot {
		const { repositories, rules, view } = this.#state;
		return {
			options: {
				compareNames: this.compareNames,
				mode: view.mode,
				separator: rules?.separator ?? '/',
				sortKey: view.sortKey,
				statusText: this.statusText,
			},
			repositories: repositories.map(({ groups, summary }) => ({
				groups,
				repository: summary.repository,
			})),
		};
	}

	#onRoot(event: FileRootEvent): void {
		if (event.kind === 'gap') return;
		// An update carries the root only when it moved, appeared or went missing.
		const root = event.root;
		if (root === null) return;
		const rules = event.kind === 'snapshot' ? event.rules : this.#state.rules;
		const separatorChanged = rules?.separator !== this.#state.rules?.separator;
		this.#publish({ ...this.#state, root, rules });
		if (separatorChanged) void this.changes.refresh(null, { recursive: true });
	}

	#onWorkspace(): void {
		const workspace = this.#workspace.state;
		const closed = this.#memory.closed;
		const displayed = workspace.repositories.filter(
			({ summary }) => !closed.has(summary.repository),
		);
		this.#readRemotes(displayed);
		this.#publish({ ...this.#state, closed: closed.size, repositories: displayed, workspace });
		const repositories = displayed.map(({ summary }) => summary.repository);
		const groups = displayed.map((repository) => repository.groups);
		const repositoriesChanged =
			repositories.length !== this.#shownRepositories.length ||
			repositories.some((repository, index) => repository !== this.#shownRepositories[index]);
		const groupsChanged = groups.some((group, index) => group !== this.#shownGroups[index]);
		this.#shownRepositories = repositories;
		this.#shownGroups = groups;
		if (repositoriesChanged) void this.repositories.refresh(null);
		if (repositoriesChanged || groupsChanged) void this.changes.refresh(null, { recursive: true });
	}

	/** Reads the remotes of each repository that is new or whose branch or upstream changed. */
	#readRemotes(repositories: readonly ScmRepositoryState[]): void {
		const displayed = new Set<string>();
		for (const { summary } of repositories) {
			displayed.add(summary.repository);
			if (!summary.warm) continue;
			const key = remotesKey(summary);
			if (this.#remoteReads.get(summary.repository)?.key === key) continue;
			this.#remoteReads.get(summary.repository)?.abort.abort();
			const abort = new AbortController();
			this.#remoteReads.set(summary.repository, { abort, key });
			void this.#session.git
				.refs({ repository: summary.repository }, abort.signal)
				.then((result) => {
					if (abort.signal.aborted || result.kind === 'error') return;
					const remotes = new Map(this.#state.remotes);
					remotes.set(summary.repository, result.value.remotes);
					this.#publish({ ...this.#state, remotes });
				});
		}
		for (const [repository, read] of this.#remoteReads) {
			if (displayed.has(repository)) continue;
			read.abort.abort();
			this.#remoteReads.delete(repository);
		}
	}

	#publish(state: SourceControlState): void {
		if (this.#disposed) return;
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}
}
