import type {
	WorkspaceFilesOperation,
	WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource } from '../workbench';

/** One read whose lost answer or rejected request reads as unavailable. */
export async function readWorkspaceFiles(
	source: WorkbenchWorkspaceFilesSource,
	workspaceId: string,
	operation: WorkspaceFilesOperation,
): Promise<WorkspaceFilesOutcome> {
	try {
		return (await source.read(workspaceId, operation)) ?? { kind: 'failed', reason: 'unavailable' };
	} catch {
		return { kind: 'failed', reason: 'unavailable' };
	}
}
