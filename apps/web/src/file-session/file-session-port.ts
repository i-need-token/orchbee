import type { FileContentPort } from './file-content-port';
import type { FileGrantPort } from './file-grant-port';
import type { FilePathRules } from './file-resource';
import type { FileTreePort } from './file-tree-port';
import type { GitPort } from './git-port';

/** The File Bar capabilities a machine's Server negotiated, one per batch. */
export type FileCapability = 'files' | 'files-edit' | 'files-history' | 'files-search';

/** The Workspace's root: its current directory, which a missing directory keeps until it returns. */
export type FileRoot =
	| { readonly path: string; readonly state: 'missing' | 'present' }
	| { readonly state: 'none' };

/** How fully the root is watched: degraded watches loaded folders only; stopped watches nothing. */
type FileWatchState = 'degraded' | 'full' | 'stopped';

/** One coalesced change to a loaded folder or an opened file. */
export interface FileChange {
	readonly directory: boolean;
	readonly path: string;
	readonly type: 'added' | 'changed' | 'deleted';
}

/** One record of the Workspace's root stream, in sequence order. */
export type FileRootEvent =
	/**
	 * Records were lost, as across a reconnect: the Client lists its loaded folders again, and a
	 * snapshot follows a reconnect.
	 */
	| { readonly kind: 'gap' }
	| {
			readonly kind: 'snapshot';
			readonly root: FileRoot;
			readonly rules: FilePathRules;
			readonly sequence: number;
	  }
	| {
			readonly changes: readonly FileChange[];
			readonly kind: 'update';
			/**
			 * Folders at or below which changes were dropped, as an overflow drops them: the Client
			 * lists again every folder it loaded at or below each, and reads again each file it
			 * opened below one.
			 */
			readonly rescan: readonly string[];
			/** The root's new value, when it moved or appeared or went missing. */
			readonly root: FileRoot | null;
			readonly sequence: number;
			readonly watch: FileWatchState | null;
	  };

/**
 * One Workspace's file session in this Client. Every request in it names that Workspace; files
 * and Git run on the machine that holds its Directory.
 */
export interface FileWorkspaceSession {
	readonly capabilities: ReadonlySet<FileCapability>;
	readonly content: FileContentPort;
	readonly git: GitPort;
	readonly grants: FileGrantPort;
	/** The machine that holds the Workspace's Directory, as the Server names it. */
	readonly machine: string;
	readonly tree: FileTreePort;
	readonly workspaceId: string;
	/** Ends the session's streams, cursors and grants; its pending requests settle `cancelled`. */
	close(): void;
	/** Follows the root stream: a snapshot first, then updates for loaded folders and opened files. */
	subscribe(listener: (event: FileRootEvent) => void): () => void;
}

/** Where the Client's file sessions come from: one per Workspace, opened on first need. */
export interface FileSessionPort {
	/** Opens a session; the backend starts nothing until its first request or subscription. */
	open(workspaceId: string): FileWorkspaceSession;
}
