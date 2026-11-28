import type { FileEntry } from '../../file-session';

/** One row of the Explorer, or the root its rows sit in, which has no row of its own. */
export interface ExplorerItem {
	/** A folder or a link to one, which sorts with the folders. */
	readonly directory: boolean;
	/** The entry its folder's listing reported; null for the root. */
	readonly entry: FileEntry | null;
	readonly name: string;
	/** The absolute path in the machine's spelling, which keys the item's row. */
	readonly path: string;
}
