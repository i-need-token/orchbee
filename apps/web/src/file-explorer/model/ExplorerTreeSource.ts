import { type FilePathRules, filePathKey } from '../../file-session';
import type { VirtualTreeSource } from '../../virtual-tree';
import type { ExplorerItem } from './explorer-item';
import type { ExplorerListings } from './ExplorerListings';

interface ExplorerTreeSourceOptions {
	readonly listings: ExplorerListings;
	readonly rules: FilePathRules;
}

/**
 * One root's tree as the virtual tree reads it, keyed by each path's comparison key so a root moved
 * to another spelling of a folder still finds its rows. A folder whose listing is kept draws at once;
 * one the Server stopped keeping draws too, and waits to be listed again.
 */
export class ExplorerTreeSource implements VirtualTreeSource<string, ExplorerItem> {
	readonly key = (item: ExplorerItem): string => filePathKey(item.path, this.#rules);
	// A name that is not valid UTF-8 cannot be named in a listing, so its folder never opens.
	readonly hasChildren = (item: ExplorerItem): boolean =>
		item.directory && item.entry?.unrepresentable !== true;
	readonly children = (
		item: ExplorerItem,
		signal: AbortSignal,
	): readonly ExplorerItem[] | Promise<readonly ExplorerItem[]> => {
		const key = this.key(item);
		const kept = this.#listings.peek(key);
		if (kept !== null) {
			this.#read.add(key);
			if (!kept.held) this.#unheld.add(key);
			return kept.listing.items;
		}
		return this.#listings.list(item.path, signal).then((listing) => {
			this.#read.add(key);
			return listing.items;
		});
	};

	readonly #listings: ExplorerListings;
	/** Folders this tree has read, which the Server keeps loaded while it shows. */
	readonly #read = new Set<string>();
	readonly #rules: FilePathRules;
	/** Folders read from listings the Server no longer keeps, not yet listed again. */
	readonly #unheld = new Set<string>();

	constructor(options: ExplorerTreeSourceOptions) {
		this.#listings = options.listings;
		this.#rules = options.rules;
	}

	/** Whether this tree has read a folder. */
	reads(key: string): boolean {
		return this.#read.has(key);
	}

	readFolders(): readonly string[] {
		return [...this.#read];
	}

	/** Takes the folders read from listings the Server no longer keeps, to list them again. */
	takeUnheld(): readonly string[] {
		const taken = [...this.#unheld];
		this.#unheld.clear();
		return taken;
	}
}
