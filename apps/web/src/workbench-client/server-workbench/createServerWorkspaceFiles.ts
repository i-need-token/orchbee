import type {
	WorkbenchWorkspaceFilesSource,
	WorkspaceFileListReport,
	WorkspaceFilesWatch,
	WorkspaceFilesWatchReport,
	WorkspaceFilesWatchScope,
	WorkspaceReplaceReport,
	WorkspaceSearch,
	WorkspaceSearchReport,
} from '../../workbench';
import {
	MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES,
	MAXIMUM_WORKSPACE_FILES_WATCHED_FILES,
	MAXIMUM_WORKSPACE_LISTED_FILES,
	MAXIMUM_WORKSPACE_REPLACE_FILES,
	MAXIMUM_WORKSPACE_REPLACE_MATCHES,
	WORKBENCH_PROTOCOL_VERSION,
	WORKSPACE_FILES_SEARCH_REQUEST_BYTES,
	parseWorkbenchRequest,
	type WorkbenchResponse,
	type WorkspaceFileListQuery,
	type WorkspaceFilesOperation,
	type WorkspaceFilesOutcome,
	type WorkspaceFilesWatchState,
	type WorkspaceReplaceFile,
	type WorkspaceReplaceQuery,
	type WorkspaceSearchQuery,
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
 * hook may keep busy, and stops it before then. A replacement waits as long, in the same lane.
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
		case 'list-files':
			return outcome.kind === 'file-list';
		case 'replace-matches':
			// Each file's result is read by its place, so only the files asked about, in order, answer.
			return (
				outcome.kind === 'replaced' &&
				outcome.files.length === operation.files.length &&
				outcome.files.every((file, index) => file.path === operation.files[index]?.path)
			);
		case 'git-stage':
		case 'git-unstage':
		case 'git-discard':
		case 'git-commit':
			return outcome.kind === 'git-done' || outcome.kind === 'git-rejected';
	}
}

const encoder = new TextEncoder();

/** Room a replacement's request keeps for its envelope beside the query and the files. */
const REPLACE_ENVELOPE_BYTES = 4096;

/**
 * The files of a replacement in requests the protocol carries, in order: each within the limits
 * on files, matches and bytes. A file is never split, because its later matches would be looked
 * for in what the earlier ones already changed.
 */
