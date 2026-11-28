import type { OrchbeeLocale } from '@orchbee/i18n';

import {
	type FileChange,
	type FileError,
	type FilePathRules,
	type FileRoot,
	type FileRootEvent,
	type FileWorkspaceSession,
	filePathName,
	filePathParent,
	isFilePathWithin,
} from '../../file-session';
import { VirtualTreeModel } from '../../virtual-tree';
import { ExplorerListError } from './ExplorerListError';
import { ExplorerTreeSource } from './ExplorerTreeSource';
import type { ExplorerItem } from './explorer-item';
import { isExcludedByDefault } from './isExcludedByDefault';

/** What the Explorer shows for its Workspace's root. */
export type ExplorerTreeState =
	/** The root stream has not described the root yet. */
	| { readonly kind: 'loading' }
	/** The Workspace has no current directory. */
	| { readonly kind: 'none' }
	| {
			readonly kind: 'tree';
			readonly model: VirtualTreeModel<string, ExplorerItem>;
			/** What VS Code titles the root: its folder's name, or the whole path of a volume root. */
			readonly name: string;
			readonly root: string;
			readonly rules: FilePathRules;
	  }
	/**
	 * The root is gone or could not be listed; VS Code shows the error row "Unable to resolve
	 * workspace folder", and the tree comes back once the root does. A root that is gone has no error.
	 */
	| {
			readonly error: FileError | null;
			readonly kind: 'unresolved';
			readonly name: string;
			readonly root: string;
	  };

interface ExplorerTreeOptions {
	readonly locale: OrchbeeLocale;
	readonly session: FileWorkspaceSession;
}

/** One root's tree and the source that lists its folders. */
interface RootTree {
	readonly model: VirtualTreeModel<string, ExplorerItem>;
	readonly root: string;
	readonly rules: FilePathRules;
	readonly source: ExplorerTreeSource;
}

/** How long changes gather before the folders they touch are listed again, as in VS Code. */
const CHANGE_DELAY_MILLISECONDS = 500;
const LOADING: ExplorerTreeState = { kind: 'loading' };
const NONE: ExplorerTreeState = { kind: 'none' };

function rootName(root: string, rules: FilePathRules): string {
	return filePathName(root, rules) || root;
}

function shownTree(tree: RootTree): ExplorerTreeState {
	return {
		kind: 'tree',
		model: tree.model,
		name: rootName(tree.root, tree.rules),
		root: tree.root,
		rules: tree.rules,
	};
}

function unresolvedTree(tree: RootTree, error: FileError): ExplorerTreeState {
	return { error, kind: 'unresolved', name: rootName(tree.root, tree.rules), root: tree.root };
}

function listFailure(error: unknown): FileError {
	return error instanceof ExplorerListError ? error.error : { code: 'failed' };
}

/**
 * The Explorer's tree over one Workspace's file session. It follows the root stream: a new root
 * gets a new tree, which replaces the shown one once its top level has loaded, so the view never
 * flashes empty; changes gather for 500 ms and then list again only the loaded folders they
 * affect, as VS Code's Explorer does; and after records were lost every loaded folder lists again.
 */
export class ExplorerTree {
	#current: RootTree | null = null;
	#disposed = false;
	/** Loaded folders that changes touched, listed again when the delay ends. */
	readonly #due = new Set<string>();
	readonly #listeners = new Set<() => void>();
	readonly #locale: OrchbeeLocale;
	/** A new root's tree while its top level loads. */
	#pending: RootTree | null = null;
	#rules: FilePathRules | null = null;
	readonly #session: FileWorkspaceSession;
	/** Records were lost, so the next snapshot lists every loaded folder again. */
	#stale = false;
	#state: ExplorerTreeState = LOADING;
	#timer: ReturnType<typeof setTimeout> | null = null;
	readonly #unsubscribe: () => void;

	constructor(options: ExplorerTreeOptions) {
		this.#locale = options.locale;
		this.#session = options.session;
		this.#unsubscribe = options.session.subscribe((event) => this.#follow(event));
	}

	get state(): ExplorerTreeState {
		return this.#state;
	}

