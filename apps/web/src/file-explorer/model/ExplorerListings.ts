import type { OrchbeeLocale } from '@orchbee/i18n';

import {
	type FileEntry,
	type FileListPage,
	type FilePathRules,
	type FileTreePort,
	filePathChild,
	filePathKey,
	isFilePathWithin,
} from '../../file-session';
import { ExplorerListError } from './ExplorerListError';
import type { ExplorerItem } from './explorer-item';
import { explorerItemComparer } from './explorerItemComparer';
import { isExcludedByDefault } from './isExcludedByDefault';

/** One folder's children as its latest listing reported them. */
export interface ExplorerListing {
	/** The children by name, which a later listing reuses for entries that did not change. */
	readonly byName: ReadonlyMap<string, ExplorerItem>;
	/** The path the folder was listed by, which releases it. */
	readonly folder: string;
	/** The children as the Explorer sorts them. */
	readonly items: readonly ExplorerItem[];
}

/** A folder's listing and the folders below it that listed with it. */
export interface ExplorerListed {
	/**
	 * The keys of the folder's single-child chain: each folder below it that is the one item its
	 * parent shows, which the Server now keeps loaded too.
	 */
	readonly chain: readonly string[];
	readonly listing: ExplorerListing;
}

/** The single-child chain a listing brings, outermost folder first. */
type ListedChain = FileListPage['descendants'];

interface KeptListing extends ExplorerListing {
	/** A change made the listing out of date, so the folder lists again before it shows. */
	dirty: boolean;
	/**
	 * The Server keeps the folder loaded and reports its changes. A listing it no longer keeps may
	 * have missed changes, so it shows only until the folder has listed again.
	 */
	held: boolean;
}

/** A listing running now, which hears the changes it may have read its folders before. */
interface RunningListing {
	readonly key: string;
	/** An entry of the folder was added or deleted. */
	changed: boolean;
	/** A change or a rescan reached a folder below it, where its single-child chain lies. */
	changedBelow: boolean;
}

interface ExplorerListingsOptions {
	readonly locale: OrchbeeLocale;
	/** Hears when the first listing starts and when the last one running ends. */
	readonly onBusy: (busy: boolean) => void;
	readonly rules: FilePathRules;
	readonly tree: FileTreePort;
}

/**
 * Listings the Server no longer keeps loaded stay for a root move back to them, least recently used
 * going first past either bound.
 */
const KEPT_FOLDERS = 1000;
const KEPT_ITEMS = 50_000;

function sameEntry(entry: FileEntry | null, other: FileEntry): boolean {
	return (
		entry !== null &&
		entry.mtimeMs === other.mtimeMs &&
		entry.readonly === other.readonly &&
		entry.size === other.size &&
		entry.symlink === other.symlink &&
		entry.type === other.type &&
		entry.unrepresentable === other.unrepresentable
	);
}

/**
 * One file session's folder listings, by folder key, kept across root moves so a folder shown again
 * draws at once. Each listing gathers every page of its folder, hides what `files.exclude` hides and
 * sorts as VS Code's Explorer does; an entry that did not change keeps its item, so its row draws
 * nothing again, and a later page's entry replaces an earlier one of the same name, since a folder
 * that changes between pages may repeat entries. A folder may list with its single-child chain, as
 * VS Code's Explorer resolves a folder with its single-child descendants.
 */
export class ExplorerListings {
	/** Least recently used first. */
	readonly #kept = new Map<string, KeptListing>();
	readonly #compare: (one: ExplorerItem, other: ExplorerItem) => number;
	/** How often each folder was made out of date, so a listing that overlapped a change stays so. */
	readonly #invalidations = new Map<string, number>();
	readonly #onBusy: (busy: boolean) => void;
	readonly #rules: FilePathRules;
	readonly #runs = new Set<RunningListing>();
	readonly #tree: FileTreePort;

