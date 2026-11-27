import type { FileError, FileResult } from './file-error';

/** What a listed path is; for a link, what its target is. */
type FileEntryType = 'directory' | 'file' | 'unknown';

/** One entry, as listing and stat report it. */
export interface FileEntry {
	readonly mtimeMs: number;
	readonly name: string;
	readonly readonly: boolean;
	readonly size: number;
	/** A link, junction or other name surrogate; `type` describes its target. */
	readonly symlink: boolean;
	readonly type: FileEntryType;
	/** A name that is not valid UTF-8: it is listed, but no operation can name it. */
	readonly unrepresentable: boolean;
}

interface FileListRequest {
	/** The page to continue from; absent for the first one. */
	readonly cursor?: string | undefined;
	readonly path: string;
	/** Also list the chain of single-child folders below `path`, up to 64 levels. */
	readonly singleChild?: boolean | undefined;
}

/** A folder and its entries. */
interface FileListing {
	readonly entries: readonly FileEntry[];
	readonly path: string;
}

/** One page of a folder's entries, which arrive unsorted. */
export interface FileListPage {
	/**
	 * Present while more pages follow. A folder that changes between pages may repeat or miss
	 * entries, which the root stream then reports.
	 */
	readonly cursor: string | null;
	/** With `singleChild`, each folder of the single-child chain below the listed one. */
	readonly descendants: readonly FileListing[];
	readonly entries: readonly FileEntry[];
}

/** One path's stat: its entry, or why it has none. */
export type FileStatItem =
	| { readonly entry: FileEntry; readonly kind: 'entry'; readonly path: string }
	| { readonly error: FileError; readonly kind: 'error'; readonly path: string };

/**
 * Browsing a Workspace's files. Listing a folder loads it, so the root stream reports its changes
 * until it is released.
 */
export interface FileTreePort {
	list(request: FileListRequest, signal: AbortSignal): Promise<FileResult<FileListPage>>;
	/** Unloads folders, so the root stream stops reporting their changes. */
	release(paths: readonly string[]): void;
	/** Stats each path on its own, such as a terminal file link before it opens. */
	stat(paths: readonly string[], signal: AbortSignal): Promise<FileResult<readonly FileStatItem[]>>;
}
