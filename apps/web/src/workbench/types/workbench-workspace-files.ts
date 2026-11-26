import type {
	WorkspaceFilesOperation,
	WorkspaceFilesOutcome,
	WorkspaceFilesWatchState,
} from '@orchbee/workbench-client-protocol';

/** What a watch follows in one Workspace: the directories shown and the files open. */
export interface WorkspaceFilesWatchScope {
	/** Workspace-relative directories whose entries show; `''` is the Workspace itself. */
	readonly directories: readonly string[];
	/** Workspace-relative files open in the working tree. */
	readonly files: readonly string[];
}

/** What a watch learned since its last report. */
export interface WorkspaceFilesWatchReport {
	/** A shown directory's entries, or an open file, may have changed. */
	readonly files: boolean;
	/** The repository's status may have changed. */
	readonly repository: boolean;
	/** `unavailable` follows nothing, so only a refresh the user asks for reads again. */
	readonly state: WorkspaceFilesWatchState;
}

/** One watch a view holds while it shows a Workspace's files. */
export interface WorkspaceFilesWatch {
	/** Follows another scope; the old one is followed until the new one is. */
	update(scope: WorkspaceFilesWatchScope): void;
	close(): void;
}

/**
 * File and Git facts for the right bar, read one at a time on demand and followed while shown, and
 * the saving of a file it edits. A Session that cannot serve them offers no source, and the window
 * offers no right bar.
 */
export interface WorkbenchWorkspaceFilesSource {
	/** One read in a Workspace; null when no answer arrived (a lost connection or a stale Session). */
	read(
		workspaceId: string,
		operation: WorkspaceFilesOperation,
	): Promise<WorkspaceFilesOutcome | null>;
	/**
	 * Saves a working-tree file's text, only while the file still hashes to `expected`, or, for
	 * null, only while there is no file there yet. Answers `written`, `conflict` or `failed`; null
	 * when no answer arrived, which leaves unknown whether the file was written.
	 */
	save(
		workspaceId: string,
		path: string,
		text: string,
		expected: string | null,
	): Promise<WorkspaceFilesOutcome | null>;
	/**
	 * Follows a scope in a Workspace and reports each change, starting with the watch's state. While
	 * no answer arrives, the state reported is `unavailable`.
	 */
	watch(
		workspaceId: string,
		scope: WorkspaceFilesWatchScope,
		onReport: (report: WorkspaceFilesWatchReport) => void,
	): WorkspaceFilesWatch;
}
