import type {
	WorkspaceFilesOperation,
	WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';

/**
 * Read-only file and Git facts for the right bar, read one at a time on demand. A Session that cannot
 * serve them offers no source, and the window offers no right bar.
 */
export interface WorkbenchWorkspaceFilesSource {
	/** One read in a Workspace; null when no answer arrived (a lost connection or a stale Session). */
	read(
		workspaceId: string,
		operation: WorkspaceFilesOperation,
	): Promise<WorkspaceFilesOutcome | null>;
}
