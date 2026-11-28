import type { VirtualTreeFindMatchType, VirtualTreeRow } from '../model/virtual-tree-types';

/** How find shows what it found: highlighted among every row, or the rows filtered down to it. */
export type VirtualTreeFindMode = 'highlight' | 'filter';

/** What a tree's find starts with, and where it looks beyond the loaded rows. */
export interface VirtualTreeFindOptions<K, T> {
	/** `highlight` unless set, as VS Code's `workbench.list.defaultFindMode` is by default. */
	readonly mode?: VirtualTreeFindMode | undefined;
	/** `fuzzy` unless set, as VS Code's `workbench.list.defaultFindMatchType` is by default. */
	readonly matchType?: VirtualTreeFindMatchType | undefined;
	/** Finds beyond the loaded rows, as VS Code's Explorer finds files through its search. */
	readonly provider?: VirtualTreeFindProvider<K, T> | undefined;
}

/** What a find asks its provider for besides the pattern. */
interface VirtualTreeFindProviderOptions {
	readonly matchType: VirtualTreeFindMatchType;
	readonly mode: VirtualTreeFindMode;
}

/** What a provider found for one pattern. */
export interface VirtualTreeFindResult<K, T> {
	/** How many items it found, which the find announces and reports when there are none. */
	readonly matchCount: number;
	/** Shown in place of the find's message, such as that the results stop at a limit. */
	readonly warning?: string | undefined;
	/** Whether a row is one it found, which keys that move focus may land on. */
	readonly isMatch: (row: VirtualTreeRow<K, T>) => boolean;
}

/**
 * VS Code's asynchronous find provider: it finds what the loaded rows cannot show, such as files
 * in folders that never loaded, and brings them into the tree through its source. The tree finds
 * among the loaded rows on every keystroke and asks the provider once typing rests for 250 ms, one
 * request at a time; a newer pattern, closing the find, or disposing the tree aborts `signal`.
 */
export interface VirtualTreeFindProvider<K, T> {
	/** Called before the first `find` once a character is typed, for state the session keeps. */
	readonly startSession?: (() => void) | undefined;
	/** Finds `pattern`; undefined leaves the rows and messages as they are, as an aborted find does. */
	readonly find: (
		pattern: string,
		options: VirtualTreeFindProviderOptions,
		signal: AbortSignal,
	) => Promise<VirtualTreeFindResult<K, T> | undefined>;
	/**
	 * While filtering, whether a row may show, with its subtree, once a result has arrived; asked
	 * again whenever the rows settle, so it answers from the provider's latest result.
	 */
	readonly isVisible?: ((row: VirtualTreeRow<K, T>) => boolean) | undefined;
	/** Called when the find empties or closes after a session started, to free its state. */
	readonly endSession?: (() => Promise<void>) | undefined;
}
