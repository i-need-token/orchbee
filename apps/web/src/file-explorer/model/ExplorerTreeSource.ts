import {
	type FileError,
	type FilePathRules,
	filePathKey,
	isFilePathWithin,
} from '../../file-session';
import type { VirtualTreeSource } from '../../virtual-tree';
import { ExplorerListError } from './ExplorerListError';
import type { ExplorerItem } from './explorer-item';
import type { ExplorerListing, ExplorerListings } from './ExplorerListings';

interface ExplorerTreeSourceOptions {
	/** The path of the file the overlay's active tab shows, read whenever a folder's children are. */
	readonly activeFile: () => string | null;
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
 * to another spelling of a folder still finds its rows, and with VS Code's compact folders, whose
 * folders list in one request. A folder whose listing is kept draws at once; one the Server stopped
 * keeping draws too, and waits to be listed again, as does one that went out of date while this
 * tree first listed its folder.
 *
 * A folder below the root that cannot be listed shows empty, as in VS Code, which reports why. One
 * that is gone usually has its deletion on the way, so it only has its parent list again; the parent
 * that still lists it after that makes the next failure a report, and never a second listing.
 *
 * What `files.exclude` hides shows only on the way to the file the overlay's active tab shows, as VS
 * Code's Explorer shows the files open in its visible editors and the folders that hold them. Below
 * a folder that shows that way, everything else stays hidden too.
 */
export class ExplorerTreeSource implements VirtualTreeSource<string, ExplorerItem> {
	readonly key = (item: ExplorerItem): string => filePathKey(item.path, this.#rules);
	// A folder keeps its twistie even when the excludes hide every child, as in VS Code, whose Explorer
	// drops the twistie only of a nesting parent whose nested files are all hidden. A name that is not
	// valid UTF-8 cannot be named in a listing, so its folder never opens.
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
			return this.#shown(item, kept.listing);
		}
		const read = this.#read.has(key);
		// The root's only child does not join it, so only a folder below the root lists its chain.
		return this.#listings.list(item.path, signal, item.entry !== null).then(
			({ chain, listing }) => {
				this.#read.add(key);
				for (const folder of chain) this.#read.add(folder);
				this.#gone.delete(key);
				// Gathered changes list again only folders the tree had read, so a listing that a change
				// or a rescan overtook before then lists again at once.
				if (!read && this.#listings.peek(key) === null) this.#outdated.add(key);
				return this.#shown(item, listing);
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

	readonly #activeFile: () => string | null;
	/** The items excluded through a folder above them, by their listing's item, kept for each row. */
	readonly #excludedThrough = new WeakMap<ExplorerItem, ExplorerItem>();
	/** Folders found gone whose parents were asked to list again. */
	readonly #gone = new Set<string>();
	readonly #listings: ExplorerListings;
	readonly #onGone: (folder: ExplorerItem) => void;
	readonly #onUnreadable: (folder: ExplorerItem, error: FileError) => void;
	/**
	 * Folders this tree has read, and the chains that listed with them, which the Server keeps loaded
	 * while it shows.
	 */
	readonly #read = new Set<string>();
	readonly #rules: FilePathRules;
	/**
	 * Folders read from listings that may lack changes, not yet listed again: one the Server no longer
	 * keeps, or one that went out of date while the tree first listed its folder.
	 */
	readonly #outdated = new Set<string>();

	constructor(options: ExplorerTreeSourceOptions) {
		this.#activeFile = options.activeFile;
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

	/** A folder's children as the tree shows them, the listing's own while nothing excluded shows. */
	#shown(folder: ExplorerItem, listing: ExplorerListing): readonly ExplorerItem[] {
		const active = this.#activeFile();
		if (active === null) return folder.excluded ? NO_ITEMS : listing.items;
		const leads = (child: ExplorerItem): boolean =>
			isFilePathWithin(active, child.path, this.#rules);
		if (!folder.excluded) {
			const shown = listing.excluded.filter(leads);
			if (shown.length === 0) return listing.items;
			return [...listing.items, ...shown].sort(this.#listings.compare);
		}
		// One child at most holds the active file, and below an excluded folder it is excluded too.
		const child = listing.items.find(leads) ?? listing.excluded.find(leads);
		if (child === undefined) return NO_ITEMS;
		if (child.excluded) return [child];
		let through = this.#excludedThrough.get(child);
		if (through === undefined) {
			through = { ...child, excluded: true };
			this.#excludedThrough.set(child, through);
		}
		return [through];
	}
}
