import { type FileError, type FilePathRules, filePathKey } from '../../file-session';
import type { VirtualTreeSource } from '../../virtual-tree';
import { ExplorerListError } from './ExplorerListError';
import type { ExplorerItem } from './explorer-item';
import type { ExplorerListings } from './ExplorerListings';

interface ExplorerTreeSourceOptions {
	readonly listings: ExplorerListings;
	/** Hears a folder below the root that is gone, so its parent lists again and drops it. */
	readonly onGone: (folder: ExplorerItem) => void;
	/** Hears a folder below the root that could not be listed, which shows empty. */
	readonly onUnreadable: (folder: ExplorerItem, error: FileError) => void;
	readonly rules: FilePathRules;
}

const NO_ITEMS: readonly ExplorerItem[] = Object.freeze([]);

/**
 * One root's tree as the virtual tree reads it, keyed by each path's comparison key so a root moved
 * to another spelling of a folder still finds its rows, and with VS Code's compact folders. A folder whose listing is kept draws at once;
 * one the Server stopped keeping draws too, and waits to be listed again, as does one that went out
 * of date while this tree first listed its folder.
 *
 * A folder below the root that cannot be listed shows empty, as in VS Code, which reports why. One
 * that is gone usually has its deletion on the way, so it only has its parent list again; the parent
 * that still lists it after that makes the next failure a report, and never a second listing.
 */
export class ExplorerTreeSource implements VirtualTreeSource<string, ExplorerItem> {
	readonly key = (item: ExplorerItem): string => filePathKey(item.path, this.#rules);
	// A name that is not valid UTF-8 cannot be named in a listing, so its folder never opens.
	readonly hasChildren = (item: ExplorerItem): boolean =>
		item.directory && item.entry?.unrepresentable !== true;
	/**
	 * VS Code's compact folders: a folder that is its parent's only child, once the default excludes
	 * leave it alone, shows in its parent's row; a file never joins, nor a folder that cannot open.
	 */
	readonly compressible = (item: ExplorerItem): boolean => this.hasChildren(item);
	readonly children = (
		item: ExplorerItem,
		signal: AbortSignal,
	): readonly ExplorerItem[] | Promise<readonly ExplorerItem[]> => {
		const key = this.key(item);
		const kept = this.#listings.peek(key);
		if (kept !== null) {
			this.#read.add(key);
			if (!kept.held) this.#outdated.add(key);
			return kept.listing.items;
		}
		const read = this.#read.has(key);
		return this.#listings.list(item.path, signal).then(
			(listing) => {
				this.#read.add(key);
				this.#gone.delete(key);
				// Gathered changes list again only folders the tree had read, so a listing that a change
				// or a rescan overtook before then lists again at once.
				if (!read && this.#listings.peek(key) === null) this.#outdated.add(key);
				return listing.items;
			},
			(error: unknown) => {
				// The root, which alone has no entry, shows its failure as the unresolved row instead.
				if (item.entry === null || !(error instanceof ExplorerListError)) throw error;
				const { code } = error.error;
				if ((code === 'not-found' || code === 'not-directory') && !this.#gone.delete(key)) {
					this.#gone.add(key);
					this.#onGone(item);
				} else {
					this.#onUnreadable(item, error.error);
				}
				return NO_ITEMS;
			},
		);
	};

	/** Folders found gone whose parents were asked to list again. */
	readonly #gone = new Set<string>();
	readonly #listings: ExplorerListings;
	readonly #onGone: (folder: ExplorerItem) => void;
	readonly #onUnreadable: (folder: ExplorerItem, error: FileError) => void;
	/** Folders this tree has read, which the Server keeps loaded while it shows. */
	readonly #read = new Set<string>();
	readonly #rules: FilePathRules;
	/**
	 * Folders read from listings that may lack changes, not yet listed again: one the Server no longer
	 * keeps, or one that went out of date while the tree first listed its folder.
	 */
	readonly #outdated = new Set<string>();

	constructor(options: ExplorerTreeSourceOptions) {
		this.#listings = options.listings;
		this.#onGone = options.onGone;
		this.#onUnreadable = options.onUnreadable;
		this.#rules = options.rules;
	}

	/** Whether this tree has read a folder. */
	reads(key: string): boolean {
		return this.#read.has(key);
	}

	readFolders(): readonly string[] {
		return [...this.#read];
	}

	/** Takes the folders read from listings that may lack changes, to list them again. */
	takeOutdated(): readonly string[] {
		const taken = [...this.#outdated];
		this.#outdated.clear();
		return taken;
	}
}
