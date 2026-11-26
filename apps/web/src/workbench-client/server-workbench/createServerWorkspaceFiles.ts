import type {
	WorkbenchWorkspaceFilesSource,
	WorkspaceFilesWatch,
	WorkspaceFilesWatchReport,
	WorkspaceFilesWatchScope,
} from '../../workbench';
import {
	MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES,
	MAXIMUM_WORKSPACE_FILES_WATCHED_FILES,
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchResponse,
	type WorkspaceFilesOperation,
	type WorkspaceFilesOutcome,
	type WorkspaceFilesWatchState,
} from '@orchbee/workbench-client-protocol';

import {
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	type ServerConnection,
	type ServerSubscription,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type FilesSession = Pick<ServerWorkbenchSession, 'ask' | 'snapshot'>;
type FilesConnection = Pick<ServerConnection, 'subscribe'>;

/** One read's budget: the Server's own five and a half seconds and the carrier's way back. */
const READ_MILLISECONDS = 7_000;
/** A watch stream the Server closed opens again after this long. */
const REOPEN_MILLISECONDS = 2000;

/** Whether an outcome is one the operation can have: its kind, and a chunk's offset and encoding. */
function answers(outcome: WorkspaceFilesOutcome, operation: WorkspaceFilesOperation): boolean {
	if (outcome.kind === 'failed') return true;
	switch (operation.kind) {
		case 'list-directory':
			return outcome.kind === 'directory';
		case 'git-status':
			return outcome.kind === 'git-status';
		case 'read-file': {
			if (outcome.kind !== 'file') return false;
			const { content } = outcome;
			if (content.kind === 'text' || content.kind === 'bytes') {
				return content.kind === operation.encoding && content.offset === operation.offset;
			}
			return true;
		}
		case 'watch':
			return outcome.kind === 'watching';
	}
}

/** The scope a request can carry: each path once, the first ones within the protocol's limits. */
function bounded(scope: WorkspaceFilesWatchScope): WorkspaceFilesWatchScope {
	return {
		directories: [...new Set(scope.directories)].slice(
			0,
			MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES,
		),
		files: [...new Set(scope.files)].slice(0, MAXIMUM_WORKSPACE_FILES_WATCHED_FILES),
	};
}

function sameScope(left: WorkspaceFilesWatchScope, right: WorkspaceFilesWatchScope): boolean {
	const same = (a: readonly string[], b: readonly string[]): boolean =>
		a.length === b.length && a.every((path, index) => path === b[index]);
	return same(left.directories, right.directories) && same(left.files, right.files);
}

interface Followed {
	readonly files: string;
	readonly repository: string;
	readonly state: WorkspaceFilesWatchState;
}

/**
 * One watch over a `workbench.workspace-files` State stream. Each frame carries the Workspace's
 * revisions, and a revision that moved since the last frame is a change. A new scope opens its stream
 * before the old one closes, so nothing goes unfollowed between them; frames from an older stream
 * are dropped. A stream the Server closes opens again, and reports `unavailable` meanwhile.
 */
function watchWorkspace(
	connection: FilesConnection,
	session: FilesSession,
	workspaceId: string,
	initial: WorkspaceFilesWatchScope,
	onReport: (report: WorkspaceFilesWatchReport) => void,
): WorkspaceFilesWatch {
	let scope = bounded(initial);
	let closed = false;
	let stream: ServerSubscription | null = null;
	let reopen: ReturnType<typeof setTimeout> | null = null;
	// What the last frame said; revisions from a reopened stream are compared with it too.
	let followed: Followed | null = null;
	let reported: WorkspaceFilesWatchState | null = null;

	const report = (next: WorkspaceFilesWatchReport): void => {
		if (closed) return;
		if (!next.files && !next.repository && next.state === reported) return;
		reported = next.state;
		onReport(next);
	};
	const lost = (): void => report({ files: false, repository: false, state: 'unavailable' });

	const adopt = (record: Uint8Array, requestId: string): void => {
		let response: WorkbenchResponse;
		try {
			response = decodeWorkbenchResponse(record);
		} catch {
			return;
		}
		if (
			response.kind !== 'workbench.workspace-files.result' ||
			response.requestId !== requestId ||
			response.workspaceId !== workspaceId
		) {
			return;
		}
		const { outcome } = response;
		if (outcome.kind !== 'watching') {
			lost();
			return;
		}
		const previous = followed;
		followed = { files: outcome.files, repository: outcome.repository, state: outcome.state };
		report({
			files: previous !== null && previous.files !== outcome.files,
			repository: previous !== null && previous.repository !== outcome.repository,
			state: outcome.state,
		});
	};

	const open = (): void => {
		if (closed) return;
		// Only a Workspace the Session shows is followed; its own identity names it.
		const shown = session.snapshot.navigation.workspaces.some(
			(candidate) => candidate.workspaceId === workspaceId,
		);
		if (!shown) {
			lost();
			return;
		}
		const requestId = createWorkbenchRequestId();
		let request: Uint8Array;
		try {
			request = encodeWorkbenchRequest(
				parseWorkbenchRequest({
					kind: 'workbench.workspace-files',
					operation: { directories: scope.directories, files: scope.files, kind: 'watch' },
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: session.snapshot.sessionId,
					workspaceId,
				}),
			);
		} catch {
			lost();
			return;
		}
		const previous = stream;
		try {
			const opened = connection.subscribe('state', request, {
				onClosed: () => {
					if (stream !== opened) return;
					stream = null;
					lost();
					if (closed) return;
					reopen = setTimeout(() => {
						reopen = null;
						open();
					}, REOPEN_MILLISECONDS);
				},
				onFrame: (_kind, record) => {
					if (stream === opened) adopt(record, requestId);
				},
				// Every frame is the whole state, so a gap loses nothing.
				onGap: () => undefined,
			});
			stream = opened;
		} catch {
			// The connection has ended, which its Workbench session reports.
			stream = null;
			lost();
		}
		previous?.close();
	};

	open();
	return {
		update(next) {
			const nextScope = bounded(next);
			if (closed || sameScope(scope, nextScope)) return;
			scope = nextScope;
			// A stream waiting to reopen takes the new scope when it does.
			if (reopen === null) open();
		},
		close() {
			closed = true;
			if (reopen !== null) clearTimeout(reopen);
			reopen = null;
			stream?.close();
			stream = null;
		},
	};
}

/**
 * `workbench.workspace-files` reads and watches for the right bar. Nothing is read until the bar
 * asks, and the Server starts its file service only then.
 */
export function createServerWorkspaceFiles(
	session: FilesSession,
	connection: FilesConnection,
): WorkbenchWorkspaceFilesSource {
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
				answer.response.requestId !== requestId ||
				!answers(answer.response.outcome, operation)
			) {
				return null;
			}
			return answer.response.outcome;
		},
		watch(workspaceId, scope, onReport) {
			return watchWorkspace(connection, session, workspaceId, scope, onReport);
		},
	};
}
