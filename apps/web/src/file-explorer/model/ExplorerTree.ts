import type { OrchbeeLocale } from '@orchbee/i18n';

import {
	type FileChange,
	type FileError,
	type FilePathRules,
	type FileRoot,
	type FileRootEvent,
	type FileWorkspaceSession,
	filePathKey,
	filePathName,
	filePathParent,
	isFilePathWithin,
} from '../../file-session';
import { VirtualTreeModel } from '../../virtual-tree';
import { EXPLORER_ROW_HEIGHT } from '../constants/explorer-row-height';
import { ExplorerListError } from './ExplorerListError';
import { ExplorerListings } from './ExplorerListings';
import { ExplorerTreeSource } from './ExplorerTreeSource';
import type { ExplorerViewMemory } from './ExplorerViewMemory';
import type { ExplorerItem } from './explorer-item';
import { isExcludedByDefault } from './isExcludedByDefault';

/** What the Explorer shows for its Workspace's root. */
export type ExplorerTreeState =
	/** The root stream has not described the root yet. */
	| { readonly kind: 'loading' }
	/** The Workspace has no current directory. */
	| { readonly kind: 'none' }
	/** A root's tree is loading with no other tree to show meanwhile, so its title shows alone. */
	| { readonly kind: 'opening'; readonly name: string; readonly root: string }
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
	/** What the Workspace's Explorer keeps across root moves and file sessions. */
	readonly memory: ExplorerViewMemory;
	/** Reports a folder below the root that could not be listed, by name, as VS Code notifies one. */
	readonly reportUnreadable: (name: string, error: FileError) => void;
	readonly session: FileWorkspaceSession;
}

/** One root's tree and the source that reads its folders. */
interface RootTree {
	/** The root's comparison key, as its rows' keys are their paths'. */
	readonly key: string;
	readonly model: VirtualTreeModel<string, ExplorerItem>;
	/** After `cd ..`, the replaced root, whose row takes the place the replaced tree's top had. */
	readonly previousRoot: string | null;
	readonly root: string;
	readonly rules: FilePathRules;
	readonly source: ExplorerTreeSource;
	/** Stops following the model, once the tree shows. */
	unsubscribe: (() => void) | null;
}

/** How long changes gather before the folders they touch are listed again, as in VS Code. */
const CHANGE_DELAY_MILLISECONDS = 500;
/**
 * How long a listing runs before VS Code's progress bar shows; it waits longer while the window
 * restores, which the Explorer's first root stands for here until it settles.
 */
const PROGRESS_DELAY_MILLISECONDS = 800;
const RESTORE_PROGRESS_DELAY_MILLISECONDS = 1500;
const LOADING: ExplorerTreeState = { kind: 'loading' };
const NONE: ExplorerTreeState = { kind: 'none' };
const RESOLVED: Promise<void> = Promise.resolve();
/**
 * The folder names of VS Code's default `explorer.autoRevealExclude`, whose globs match them at any
 * depth: the active file is not revealed at or below a folder of these names.
 */
const AUTO_REVEAL_EXCLUDED: ReadonlySet<string> = new Set(['bower_components', 'node_modules']);

function rootName(root: string, rules: FilePathRules): string {
	return filePathName(root, rules) || root;
}

