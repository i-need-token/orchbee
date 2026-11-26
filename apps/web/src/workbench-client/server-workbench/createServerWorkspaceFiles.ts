import type {
	WorkbenchWorkspaceFilesSource,
	WorkspaceFilesWatch,
	WorkspaceFilesWatchReport,
	WorkspaceFilesWatchScope,
	WorkspaceSearch,
	WorkspaceSearchReport,
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
	type WorkspaceSearchQuery,
	type WorkspaceSearchState,
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
/**
 * One save's budget: the Server waits twenty-one and a half seconds for a save, which is never cut
 * short once it starts writing, and the carrier needs a little more for a large text.
 */
const SAVE_MILLISECONDS = 24_000;
/**
 * One repository change's budget: the Server waits sixty-four and a half seconds for Git, which a
 * hook may keep busy, and stops it before then.
 */
const CHANGE_MILLISECONDS = 66_000;
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
		case 'write-file':
			return outcome.kind === 'written' || outcome.kind === 'conflict';
		case 'watch':
			return outcome.kind === 'watching';
		case 'search':
			return outcome.kind === 'search-results';
		case 'git-stage':
		case 'git-unstage':
		case 'git-discard':
		case 'git-commit':
			return outcome.kind === 'git-done' || outcome.kind === 'git-rejected';
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

/** The outcome a stream frame carries, or null for a frame that does not answer `requestId`. */
function outcomeOf(
	record: Uint8Array,
	requestId: string,
	workspaceId: string,
): WorkspaceFilesOutcome | null {
	let response: WorkbenchResponse;
	try {
		response = decodeWorkbenchResponse(record);
	} catch {
		return null;
	}
	if (
		response.kind !== 'workbench.workspace-files.result' ||
		response.requestId !== requestId ||
		response.workspaceId !== workspaceId
	) {
		return null;
	}
	return response.outcome;
}

/** The request opening a stream in a shown Workspace; null when it cannot be encoded. */
function streamRequest(
	session: FilesSession,
	workspaceId: string,
	requestId: string,
	operation: WorkspaceFilesOperation,
): Uint8Array | null {
	try {
		return encodeWorkbenchRequest(
			parseWorkbenchRequest({
				kind: 'workbench.workspace-files',
				operation,
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: session.snapshot.sessionId,
				workspaceId,
			}),
		);
	} catch {
		return null;
	}
}

/** Whether the Session shows a Workspace; only such a Workspace is asked about or followed. */
function shows(session: FilesSession, workspaceId: string): boolean {
	return session.snapshot.navigation.workspaces.some(
		(candidate) => candidate.workspaceId === workspaceId,
	);
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
		const outcome = outcomeOf(record, requestId, workspaceId);
		if (outcome === null) return;
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
		if (!shows(session, workspaceId)) {
			lost();
			return;
		}
		const requestId = createWorkbenchRequestId();
		const request = streamRequest(session, workspaceId, requestId, {
			directories: scope.directories,
			files: scope.files,
			kind: 'watch',
		});
		if (request === null) {
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
 * One search over a `workbench.workspace-files` Events stream. Each frame is a page of what the
 * Server found, counted from the search's first entry, so a page that does not follow the last one
 * shows that entries went missing and ends the search as `unavailable`. A page the Server answers
 * again unchanged is dropped. The stream closes once the search has ended, which frees its place in
 * the Server; a stream the Server closes first ends the search as `unavailable`.
 */
function searchWorkspace(
	connection: FilesConnection,
	session: FilesSession,
	workspaceId: string,
	query: WorkspaceSearchQuery,
	onReport: (report: WorkspaceSearchReport) => void,
): WorkspaceSearch {
	let closed = false;
	let stream: ServerSubscription | null = null;
	// Entries reported so far, which the next page starts from.
	let received = 0;

	const close = (): void => {
		closed = true;
		stream?.close();
		stream = null;
	};
	const report = (next: WorkspaceSearchReport): void => {
		if (closed) return;
		received += next.files.length;
		onReport(next);
		if (next.state !== 'searching') close();
	};
	const end = (state: WorkspaceSearchState): void => report({ files: [], state });

	const adopt = (record: Uint8Array, requestId: string): void => {
		const outcome = outcomeOf(record, requestId, workspaceId);
		if (outcome === null) return;
		if (outcome.kind !== 'search-results') {
			end('unavailable');
			return;
		}
		const { files, from, state } = outcome;
		if (from < received && from + files.length === received) return;
		if (from !== received) {
			end('unavailable');
			return;
		}
		if (files.length === 0 && state === 'searching') return;
		report({ files, state });
	};

	const requestId = createWorkbenchRequestId();
	const request = shows(session, workspaceId)
		? streamRequest(session, workspaceId, requestId, { kind: 'search', query })
		: null;
	if (request === null) {
		end('unavailable');
		return { close };
	}
	try {
		const opened = connection.subscribe('events', request, {
			onClosed: () => {
				if (stream !== opened) return;
				stream = null;
				end('unavailable');
			},
			onFrame: (_kind, record) => {
				if (stream === opened) adopt(record, requestId);
			},
			// Each page names the entry it starts from, so a page after a gap shows what was lost.
			onGap: () => undefined,
		});
		stream = opened;
	} catch {
		// The connection has ended, which its Workbench session reports.
		end('unavailable');
	}
	return { close };
}

/**
 * `workbench.workspace-files` reads, saves, repository changes, watches and searches for the right
 * bar. Nothing is read until the bar asks, and the Server starts its file service only then.
 */
export function createServerWorkspaceFiles(
	session: FilesSession,
	connection: FilesConnection,
): WorkbenchWorkspaceFilesSource {
	/** One request and its answer; null when none arrived or it does not answer the operation. */
	async function ask(
		workspaceId: string,
		operation: WorkspaceFilesOperation,
		milliseconds: number,
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
			milliseconds,
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
	}
	return {
		read(workspaceId, operation) {
			return ask(workspaceId, operation, READ_MILLISECONDS);
		},
		save(workspaceId, path, text, expected) {
			return ask(workspaceId, { expected, kind: 'write-file', path, text }, SAVE_MILLISECONDS);
		},
		changeRepository(workspaceId, change) {
			return ask(workspaceId, change, CHANGE_MILLISECONDS);
		},
		watch(workspaceId, scope, onReport) {
			return watchWorkspace(connection, session, workspaceId, scope, onReport);
		},
		search(workspaceId, query, onReport) {
			return searchWorkspace(connection, session, workspaceId, query, onReport);
		},
	};
}