	constructor(options: ExplorerListingsOptions) {
		this.#compare = explorerItemComparer(options.locale);
		this.#onBusy = options.onBusy;
		this.#rules = options.rules;
		this.#tree = options.tree;
	}

	/** A kept listing nothing is known to have changed, and whether the Server still keeps it. */
	peek(key: string): { readonly held: boolean; readonly listing: ExplorerListing } | null {
		const listing = this.#kept.get(key);
		if (listing === undefined || listing.dirty) return null;
		this.#kept.delete(key);
		this.#kept.set(key, listing);
		return { held: listing.held, listing };
	}

	/** Whether a folder's listing is kept and, given a name, whether that listing shows the entry. */
	shows(key: string, name?: string): boolean {
		const listing = this.#kept.get(key);
		if (listing === undefined) return false;
		return name === undefined || listing.byName.has(name);
	}

	/**
	 * Lists a folder; the Server then keeps it loaded and reports its changes until it is released.
	 * With `singleChild`, each folder below it that is its parent's only child lists with it, so the
	 * compact row they make draws from one request.
	 */
	async list(folder: string, signal: AbortSignal, singleChild: boolean): Promise<ExplorerListed> {
		const run: RunningListing = {
			changed: false,
			changedBelow: false,
			key: filePathKey(folder, this.#rules),
		};
		this.#runs.add(run);
		if (this.#runs.size === 1) this.#onBusy(true);
		try {
			return await this.#list(folder, run, signal, singleChild);
		} finally {
			this.#runs.delete(run);
			if (this.#runs.size === 0) this.#onBusy(false);
		}
	}

	async #list(
		folder: string,
		run: RunningListing,
		signal: AbortSignal,
		singleChild: boolean,
	): Promise<ExplorerListed> {
		const { key } = run;
		const invalidations = this.#invalidations.get(key) ?? 0;
		const entries = new Map<string, FileEntry>();
		let chain: ListedChain = [];
		let cursor: string | undefined;
		do {
			const page = await this.#tree.list({ cursor, path: folder, singleChild }, signal);
			if (page.kind === 'error') {
				if (page.error.code === 'cancelled') {
					throw new DOMException('Listing cancelled.', 'AbortError');
				}
				throw new ExplorerListError(folder, page.error);
			}
			for (const entry of page.value.entries) entries.set(entry.name, entry);
			// The chain comes only with a first page that holds every entry.
			if (cursor === undefined) chain = page.value.descendants;
			cursor = page.value.cursor ?? undefined;
		} while (cursor !== undefined);
		const changed = run.changed || (this.#invalidations.get(key) ?? 0) !== invalidations;
		const listing = this.#keep(folder, key, entries.values(), changed);
		const kept = this.#keepChain(listing, chain, run.changedBelow);
		this.#evict();
		return { chain: kept, listing };
	}

	/** Keeps a folder's listing, with the items of the entries that did not change since the last. */
	#keep(folder: string, key: string, entries: Iterable<FileEntry>, dirty: boolean): KeptListing {
		const previous = this.#kept.get(key);
		const byName = new Map<string, ExplorerItem>();
		for (const entry of entries) {
			if (isExcludedByDefault(entry.name)) continue;
			const known = previous?.byName.get(entry.name);
			byName.set(
				entry.name,
				known !== undefined && sameEntry(known.entry, entry)
					? known
					: {
							directory: entry.type === 'directory',
							entry,
							name: entry.name,
							path: filePathChild(folder, entry.name, this.#rules),
						},
			);
		}
		const listing: KeptListing = {
			byName,
			dirty,
			folder,
			held: true,
			items: [...byName.values()].sort(this.#compare),
		};
		this.#kept.delete(key);
		this.#kept.set(key, listing);
		return listing;
	}

	/**
	 * Keeps the listings of a folder's single-child chain while each folder is the one item its parent
	 * shows, which is when the tree joins and expands it, and returns their keys. The Server loaded
	 * the whole chain, so the rest goes back at once, such as a lone `.git` the Explorer hides.
	 */
	#keepChain(listing: KeptListing, chain: ListedChain, dirty: boolean): readonly string[] {
		const keys: string[] = [];
		let parent = listing;
		for (const [index, descendant] of chain.entries()) {
			const key = filePathKey(descendant.path, this.#rules);
			const only = parent.items.length === 1 ? parent.items[0] : undefined;
			if (only === undefined || !only.directory || filePathKey(only.path, this.#rules) !== key) {
				this.#giveBack(chain.slice(index));
				break;
			}
			parent = this.#keep(descendant.path, key, descendant.entries, dirty);
			keys.push(key);
		}
		return keys;
	}

	/** Releases the folders of a chain that no kept listing holds. */
	#giveBack(chain: ListedChain): void {
		const folders: string[] = [];
		for (const { path } of chain) {
			if (this.#kept.get(filePathKey(path, this.#rules))?.held !== true) folders.push(path);
		}
		if (folders.length > 0) this.#tree.release(folders);
	}

	/**
	 * Notes for the listings running now that an entry of a folder was added or deleted: a listing of
	 * the folder may have read it before, and so may the chain of a listing above it.
	 */
	noteChange(key: string): void {
		for (const run of this.#runs) {
			if (run.key === key) run.changed = true;
			else if (isFilePathWithin(key, run.key, this.#rules)) run.changedBelow = true;
		}
	}

	/** Marks a folder's listing out of date after a change reached it. */
	invalidate(key: string): void {
		this.#invalidations.set(key, (this.#invalidations.get(key) ?? 0) + 1);
		const listing = this.#kept.get(key);
		if (listing !== undefined) listing.dirty = true;
	}

	/**
	 * Marks out of date every listing at or below `folder` that the Server keeps or is listing, and
	 * any chain listing with a folder above it, as after the root stream dropped changes there, and
	 * returns the keys of those listings. A listing it no longer keeps lists again before it counts
	 * anyway.
	 */
	invalidateWithin(folder: string): readonly string[] {
		const keys = new Set<string>();
		for (const [key, listing] of this.#kept) {
			if (listing.held && isFilePathWithin(key, folder, this.#rules)) keys.add(key);
		}
		for (const run of this.#runs) {
			const within = isFilePathWithin(run.key, folder, this.#rules);
			if (within) keys.add(run.key);
			if (within || isFilePathWithin(folder, run.key, this.#rules)) run.changedBelow = true;
		}
		for (const key of keys) this.invalidate(key);
		return [...keys];
	}

	/** Notes that the Server dropped every loaded folder, as it does when records were lost. */
	forgetHeld(): void {
		for (const listing of this.#kept.values()) listing.held = false;
		this.#evict();
	}

	/** Releases folders on the Server; their listings stay, to show while they list again. */
	release(keys: Iterable<string>): void {
		const folders: string[] = [];
		for (const key of keys) {
			const listing = this.#kept.get(key);
			if (listing === undefined || !listing.held) continue;
			listing.held = false;
			folders.push(listing.folder);
		}
		if (folders.length === 0) return;
		this.#tree.release(folders);
		this.#evict();
	}

	/** Releases every folder the Server keeps loaded, once the Explorer goes. */
	releaseAll(): void {
		this.release([...this.#kept.keys()]);
		this.#kept.clear();
		this.#invalidations.clear();
	}

	#evict(): void {
		let folders = 0;
		let items = 0;
		for (const listing of this.#kept.values()) {
			if (listing.held) continue;
			folders += 1;
			items += listing.items.length;
		}
		for (const [key, listing] of this.#kept) {
			if (folders <= KEPT_FOLDERS && items <= KEPT_ITEMS) return;
			if (listing.held) continue;
			this.#kept.delete(key);
			this.#invalidations.delete(key);
			folders -= 1;
			items -= listing.items.length;
		}
	}
}
