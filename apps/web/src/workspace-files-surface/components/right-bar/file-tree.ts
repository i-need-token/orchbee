import type {
	WorkspaceDirectoryEntry,
	WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';

type Failure = Extract<WorkspaceFilesOutcome, { kind: 'failed' }>['reason'];

/** One folder's listing as far as it has been read. */
export type Listing =
	| { readonly state: 'loading' }
	| {
			readonly state: 'loaded';
			readonly entries: readonly WorkspaceDirectoryEntry[];
			readonly truncated: boolean;
	  }
	| { readonly state: 'failed'; readonly reason: Failure };

/** One visible row of the tree: an entry, or a note in place of a folder's entries. */
export type TreeRow =
	| {
			readonly kind: 'entry';
			readonly key: string;
			readonly path: string;
			readonly depth: number;
			readonly entry: WorkspaceDirectoryEntry;
			readonly expanded: boolean;
	  }
	| {
			readonly kind: 'note';
			readonly key: string;
			readonly depth: number;
			readonly note: 'loading' | 'failed' | 'empty' | 'truncated';
			readonly count: number;
	  };

function childPath(folder: string, name: string): string {
	return folder === '' ? name : `${folder}/${name}`;
}

/** The parent folder of a Workspace-relative path; `''` for the Workspace itself. */
export function parentPath(path: string): string {
	const slash = path.lastIndexOf('/');
	return slash < 0 ? '' : path.slice(0, slash);
}

/** The rows an open tree shows, walking only into expanded folders. */
export function flattenTree(
	listings: ReadonlyMap<string, Listing>,
	expanded: ReadonlySet<string>,
): TreeRow[] {
	const rows: TreeRow[] = [];
	const walk = (folder: string, depth: number): void => {
		const listing = listings.get(folder);
		if (listing === undefined || listing.state === 'loading') {
			rows.push({ count: 0, depth, key: `${folder}\0loading`, kind: 'note', note: 'loading' });
			return;
		}
		if (listing.state === 'failed') {
			rows.push({ count: 0, depth, key: `${folder}\0failed`, kind: 'note', note: 'failed' });
			return;
		}
		if (listing.entries.length === 0) {
			rows.push({ count: 0, depth, key: `${folder}\0empty`, kind: 'note', note: 'empty' });
			return;
		}
		for (const entry of listing.entries) {
			const path = childPath(folder, entry.name);
			const open = entry.entryKind === 'directory' && expanded.has(path);
			rows.push({ depth, entry, expanded: open, key: path, kind: 'entry', path });
			if (open) walk(path, depth + 1);
		}
		if (listing.truncated) {
			rows.push({
				count: listing.entries.length,
				depth,
				key: `${folder}\0truncated`,
				kind: 'note',
				note: 'truncated',
			});
		}
	};
	walk('', 0);
	return rows;
}
