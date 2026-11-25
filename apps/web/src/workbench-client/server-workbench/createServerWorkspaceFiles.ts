import type { WorkbenchWorkspaceFilesSource } from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkspaceFilesOperation,
	type WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';

import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type FilesSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>;

/** One read's budget: the Server's own five and a half seconds and the carrier's way back. */
const READ_MILLISECONDS = 7_000;

/**
 * `workbench.workspace-files` reads for the right bar. Nothing is read until the bar asks, and the
 * Server starts its file service only then.
 */
export function createServerWorkspaceFiles(session: FilesSession): WorkbenchWorkspaceFilesSource {
	return {
		async read(
			workspaceId: string,
			operation: WorkspaceFilesOperation,
		): Promise<WorkspaceFilesOutcome | null> {
			// Only a Workspace the Session shows is asked about; its own identity names it.
			const workspace = session.snapshot.navigation.workspaces.find(
				(candidate) => candidate.workspaceId === workspaceId,
			);
			if (workspace === undefined) return { kind: 'failed', reason: 'not-found' };
			const requestId = createWorkbenchRequestId();
			const answer = await session.ask(
				{
					kind: 'workbench.workspace-files',
					operation,
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: session.snapshot.sessionId,
					workspaceId: workspace.workspaceId,
				},
				READ_MILLISECONDS,
			);
			if (
				answer.kind !== 'answered' ||
				answer.response.kind !== 'workbench.workspace-files.result' ||
				answer.response.requestId !== requestId
			) {
				return null;
			}
			return answer.response.outcome;
		},
	};
}
