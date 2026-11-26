import type {
	WorkspaceFileListQuery,
	WorkspaceFilesOperation,
	WorkspaceFilesOutcome,
	WorkspaceFilesWatchState,
	WorkspaceReplaceFile,
	WorkspaceReplaceQuery,
	WorkspaceReplaceResult,
	WorkspaceSearchFile,
	WorkspaceSearchQuery,
	WorkspaceSearchState,
} from '@orchbee/workbench-client-protocol';

/** A change to a Workspace's repository: staging, unstaging, discarding or committing. */
export type WorkspaceGitChange = Extract<
	WorkspaceFilesOperation,
	{ readonly kind: 'git-stage' | 'git-unstage' | 'git-discard' | 'git-commit' }
>;

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

/** What a search found since its last report, and where it stands. */
export interface WorkspaceSearchReport {
	/**
	 * Entries found since the last report, in the order found. An entry with the same path as the
	 * one before it, in this report or the last, continues that file's lines.
	 */
	readonly files: readonly WorkspaceSearchFile[];
	/** Every state but `searching` ends the search; nothing is reported after it. */
	readonly state: WorkspaceSearchState;
}

/** The file paths a listing found since its last report, and where it stands. */
export interface WorkspaceFileListReport {
	/** Workspace-relative paths found since the last report, in no set order, each once. */
	readonly paths: readonly string[];
	/**
	 * Every state but `searching` ends the listing; nothing is reported after it. `limit-reached`
	 * means the Workspace holds more files than a listing names.
	 */
	readonly state: Exclude<WorkspaceSearchState, 'invalid-pattern'>;
}

/**
 * A file the window asks the right bar to open as a kept tab, and where in it, counted from 1 in
 * lines and UTF-16 columns; no line opens it where it was. Each ask has its own serial.
 */
export interface WorkspaceFileOpenRequest {
	readonly serial: number;
	readonly workspaceId: string;
	readonly path: string;
	readonly line?: number | undefined;
	readonly column?: number | undefined;
}

/** What became of one file a replacement named. */
export interface WorkspaceReplaceReport {
	readonly path: string;
	/** Null when no answer arrived, which leaves unknown whether the file was written. */
	readonly result: WorkspaceReplaceResult | null;
}

/** One search or file listing a view holds while it shows the results. */
export interface WorkspaceSearch {
	/** Stops the search or listing, if it still runs, and reports nothing more. */
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
	 * Changes a Workspace's repository. Answers `git-done`, `git-rejected` with Git's own output, or
	 * `failed`; null when no answer arrived, which leaves unknown whether the change was made.
	 */
	changeRepository(
		workspaceId: string,
		change: WorkspaceGitChange,
	): Promise<WorkspaceFilesOutcome | null>;
	/**
	 * Replaces the matches a search found in each file, in order, in as many requests as the
	 * protocol needs; a file is written only while each named match still starts there. Reports end
	 * after a request that failed, went unanswered or ran out of time, so files past the last report
	 * were not started.
	 */
	replace(
		workspaceId: string,
		query: WorkspaceReplaceQuery,
		files: readonly WorkspaceReplaceFile[],
	): Promise<readonly WorkspaceReplaceReport[]>;
	/**
	 * Follows a scope in a Workspace and reports each change, starting with the watch's state. While
	 * no answer arrives, the state reported is `unavailable`.
	 */
	watch(
		workspaceId: string,
		scope: WorkspaceFilesWatchScope,
		onReport: (report: WorkspaceFilesWatchReport) => void,
	): WorkspaceFilesWatch;
	/**
	 * Searches file contents in a Workspace and reports what it finds until the search ends. A
	 * window searches a Workspace once at a time, so a new search of it ends the one before. A
	 * search whose answers stop arriving ends as `unavailable`.
	 */
	search(
		workspaceId: string,
		query: WorkspaceSearchQuery,
		onReport: (report: WorkspaceSearchReport) => void,
	): WorkspaceSearch;
	/**
	 * Lists the paths of the files in a Workspace that a search with the same excludes and ignore
	 * choice would read, and reports them until the listing ends. A new listing of a Workspace ends
	 * the one before, but not a search; one whose answers stop arriving ends as `unavailable`.
	 */
	listFiles(
		workspaceId: string,
		query: WorkspaceFileListQuery,
		onReport: (report: WorkspaceFileListReport) => void,
	): WorkspaceSearch;
}
