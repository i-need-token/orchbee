import { type FilePathRules, filePathParent, isFilePathWithin } from '../../file-session';
import type { VirtualTreeModel } from '../../virtual-tree';
import { EXPLORER_EXPANSION_LIMIT } from '../constants/explorer-expansion-limit';
import type { ExplorerItem } from './explorer-item';

/** A root's tree state, as the virtual tree saves and restores it. */
type ExplorerTreeViewState = ReturnType<VirtualTreeModel<string, ExplorerItem>['viewState']>;

/** A Workspace's expanded folders as this device keeps them between runs. */
export interface ExplorerExpansionRecord {
	/** The saved folders' comparison keys, least recently expanded first. */
	read(): readonly string[];
	/** Saves the folders once the person has expanded or collapsed one. */
	write(expanded: readonly string[]): void;
}

/** What a new root's tree restores. */
interface ExplorerRestore {
	readonly state: ExplorerTreeViewState;
	/**
	 * The root the tree replaces, when the new root holds it: after `cd ..` it shows as an expanded
	 * folder, and its row takes the place the replaced tree's top had, so what was in sight stays.
	 */
	readonly previousRoot: string | null;
}

/** The tree a new root replaces: its root's key and its state. */
interface ExplorerPreviousTree {
	readonly root: string;
	readonly state: ExplorerTreeViewState;
}

/** Roots whose focus, selection and scroll stay in memory, least recently left going first. */
const ROOT_VIEWS = 20;

/**
 * What one Workspace's Explorer keeps across root moves, by path rather than by root as VS Code
 * does, so moving the root up or down keeps what was expanded: the expanded folders, which the
 * Workspace shares and the record saves, and each recent root's focus, selection and scroll.
 */
export class ExplorerViewMemory {
	/** Comparison keys, least recently expanded first. */
	readonly #expanded: Set<string>;
	readonly #record: ExplorerExpansionRecord | null;
	/** By root key, least recently left first; their expansion lives in `#expanded`. */
	readonly #views = new Map<string, ExplorerTreeViewState>();

	constructor(record: ExplorerExpansionRecord | null) {
		this.#record = record;
		this.#expanded = new Set(record?.read() ?? []);
	}

	/**
	 * The state a root's tree restores: the Workspace's expanded folders below it, and what the tree
	 * it replaces had selected and focused there; otherwise what the root last had, scroll included.
	 * After `cd ..` focus lands on the replaced root.
	 */
	restore(
		root: string,
		previous: ExplorerPreviousTree | null,
		rules: FilePathRules,
	): ExplorerRestore {
		const below = (key: string): boolean => key !== root && isFilePathWithin(key, root, rules);
		const remembered = this.#views.get(root);
		const expanded = [...this.#expanded].filter(below);
		let { anchor, focus, selection } = remembered ?? { anchor: null, focus: null, selection: [] };
		let previousRoot: string | null = null;
		if (previous !== null) {
			const carried = previous.state.selection.filter(below);
			if (carried.length > 0) selection = carried;
			if (previous.state.focus !== null && below(previous.state.focus)) {
				focus = previous.state.focus;
			}
			if (previous.state.anchor !== null && below(previous.state.anchor)) {
				anchor = previous.state.anchor;
			}
			if (below(previous.root)) {
				previousRoot = previous.root;
				focus = previous.root;
				anchor = previous.root;
				for (let key: string | null = previous.root; key !== null && below(key);) {
					expanded.push(key);
					key = filePathParent(key, rules);
				}
			}
		}
		return {
			previousRoot,
			state: { anchor, expanded, focus, scrollTop: remembered?.scrollTop ?? 0, selection },
		};
	}

	/** Takes in a shown tree's expanded folders after the person expanded or collapsed one. */
	noteExpansion(root: string, expanded: readonly string[], rules: FilePathRules): void {
		for (const key of this.#expanded) {
			if (key !== root && isFilePathWithin(key, root, rules)) this.#expanded.delete(key);
		}
		for (const key of expanded) this.#expanded.add(key);
		for (const key of this.#expanded) {
			if (this.#expanded.size <= EXPLORER_EXPANSION_LIMIT) break;
			this.#expanded.delete(key);
		}
		this.#record?.write([...this.#expanded]);
	}

	/** Remembers where a root's tree was once another root replaces it. */
	rememberView(root: string, state: ExplorerTreeViewState): void {
		this.#views.delete(root);
		this.#views.set(root, { ...state, expanded: [] });
		for (const key of this.#views.keys()) {
			if (this.#views.size <= ROOT_VIEWS) break;
			this.#views.delete(key);
		}
	}
}