function openingTree(root: string, rules: FilePathRules): ExplorerTreeState {
	return { kind: 'opening', name: rootName(root, rules), root };
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

/** Why a tree's root could not be listed: a folder below it that fails shows empty instead. */
function rootFailure(error: unknown): FileError {
	return error instanceof ExplorerListError ? error.error : { code: 'failed' };
}

/** A folder's refresh lists its expanded folders again too, so only the outermost ones refresh. */
function outermost(keys: readonly string[], rules: FilePathRules): readonly string[] {
	return keys.filter(
		(key) => !keys.some((other) => other !== key && isFilePathWithin(key, other, rules)),
	);
}

/** The folders between a root and a path below it, outermost first. */
function foldersBetween(root: string, path: string, rules: FilePathRules): readonly string[] {
	const rootKey = filePathKey(root, rules);
	const folders: string[] = [];
	for (
		let folder = filePathParent(path, rules);
		folder !== null && filePathKey(folder, rules) !== rootKey;
		folder = filePathParent(folder, rules)
	) {
		folders.push(folder);
	}
	return folders.reverse();
}

/** Whether a path lies below a root, which alone gives it a row. */
function isBelow(root: string, path: string, rules: FilePathRules): boolean {
	return (
		filePathKey(path, rules) !== filePathKey(root, rules) && isFilePathWithin(path, root, rules)
	);
}

/**
 * The entries `files.exclude` hides by name on the way from a root to a path below it, with the
 * folder that lists each, outermost first.
 */
function excludedOnWay(
	root: string,
	path: string,
	rules: FilePathRules,
): readonly { readonly entry: string; readonly folder: string }[] {
	if (!isBelow(root, path, rules)) return [];
	const folders = foldersBetween(root, path, rules);
	const entries = [...folders, path];
	return [root, ...folders].flatMap((folder, index) => {
		const entry = entries[index];
		return entry !== undefined && isExcludedByDefault(filePathName(entry, rules))
			? [{ entry, folder }]
			: [];
	});
}

/**
 * The Explorer's tree over one Workspace's file session. It follows the root stream: a new root
 * gets a new tree, which replaces the shown one once it has loaded, so the view never flashes empty.
 * Folder listings outlive roots, so a root moved up or down draws what was already listed at once
 * and lists again only what the Server stopped watching; the Workspace's memory carries expansion,
 * focus, selection and scroll across roots by path. Changes gather for 500 ms and then list again
 * only the loaded folders they affect, as VS Code's Explorer does; a rescan, which stands for the
 * changes the backend dropped below its folders, lists again every loaded folder there; and after
 * records were lost every loaded folder lists again.
 *
 * It follows the file the overlay's active tab shows, as VS Code's Explorer follows the active
 * editor: what `files.exclude` hides shows on the way to that file, and the file can be revealed.
 */
export class ExplorerTree {
	/** The file the overlay's active tab shows; null without tabs. */
	#activeFile: string | null = null;
	/** The refreshes that drew the entries on the way to the active file, which a reveal waits for. */
	#activeRefresh: Promise<void> = RESOLVED;
	#current: RootTree | null = null;
	#disposed = false;
	/** Loaded folders that changes touched, listed again when the delay ends. */
	readonly #due = new Set<string>();
	/** The shown tree whose expanded folders changed, saved once its operation's events are through. */
	#expansionChanged: RootTree | null = null;
	readonly #listeners = new Set<() => void>();
	/** Created with the first snapshot, which gives the machine's path rules. */
	#listings: ExplorerListings | null = null;
	readonly #locale: OrchbeeLocale;
	readonly #memory: ExplorerViewMemory;
	/** A new root's tree while it loads. */
	#pending: RootTree | null = null;
	/** A reveal asked while no tree showed, made once one does unless it was called off first. */
	#pendingReveal: { readonly path: string; readonly signal: AbortSignal } | null = null;
	#progress = false;
	#progressTimer: ReturnType<typeof setTimeout> | null = null;
	readonly #reportUnreadable: (name: string, error: FileError) => void;
	/** Whether the Explorer has shown its root's tree, an error or no directory since it opened. */
	#settled = false;
	/** The first snapshot's: a session stays on one machine, so they hold for its listings. */
	#rules: FilePathRules | null = null;
	readonly #session: FileWorkspaceSession;
	/** Records were lost, so the next snapshot lists every loaded folder again. */
	#stale = false;
	#state: ExplorerTreeState = LOADING;
	#timer: ReturnType<typeof setTimeout> | null = null;
	readonly #unsubscribe: () => void;

	constructor(options: ExplorerTreeOptions) {
		this.#locale = options.locale;
		this.#memory = options.memory;
		this.#reportUnreadable = options.reportUnreadable;
		this.#session = options.session;
		this.#unsubscribe = options.session.subscribe((event) => this.#follow(event));
	}

	get state(): ExplorerTreeState {
		return this.#state;
	}

	/** Whether a listing has run long enough for the progress bar to show. */
	get progress(): boolean {
		return this.#progress;
	}

	/** Hears every change of `state` and `progress`; bound, for `useSyncExternalStore`. */
	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	/** Lists every loaded folder again, as Refresh Explorer does; a root's first listing runs anyway. */
	refresh(): void {
		const current = this.#current;
		if (current !== null && this.#state.kind !== 'opening') this.#reload(current);
	}

	/**
	 * Follows the file the overlay's active tab shows. What `files.exclude` hides shows on the way to
	 * it, as VS Code's Explorer shows the files open in its visible editors and the folders holding
	 * them, so the outermost folder where that changes draws its children again. Listings follow such
	 * entries only on that way, so a folder holding one the way newly passes through lists again.
	 */
	setActiveFile(path: string | null): void {
		const previous = this.#activeFile;
		this.#activeFile = path;
		const listings = this.#listings;
		const rules = this.#rules;
		if (this.#disposed || listings === null || rules === null) return;
		const key = path === null ? null : filePathKey(path, rules);
		if (key === (previous === null ? null : filePathKey(previous, rules))) return;
		const refreshes: Promise<void>[] = [];
		for (const tree of [this.#current, this.#pending]) {
			if (tree === null) continue;
			const way = path === null ? [] : excludedOnWay(tree.root, path, rules);
			for (const { entry, folder } of way) {
				if (previous === null || !isFilePathWithin(previous, entry, rules)) {
					listings.invalidate(filePathKey(folder, rules));
				}
			}
			const folders = new Set<string>();
			for (const each of [previous, path]) {
				const outer = each === null ? undefined : excludedOnWay(tree.root, each, rules)[0];
				if (outer !== undefined) folders.add(filePathKey(outer.folder, rules));
			}
			for (const folder of outermost([...folders], rules)) {
				refreshes.push(
					tree.model.refresh(folder === tree.key ? null : folder).catch(() => undefined),
				);
			}
		}
		this.#activeRefresh =
			refreshes.length === 0 ? RESOLVED : Promise.all(refreshes).then(() => undefined);
	}

	/**
	 * Reveals a file below the root, as VS Code's Explorer reveals the active editor's file: unless its
	 * row holds the focus and the only selection already, the folders above it expand one after
	 * another, the view scrolls the row to its middle unless it shows already, and the row takes the
	 * focus and the selection. A file at or below a folder `explorer.autoRevealExclude` names stays as
	 * it is. A reveal asked while no tree shows waits for one; another root ends it.
	 */
	revealFile(path: string, signal: AbortSignal): void {
		this.#pendingReveal = null;
		const current = this.#current;
		if (current !== null && this.#state.kind === 'tree') void this.#reveal(current, path, signal);
		else this.#pendingReveal = { path, signal };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#saveExpansion();
		this.#unsubscribe();
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = null;
		if (this.#progressTimer !== null) clearTimeout(this.#progressTimer);
		this.#progressTimer = null;
		this.#due.clear();
		this.#listeners.clear();
		this.#pendingReveal = null;
		const current = this.#current;
		if (current !== null) this.#memory.rememberView(current.key, current.model.viewState());
		for (const tree of [this.#current, this.#pending]) {
			if (tree === null) continue;
			tree.unsubscribe?.();
			tree.model.dispose();
		}
		this.#current = null;
		this.#pending = null;
		this.#listings?.releaseAll();
	}

	#follow(event: FileRootEvent): void {
		if (this.#disposed) return;
		switch (event.kind) {
			case 'gap':
				this.#stale = true;
				return;
			case 'snapshot': {
				const rules = (this.#rules ??= event.rules);
				const listings = (this.#listings ??= new ExplorerListings({
					locale: this.#locale,
					onBusy: (busy) => this.#noteBusy(busy),
					rules,
					tree: this.#session.tree,
				}));
				const stale = this.#stale;
				this.#stale = false;
				// The records that were lost may include changes to any loaded folder.
				if (stale) listings.forgetHeld();
				this.#bind(listings, rules, event.root, stale);
				return;
			}
			case 'update': {
				const listings = this.#listings;
				const rules = this.#rules;
				if (listings === null || rules === null) return;
				if (event.root !== null) this.#bind(listings, rules, event.root, false);
				for (const change of event.changes) this.#note(listings, rules, change);
				for (const folder of event.rescan) this.#rescan(listings, folder);
			}
		}
	}

	#bind(listings: ExplorerListings, rules: FilePathRules, root: FileRoot, stale: boolean): void {
		if (root.state === 'none') {
			this.#cancelPending();
			this.#drop();
			this.#setState(NONE);
			return;
		}
		const key = filePathKey(root.path, rules);
		if (root.state === 'missing') {
			this.#cancelPending();
			// The tree stays, expansion and all, for the root to come back to.
			if (this.#current?.key !== key) this.#drop();
			this.#setState({
				error: null,
				kind: 'unresolved',
				name: rootName(root.path, rules),
				root: root.path,
			});
			return;
		}
		const current = this.#current;
		if (this.#pending?.key === key) return;
		if (current?.key === key) {
			this.#cancelPending();
			// The root came back, or records about it were lost.
			if (stale || this.#state.kind !== 'tree') {
				if (this.#state.kind === 'opening') this.#setState(openingTree(current.root, rules));
				this.#reload(current);
			}
			return;
		}
		this.#load(listings, rules, root.path);
	}

	#load(listings: ExplorerListings, rules: FilePathRules, root: string): void {
		this.#cancelPending();
		const key = filePathKey(root, rules);
		const source = new ExplorerTreeSource({
			activeFile: () => this.#activeFile,
			listings,
			onGone: (folder) => {
				// Its deletion is usually on the way; the parent listing again drops it either way.
				const parent = filePathParent(folder.path, rules);
				if (parent !== null) this.#noteFolder(listings, filePathKey(parent, rules));
			},
			onUnreadable: (folder, error) => {
				if (!this.#disposed) this.#reportUnreadable(folder.name, error);
			},
			rules,
		});
		const model = new VirtualTreeModel<string, ExplorerItem>({
			input: {
				directory: true,
				entry: null,
				excluded: false,
				name: rootName(root, rules),
				path: root,
			},
			source,
		});
		const current = this.#current;
		const restore = this.#memory.restore(
			key,
			current === null ? null : { root: current.key, state: current.model.viewState() },
			rules,
		);
		const tree: RootTree = {
			key,
			model,
			previousRoot: restore.previousRoot,
			root,
			rules,
			source,
			unsubscribe: null,
		};
		this.#pending = tree;
		// A shown tree stays until the new one has loaded; anything else gives way to the new title.
		if (this.#state.kind !== 'tree') this.#setState(openingTree(root, rules));
		model.restoreViewState(restore.state).then(
			() => this.#adopt(tree, null),
			(error: unknown) => this.#adopt(tree, rootFailure(error)),
		);
	}

	/** Shows a new root's tree once it has loaded, or the error row when its root failed to. */
	#adopt(tree: RootTree, error: FileError | null): void {
		if (this.#pending !== tree) return;
		this.#pending = null;
		const previous = this.#current;
		if (error === null && previous !== null && tree.previousRoot !== null) {
			// The replaced tree's rows sit below its root's row now, one row further down.
			const index = tree.model.indexOf(tree.previousRoot);
			if (index >= 0) {
				tree.model.setScrollTop(index * EXPLORER_ROW_HEIGHT + previous.model.scrollTop);
			}
		}
		this.#current = tree;
		if (previous !== null) {
			this.#memory.rememberView(previous.key, previous.model.viewState());
			this.#discard(previous, tree);
		}
		tree.unsubscribe = tree.model.subscribe((event) => {
			if (this.#current !== tree) return;
			if (event.kind === 'collapse') this.#noteExpansionChange(tree);
			this.#relistOutdated(tree);
		});
		this.#setState(error === null ? shownTree(tree) : unresolvedTree(tree, error));
		// Folders a change reached while the tree loaded list again, with those drawn unwatched.
		const listings = this.#listings;
		if (listings === null) return;
		for (const key of tree.source.takeOutdated()) listings.invalidate(key);
		this.#refreshOutdated(
			tree,
			tree.source.readFolders().filter((key) => listings.peek(key) === null),
		);
	}

	/** Saves the expanded folders once for an operation that changes several, as a reveal does. */
	#noteExpansionChange(tree: RootTree): void {
		if (this.#expansionChanged === null) queueMicrotask(() => this.#saveExpansion());
		else if (this.#expansionChanged !== tree) this.#saveExpansion();
		this.#expansionChanged = tree;
	}

	#saveExpansion(): void {
		const tree = this.#expansionChanged;
		if (tree === null) return;
		this.#expansionChanged = null;
		this.#memory.noteExpansion(tree.key, tree.model.viewState().expanded, tree.rules);
	}

	/** Lists again the folders a tree drew from listings that may lack changes. */
	#relistOutdated(tree: RootTree): void {
		const listings = this.#listings;
		const outdated = tree.source.takeOutdated();
		if (listings === null || outdated.length === 0) return;
		for (const key of outdated) listings.invalidate(key);
		this.#refreshOutdated(tree, outdated);
	}

	#refreshOutdated(tree: RootTree, keys: readonly string[]): void {
		for (const key of outermost(keys, tree.rules)) {
			tree.model
				.refresh(key === tree.key ? null : key)
				.catch(() => undefined)
				.then(() => {
					if (this.#current === tree) this.#relistOutdated(tree);
				});
		}
	}

	#reload(tree: RootTree): void {
		const listings = this.#listings;
		if (listings !== null) {
			for (const key of tree.source.readFolders()) listings.invalidate(key);
		}
		tree.model.refresh(null, { recursive: true }).then(
			() => {
				if (this.#current !== tree) return;
				if (this.#state.kind === 'tree' && this.#state.model === tree.model) return;
				this.#setState(shownTree(tree));
			},
			(error: unknown) => {
				if (this.#current === tree) this.#setState(unresolvedTree(tree, rootFailure(error)));
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
		this.#memory.rememberView(current.key, current.model.viewState());
		this.#discard(current, null);
	}

	/**
	 * Disposes a tree and releases the folders only it read; `kept` names the tree that stays, whose
	 * folders the Server keeps loaded for it, since a session holds each folder once.
	 */
	#discard(tree: RootTree, kept: RootTree | null): void {
		tree.unsubscribe?.();
		tree.unsubscribe = null;
		tree.model.dispose();
		this.#listings?.release(
			tree.source.readFolders().filter((key) => kept === null || !kept.source.reads(key)),
		);
	}

	#note(listings: ExplorerListings, rules: FilePathRules, change: FileChange): void {
		if (change.type === 'changed') return;
		const parent = filePathParent(change.path, rules);
		if (parent === null) return;
		const key = filePathKey(parent, rules);
		// A listing on its way may have read the folder before the change, whatever it shows now.
		listings.noteChange(key);
		if (!listings.lists(key)) return;
		const name = filePathName(change.path, rules);
		// What `files.exclude` hides by name shows only on the way to the active file, and the folder
		// that holds it lists again once that way comes to pass through it.
		if (isExcludedByDefault(name) && !this.#onActiveWay(change.path, rules)) return;
		const listed = listings.lists(key, name);
		// A new entry matters only where the listing lacks it, a deleted one only where it has it.
		if (change.type === 'added' ? listed : !listed) return;
		this.#noteFolder(listings, key);
	}

	/** Whether a path is the active file or a folder holding it. */
	#onActiveWay(path: string, rules: FilePathRules): boolean {
		const active = this.#activeFile;
		return active !== null && isFilePathWithin(active, path, rules);
	}

	#noteFolder(listings: ExplorerListings, key: string): void {
		if (!listings.lists(key)) return;
		listings.invalidate(key);
		this.#schedule(key);
	}

	/**
	 * The root stream dropped changes at or below `folder`, so every folder loaded there lists again,
	 * as the backend expects of a rescan, and so does each folder listing when it came: the backend
	 * sends the rescan where the first dropped change would have gone, ahead of later replies, so such
	 * a listing may predate the changes.
	 */
	#rescan(listings: ExplorerListings, folder: string): void {
		for (const key of listings.invalidateWithin(folder)) this.#schedule(key);
	}

	/** Lists a folder the shown tree read again, once changes have gathered. */
	#schedule(key: string): void {
		if (this.#current?.source.reads(key) !== true) return;
		this.#due.add(key);
		this.#timer ??= setTimeout(() => this.#flush(), CHANGE_DELAY_MILLISECONDS);
	}

	#flush(): void {
		this.#timer = null;
		const due = [...this.#due];
		this.#due.clear();
		const current = this.#current;
		// A root that is gone lists nothing; when it returns, every loaded folder lists again.
		if (current === null || this.#state.kind !== 'tree') return;
		// A folder deleted meanwhile shows empty until its parent lists again and drops it.
		this.#refreshOutdated(current, due);
	}

	#noteBusy(busy: boolean): void {
		if (this.#disposed) return;
		if (busy) {
			this.#progressTimer ??= setTimeout(
				() => {
					this.#progressTimer = null;
					this.#setProgress(true);
				},
				this.#settled ? PROGRESS_DELAY_MILLISECONDS : RESTORE_PROGRESS_DELAY_MILLISECONDS,
			);
			return;
		}
		if (this.#progressTimer !== null) clearTimeout(this.#progressTimer);
		this.#progressTimer = null;
		this.#setProgress(false);
	}

	#setProgress(progress: boolean): void {
		if (this.#progress === progress) return;
		this.#progress = progress;
		for (const listener of this.#listeners) listener();
	}

	#setState(state: ExplorerTreeState): void {
		if (this.#disposed) return;
		this.#state = state;
		if (state.kind !== 'loading' && state.kind !== 'opening') this.#settled = true;
		for (const listener of this.#listeners) listener();
		const reveal = this.#pendingReveal;
		const current = this.#current;
		if (reveal === null || current === null || state.kind !== 'tree') return;
		this.#pendingReveal = null;
		void this.#reveal(current, reveal.path, reveal.signal);
	}

	async #reveal(tree: RootTree, path: string, signal: AbortSignal): Promise<void> {
		const { model, root, rules } = tree;
		const key = filePathKey(path, rules);
		if (signal.aborted || !isBelow(root, path, rules)) return;
		if (model.focus === key && model.selection.size === 1 && model.selection.has(key)) return;
		const folders = foldersBetween(root, path, rules);
		if ([...folders, path].some((each) => AUTO_REVEAL_EXCLUDED.has(filePathName(each, rules)))) {
			return;
		}
		// What `files.exclude` hides on the way shows once the tree has drawn it for this file.
		await this.#activeRefresh;
		for (const folder of folders) {
			if (signal.aborted || this.#current !== tree) return;
			await model.expand(filePathKey(folder, rules)).catch(() => undefined);
		}
		if (signal.aborted || this.#current !== tree || model.indexOf(key) < 0) return;
		model.reveal(key, { onlyOutOfSight: true, relativeTop: 0.5 });
		model.setFocus(key);
		model.setSelection([key]);
	}
}