function replaceBatches(
	query: WorkspaceReplaceQuery,
	files: readonly WorkspaceReplaceFile[],
): readonly (readonly WorkspaceReplaceFile[])[] {
	const room =
		WORKSPACE_FILES_SEARCH_REQUEST_BYTES -
		REPLACE_ENVELOPE_BYTES -
		encoder.encode(JSON.stringify(query)).length;
	const batches: WorkspaceReplaceFile[][] = [];
	let batch: WorkspaceReplaceFile[] = [];
	let matches = 0;
	let bytes = 0;
	for (const file of files) {
		const size = encoder.encode(JSON.stringify(file)).length + 1;
		if (
			batch.length > 0 &&
			(batch.length === MAXIMUM_WORKSPACE_REPLACE_FILES ||
				matches + file.matches.length > MAXIMUM_WORKSPACE_REPLACE_MATCHES ||
				bytes + size > room)
		) {
			batches.push(batch);
			batch = [];
			matches = 0;
			bytes = 0;
		}
		batch.push(file);
		matches += file.matches.length;
		bytes += size;
	}
	if (batch.length > 0) batches.push(batch);
	return batches;
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

/** One page of a paged stream: the entries it carries, counted from the stream's first, and its state. */
interface StreamPage<Entry, State extends string> {
	readonly entries: readonly Entry[];
	readonly from: number;
	readonly state: State;
}

/**
 * One search or listing over a `workbench.workspace-files` Events stream. Each frame is a page of
 * what the Server found, counted from the first entry, so a page that does not follow the last one
 * shows that entries went missing and ends the stream as `unavailable`, as does a frame of another
 * kind or, past `limit` entries, one with more. A page the Server answers again unchanged is
 * dropped. The stream closes once its state is final, which frees its place in the Server; a
 * stream the Server closes first ends as `unavailable`.
 */
function streamPages<Entry, State extends string>(
	connection: FilesConnection,
	session: FilesSession,
	workspaceId: string,
	operation: Extract<WorkspaceFilesOperation, { readonly kind: 'search' | 'list-files' }>,
	limit: number,
	pageOf: (outcome: WorkspaceFilesOutcome) => StreamPage<Entry, State> | null,
	onPage: (entries: readonly Entry[], state: State | 'unavailable') => void,
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
	const report = (entries: readonly Entry[], state: State | 'unavailable'): void => {
		if (closed) return;
		received += entries.length;
		onPage(entries, state);
		if (state !== 'searching') close();
	};
	const end = (): void => report([], 'unavailable');

	const adopt = (record: Uint8Array, requestId: string): void => {
		const outcome = outcomeOf(record, requestId, workspaceId);
		if (outcome === null) return;
		const page = pageOf(outcome);
		if (page === null) {
			end();
			return;
		}
		const { entries, from, state } = page;
		if (from < received && from + entries.length === received) return;
		if (from !== received || received + entries.length > limit) {
			end();
			return;
		}
		if (entries.length === 0 && state === 'searching') return;
		report(entries, state);
	};

	const requestId = createWorkbenchRequestId();
	const request = shows(session, workspaceId)
		? streamRequest(session, workspaceId, requestId, operation)
		: null;
	if (request === null) {
		end();
		return { close };
	}
	try {
		const opened = connection.subscribe('events', request, {
			onClosed: () => {
				if (stream !== opened) return;
				stream = null;
				end();
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
		end();
	}
	return { close };
}

/** A search of file contents, whose entries carry each file's matching lines. */
function searchWorkspace(
	connection: FilesConnection,
	session: FilesSession,
	workspaceId: string,
	query: WorkspaceSearchQuery,
	onReport: (report: WorkspaceSearchReport) => void,
): WorkspaceSearch {
	return streamPages(
		connection,
		session,
		workspaceId,
		{ kind: 'search', query },
		Number.POSITIVE_INFINITY,
		(outcome) =>
			outcome.kind === 'search-results'
				? { entries: outcome.files, from: outcome.from, state: outcome.state }
				: null,
		(files, state) => onReport({ files, state }),
	);
}

/** A listing of the files a search would read, which never names more than the Server lists. */
function listWorkspaceFiles(
	connection: FilesConnection,
	session: FilesSession,
	workspaceId: string,
	query: WorkspaceFileListQuery,
	onReport: (report: WorkspaceFileListReport) => void,
): WorkspaceSearch {
	return streamPages(
		connection,
		session,
		workspaceId,
		{ kind: 'list-files', query },
		MAXIMUM_WORKSPACE_LISTED_FILES,
		(outcome) =>
			outcome.kind === 'file-list'
				? { entries: outcome.paths, from: outcome.from, state: outcome.state }
				: null,
		(paths, state) => onReport({ paths, state }),
	);
}

/**
 * `workbench.workspace-files` reads, saves, replacements, repository changes, watches, searches and
 * file listings for the right bar. Nothing is read until the bar asks, and the Server starts its file service only then.
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
		async replace(workspaceId, query, files) {
			const reports: WorkspaceReplaceReport[] = [];
			for (const batch of replaceBatches(query, files)) {
				const outcome = await ask(
					workspaceId,
					{ files: batch, kind: 'replace-matches', query },
					CHANGE_MILLISECONDS,
				);
				if (outcome?.kind !== 'replaced') {
					// Nothing after a batch that failed, or whose answer never came, is sent.
					const result = outcome?.kind === 'failed' ? outcome : null;
					for (const file of batch) reports.push({ path: file.path, result });
					break;
				}
				for (const file of outcome.files) reports.push(file);
				// A batch the Server could not finish in time leaves the rest for later.
				const late = outcome.files.some(
					(file) => file.result.kind === 'failed' && file.result.reason === 'timeout',
				);
				if (late) break;
			}
			return reports;
		},
		watch(workspaceId, scope, onReport) {
			return watchWorkspace(connection, session, workspaceId, scope, onReport);
		},
		search(workspaceId, query, onReport) {
			return searchWorkspace(connection, session, workspaceId, query, onReport);
		},
		listFiles(workspaceId, query, onReport) {
			return listWorkspaceFiles(connection, session, workspaceId, query, onReport);
		},
	};
}