	/** Hears every change of `state`; bound, so a view can hand it to `useSyncExternalStore`. */
	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	/** Lists every loaded folder again, as Refresh Explorer does. */
	refresh(): void {
		const current = this.#current;
		if (current !== null) this.#reload(current);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#unsubscribe();
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = null;
		this.#due.clear();
		this.#listeners.clear();
		const loaded = new Set<string>();
		for (const tree of [this.#current, this.#pending]) {
			if (tree === null) continue;
			tree.model.dispose();
			for (const folder of tree.source.listedFolders()) loaded.add(folder);
		}
		this.#current = null;
		this.#pending = null;
		this.#session.tree.release([...loaded]);
	}

	#follow(event: FileRootEvent): void {
		if (this.#disposed) return;
		switch (event.kind) {
			case 'gap':
				this.#stale = true;
				return;
			case 'snapshot': {
				this.#rules = event.rules;
				const stale = this.#stale;
				this.#stale = false;
				this.#bind(event.root, stale);
				return;
			}
			case 'update':
				if (event.root !== null) this.#bind(event.root, false);
				for (const change of event.changes) this.#note(change);
				for (const folder of event.rescan) this.#noteFolder(folder);
		}
	}

	#bind(root: FileRoot, stale: boolean): void {
		const rules = this.#rules;
		if (rules === null) return;
		if (root.state === 'none') {
			this.#cancelPending();
			this.#drop();
			this.#setState(NONE);
			return;
		}
		if (root.state === 'missing') {
			this.#cancelPending();
			// The tree stays, expansion and all, for the root to come back to.
			if (this.#current?.root !== root.path) this.#drop();
			this.#setState({
				error: null,
				kind: 'unresolved',
				name: rootName(root.path, rules),
				root: root.path,
			});
			return;
		}
		const current = this.#current;
		if (this.#pending?.root === root.path) return;
		if (current?.root === root.path) {
			this.#cancelPending();
			// The root came back, or records about it were lost.
			if (stale || this.#state.kind !== 'tree') this.#reload(current);
			return;
		}
		this.#load(root.path, rules);
	}

	#load(root: string, rules: FilePathRules): void {
		this.#cancelPending();
		const source = new ExplorerTreeSource({
			locale: this.#locale,
			rules,
			tree: this.#session.tree,
		});
		const model = new VirtualTreeModel<string, ExplorerItem>({
			input: { directory: true, entry: null, name: rootName(root, rules), path: root },
			source,
		});
		const tree: RootTree = { model, root, rules, source };
		this.#pending = tree;
		model.refresh(null).then(
			() => this.#adopt(tree, null),
			(error: unknown) => this.#adopt(tree, listFailure(error)),
		);
	}

	/** Shows a new root's tree once its top level has loaded or failed to. */
	#adopt(tree: RootTree, error: FileError | null): void {
		if (this.#pending !== tree) return;
		this.#pending = null;
		const previous = this.#current;
		this.#current = tree;
		if (previous !== null) this.#discard(previous, tree);
		this.#setState(error === null ? shownTree(tree) : unresolvedTree(tree, error));
	}

	#reload(tree: RootTree): void {
		tree.model.refresh(null, { recursive: true }).then(
			() => {
				if (this.#current !== tree) return;
				if (this.#state.kind === 'tree' && this.#state.model === tree.model) return;
				this.#setState(shownTree(tree));
			},
			(error: unknown) => {
				// A folder below the root that fails only collapses its row.
				if (this.#current !== tree || !(error instanceof ExplorerListError)) return;
				if (error.path !== tree.root) return;
				this.#setState(unresolvedTree(tree, error.error));
			},
		);
	}

	#cancelPending(): void {
		const pending = this.#pending;
		if (pending === null) return;
		this.#pending = null;
		this.#discard(pending, this.#current);
	}

	#drop(): void {
		const current = this.#current;
		if (current === null) return;
		this.#current = null;
		this.#discard(current, null);
	}

	/**
	 * Disposes a tree and releases the folders only it listed; `kept` names the tree that stays,
	 * whose folders the Server keeps loaded for it, since a session holds each folder once.
	 */
	#discard(tree: RootTree, kept: RootTree | null): void {
		tree.model.dispose();
		const released = tree.source
			.listedFolders()
			.filter((folder) => kept === null || !kept.source.shows(folder));
		if (released.length > 0) this.#session.tree.release(released);
	}

	#note(change: FileChange): void {
		const current = this.#current;
		if (current === null || change.type === 'changed') return;
		const parent = filePathParent(change.path, current.rules);
		if (parent === null || !current.source.shows(parent)) return;
		const name = filePathName(change.path, current.rules);
		const shown = current.source.shows(parent, name);
		// A new entry matters only where the tree does not show it yet, a deleted one only where it does.
		if (change.type === 'added' ? shown || isExcludedByDefault(name) : !shown) return;
		this.#schedule(parent);
	}

	#noteFolder(folder: string): void {
		if (this.#current?.source.shows(folder) === true) this.#schedule(folder);
	}

	#schedule(folder: string): void {
		this.#due.add(folder);
		if (this.#timer !== null) return;
		this.#timer = setTimeout(() => this.#flush(), CHANGE_DELAY_MILLISECONDS);
	}

	#flush(): void {
		this.#timer = null;
		const due = [...this.#due];
		this.#due.clear();
		const current = this.#current;
		// A root that is gone lists nothing; when it returns, every loaded folder lists again.
		if (current === null || this.#state.kind !== 'tree') return;
		// A folder's refresh lists its expanded folders again too, so one below another due folder waits.
		const outermost = due.filter(
			(folder) =>
				!due.some((other) => other !== folder && isFilePathWithin(folder, other, current.rules)),
		);
		for (const folder of outermost) {
			// A folder deleted meanwhile only collapses; its deletion arrives on its own.
			current.model.refresh(folder === current.root ? null : folder).catch(() => undefined);
		}
	}

	#setState(state: ExplorerTreeState): void {
		if (this.#disposed) return;
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}
}
