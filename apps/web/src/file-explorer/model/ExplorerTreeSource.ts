import type { OrchbeeLocale } from '@orchbee/i18n';

import {
	type FileEntry,
	type FilePathRules,
	type FileTreePort,
	filePathChild,
} from '../../file-session';
import type { VirtualTreeSource } from '../../virtual-tree';
import { ExplorerListError } from './ExplorerListError';
import type { ExplorerItem } from './explorer-item';
import { explorerItemComparer } from './explorerItemComparer';
import { isExcludedByDefault } from './isExcludedByDefault';

interface ExplorerTreeSourceOptions {
	readonly locale: OrchbeeLocale;
	readonly rules: FilePathRules;
	readonly tree: FileTreePort;
}

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
 * The Explorer's folders as the virtual tree reads them: each listing gathers every page of its
 * folder, hides what `files.exclude` hides, and sorts as VS Code's Explorer does. An entry that did
 * not change keeps its item, so its row draws nothing again; a later page's entry replaces an
 * earlier one of the same name, since a folder that changes between pages may repeat entries.
 */
export class ExplorerTreeSource implements VirtualTreeSource<string, ExplorerItem> {
	readonly key = (item: ExplorerItem): string => item.path;
	// A name that is not valid UTF-8 cannot be named in a listing, so its folder never opens.
	readonly hasChildren = (item: ExplorerItem): boolean =>
		item.directory && item.entry?.unrepresentable !== true;
	readonly children = (item: ExplorerItem, signal: AbortSignal): Promise<readonly ExplorerItem[]> =>
		this.#list(item.path, signal);

	readonly #compare: (one: ExplorerItem, other: ExplorerItem) => number;
	/** Each folder's children by name, as its latest listing reported them. */
	readonly #listed = new Map<string, ReadonlyMap<string, ExplorerItem>>();
	readonly #rules: FilePathRules;
	readonly #tree: FileTreePort;

	constructor(options: ExplorerTreeSourceOptions) {
		this.#compare = explorerItemComparer(options.locale);
		this.#rules = options.rules;
		this.#tree = options.tree;
	}

	/** Folders listed so far, which the Server keeps loaded until they are released. */
	listedFolders(): readonly string[] {
		return [...this.#listed.keys()];
	}

	/** Whether `folder` was listed and, given a name, whether its latest listing showed that entry. */
	shows(folder: string, name?: string): boolean {
		const children = this.#listed.get(folder);
		if (children === undefined) return false;
		return name === undefined || children.has(name);
	}

	async #list(folder: string, signal: AbortSignal): Promise<readonly ExplorerItem[]> {
		const entries = new Map<string, FileEntry>();
		let cursor: string | undefined;
		do {
			const page = await this.#tree.list({ cursor, path: folder }, signal);
			if (page.kind === 'error') {
				if (page.error.code === 'cancelled')
					throw new DOMException('Listing cancelled.', 'AbortError');
				throw new ExplorerListError(folder, page.error);
			}
			for (const entry of page.value.entries) entries.set(entry.name, entry);
			cursor = page.value.cursor ?? undefined;
		} while (cursor !== undefined);
		const previous = this.#listed.get(folder);
		const children = new Map<string, ExplorerItem>();
		for (const entry of entries.values()) {
			if (isExcludedByDefault(entry.name)) continue;
			const known = previous?.get(entry.name);
			children.set(
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
		this.#listed.set(folder, children);
		return [...children.values()].sort(this.#compare);
	}
}
