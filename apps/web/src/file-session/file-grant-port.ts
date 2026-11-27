import type { FileResult } from './file-error';
import type { FileEntry } from './file-tree-port';

/** The explicit gesture that opened a file, which is the only way past the root. */
export type FileGrantSource =
	| 'diff'
	| 'os-open'
	| 'restored'
	| 'save-as'
	| 'terminal-link'
	| 'tree';

/**
 * The Workspace's boundary beyond its root: opened files and consented parent repositories. The
 * Client asks only on a person's gesture; the backend enforces the boundary and answers a request
 * outside it with `outside-scope`.
 */
export interface FileGrantPort {
	/** Closes a consented repository, which leaves the boundary. */
	closeRepository(repository: string, signal: AbortSignal): Promise<FileResult<null>>;
	/** Admits a parent repository's working tree after the person answered Yes or Always. */
	consentRepository(repository: string, signal: AbortSignal): Promise<FileResult<null>>;
	/** Admits one file as opened: it can then be read, stated and watched wherever it is. */
	grant(path: string, source: FileGrantSource, signal: AbortSignal): Promise<FileResult<FileEntry>>;
	ungrant(paths: readonly string[]): void;
}
