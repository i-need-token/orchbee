import type { FileError, FileResult } from '../file-error';
import type {
	GitMutation,
	GitMutationOutput,
	GitPort,
	GitPrompt,
	GitPromptAnswer,
	GitRef,
	GitRelayEvent,
	GitRemote,
	GitRequestOutcome,
	GitStatusEntry,
	GitSummary,
	GitSummaryEvent,
} from '../git-port';
import type { MemoryAnswering } from './MemoryAnswering';
import { memoryVersion, readMemoryContent, type MemoryContent } from './memory-content';
import type { MemorySessionState, MemoryStream } from './memory-session-state';

/** A change that ran, in the order it ran. */
interface MemoryGitRun {
	readonly mutation: GitMutation;
	readonly requestId: string;
	readonly workspaceId: string;
}

/** An answer the relay passed to Git. */
interface MemoryGitAnswer {
	readonly answer: GitPromptAnswer;
	readonly promptId: string;
	readonly workspaceId: string;
}

/** Decides what a change returns. It runs at most once per request ID. */
type MemoryGitHandler = (
	mutation: GitMutation,
	workspaceId: string,
) => FileResult<GitMutationOutput>;

interface MemoryGitStatus {
	readonly entries: readonly GitStatusEntry[];
	readonly revision: number;
}

interface MemoryStatusCursor {
	readonly offset: number;
	readonly repository: string;
	readonly revision: number;
	readonly session: MemorySessionState;
}

interface MemoryRecordedRun {
	readonly promise: Promise<FileResult<GitMutationOutput>>;
	result: FileResult<GitMutationOutput> | null;
}

interface MemoryGitUser {
	readonly email: string | null;
	readonly name: string | null;
}

interface MemoryGitOptions {
	readonly answering: MemoryAnswering;
	readonly pageSize: number;
	/** The open sessions of a Workspace, which its streams reach. */
	readonly sessionsOf: (workspaceId: string) => readonly MemorySessionState[];
}

const NO_REPOSITORIES: GitSummary = {
	candidates: [],
	git: { state: 'ok', version: '2.47.1' },
	repositories: [],
	repositoriesTruncated: false,
	unsafe: [],
};
const COMMIT_DIGITS = 40;

function failure<Value>(code: FileError['code']): FileResult<Value> {
	return { error: { code }, kind: 'error' };
}

function ok<Value>(value: Value): FileResult<Value> {
	return { kind: 'ok', value };
}

function showKey(repository: string, ref: string, path: string): string {
	return `${repository}\u0000${ref}\u0000${path}`;
}

/** Follows a running change for a repeated request, which its own signal can still cancel. */
function followRun<Value>(
	promise: Promise<FileResult<Value>>,
	signal: AbortSignal,
): Promise<FileResult<Value>> {
	if (signal.aborted) return Promise.resolve(failure('cancelled'));
	return new Promise((resolve) => {
		const cancel = (): void => resolve(failure('cancelled'));
		signal.addEventListener('abort', cancel, { once: true });
		void promise.then((result) => {
			signal.removeEventListener('abort', cancel);
			resolve(result);
		});
	});
}

/**
 * Source Control for the in-memory session. A test sets what Git reports, scripts what changes
 * return and reads which changes ran. As the backend records outcomes, a change runs once per
 * request ID; a request the link lost before it ran stays unknown.
 */
export class MemoryGit {
	private readonly answering: MemoryAnswering;
	private readonly answered: MemoryGitAnswer[] = [];
	private commits = 0;
	private handler: MemoryGitHandler;
	private readonly ignored = new Map<string, ReadonlySet<string>>();
	private nextCursor = 1;
	private readonly outputs = new Map<string, string>();
	private readonly pageSize: number;
	private readonly prompts = new Map<string, Map<string, GitPrompt>>();
	private readonly ran: MemoryGitRun[] = [];
	private readonly recorded = new Map<string, Map<string, MemoryRecordedRun>>();
	private readonly references = new Map<
		string,
		{ readonly refs: readonly GitRef[]; readonly remotes: readonly GitRemote[] }
	>();
	private readonly sessionsOf: (workspaceId: string) => readonly MemorySessionState[];
	private readonly shows = new Map<string, MemoryContent>();
	private readonly statusCursors = new Map<string, MemoryStatusCursor>();
	private readonly statuses = new Map<string, MemoryGitStatus>();
	private readonly summaries = new Map<string, GitSummary>();
	private readonly users = new Map<string, MemoryGitUser>();

	public constructor(options: MemoryGitOptions) {
		this.answering = options.answering;
		this.pageSize = options.pageSize;
		this.sessionsOf = options.sessionsOf;
		this.handler = (mutation) => this.defaultOutput(mutation);
	}

	/** The changes that ran, in order; a repeated request ID adds nothing. */
	public runs(): readonly MemoryGitRun[] {
		return [...this.ran];
	}

	/** The answers the relay passed to Git, in order. */
	public answers(): readonly MemoryGitAnswer[] {
		return [...this.answered];
	}

	/** Scripts what changes return from now on. */
	public handle(handler: MemoryGitHandler): void {
		this.handler = handler;
	}

	/** Publishes a Workspace's summary to its Source Control streams. */
	public setSummary(workspaceId: string, summary: GitSummary): void {
		this.summaries.set(workspaceId, summary);
		for (const session of this.sessionsOf(workspaceId)) {
			for (const stream of session.summaryStreams) {
				if (stream.ready) this.sendSummary(stream, workspaceId);
			}
		}
	}

	/** Sets a repository's status; a page asked at an older revision then fails `stale-revision`. */
	public setStatus(repository: string, revision: number, entries: readonly GitStatusEntry[]): void {
		this.statuses.set(repository, { entries, revision });
	}

	/** Sets a path's content at a reference: `HEAD`, the index or a commit. */
	public setShow(
		repository: string,
		ref: string,
		path: string,
		content: string | Uint8Array,
	): void {
		this.shows.set(showKey(repository, ref, path), {
			bom: false,
			content,
			encoding: null,
			mtimeMs: 0,
		});
	}

	public setIgnored(repository: string, paths: readonly string[]): void {
		this.ignored.set(repository, new Set(paths));
	}

	public setRefs(repository: string, refs: readonly GitRef[], remotes: readonly GitRemote[]): void {
		this.references.set(repository, { refs, remotes });
	}

	public setUser(repository: string, user: MemoryGitUser): void {
		this.users.set(repository, user);
	}

	public appendOutput(workspaceId: string, text: string): void {
		this.outputs.set(workspaceId, `${this.outputs.get(workspaceId) ?? ''}${text}`);
	}

	/** Asks one of Git's questions through a Workspace's relay. */
	public prompt(workspaceId: string, prompt: GitPrompt): void {
		this.promptsOf(workspaceId).set(prompt.id, prompt);
		this.sendRelay(workspaceId, { kind: 'prompt', prompt });
	}

	/** Closes a prompt, as Git does when it stops waiting for the answer. */
	public closePrompt(workspaceId: string, promptId: string): void {
		if (!this.promptsOf(workspaceId).delete(promptId)) return;
		this.sendRelay(workspaceId, { id: promptId, kind: 'closed' });
	}

	/**
	 * Drops the link of these sessions: status cursors end, open prompts close as the connection's
	 * prompts are released, and each summary stream gets a gap.
	 */
	public disconnect(sessions: readonly MemorySessionState[]): void {
		for (const [cursor, position] of this.statusCursors) {
			if (sessions.includes(position.session)) this.statusCursors.delete(cursor);
		}
		const workspaceIds = new Set(sessions.map((session) => session.workspaceId));
		for (const workspaceId of workspaceIds) {
			// Deleting the visited key keeps a Map iteration valid.
			for (const promptId of this.promptsOf(workspaceId).keys()) {
				this.closePrompt(workspaceId, promptId);
			}
		}
		for (const session of sessions) {
			for (const stream of session.summaryStreams) {
				if (stream.ready) stream.listener({ kind: 'gap' });
				stream.ready = false;
			}
			for (const stream of session.relayStreams) stream.ready = false;
		}
	}

	/** Restarts the streams of these sessions on the new link with their first records. */
	public reconnect(sessions: readonly MemorySessionState[]): void {
		for (const session of sessions) {
			for (const stream of session.summaryStreams) this.startSummary(stream, session.workspaceId);
			for (const stream of session.relayStreams) this.startRelay(stream, session.workspaceId);
		}
	}

	/** Ends a closed session's status cursors. */
	public release(session: MemorySessionState): void {
		for (const [cursor, position] of this.statusCursors) {
			if (position.session === session) this.statusCursors.delete(cursor);
		}
	}

	public createPort(session: MemorySessionState): GitPort {
		const workspaceId = session.workspaceId;
		return {
			answer: (promptId, answer, signal) =>
				this.answering.answer(session, signal, () => {
					if (!this.promptsOf(workspaceId).has(promptId)) return failure('not-found');
					this.answered.push({ answer, promptId, workspaceId });
					this.closePrompt(workspaceId, promptId);
					return ok(null);
				}),
			checkIgnore: (request, signal) =>
				this.answering.answer(session, signal, () => {
					const ignored = this.ignored.get(request.repository);
					return ok(request.paths.filter((path) => ignored?.has(path) === true));
				}),
			outcome: (requestId, signal) =>
				this.answering.answer(session, signal, (): FileResult<GitRequestOutcome> => {
					const recorded = this.recordedOf(workspaceId).get(requestId);
					if (recorded === undefined) return ok({ state: 'unknown' });
					if (recorded.result === null) return ok({ state: 'running' });
					return ok({ result: recorded.result, state: 'completed' });
				}),
			output: (cursor, signal) =>
				this.answering.answer(session, signal, () => {
					const text = this.outputs.get(workspaceId) ?? '';
					const offset = cursor === null ? 0 : Number(cursor);
					if (!Number.isInteger(offset) || offset < 0 || offset > text.length) {
						return failure('invalid-request');
					}
					return ok({ cursor: String(text.length), dropped: 0, text: text.slice(offset) });
				}),
			refs: (request, signal) =>
				this.answering.answer(session, signal, () => {
					if (request.cursor !== undefined) return failure('invalid-request');
					const references = this.references.get(request.repository);
					return ok({
						cursor: null,
						refs: references?.refs ?? [],
						remotes: references?.remotes ?? [],
					});
				}),
			run: (requestId, mutation, signal) => this.run(session, requestId, mutation, signal),
			show: (request, signal) => {
				const key = showKey(request.repository, request.ref, request.path);
				const issued = this.shows.get(key);
				return this.answering.answer(session, signal, () => {
					const content = this.shows.get(key);
					if (content === undefined) return failure('not-found');
					return readMemoryContent(content, request, memoryVersion(issued ?? content));
				});
			},
			statusPage: (request, signal) =>
				this.answering.answer(session, signal, () => this.statusPage(session, request)),
			subscribe: (listener) =>
				this.follow(session, session.summaryStreams, listener, (stream) =>
					this.startSummary(stream, workspaceId),
				),
			subscribeRelay: (listener) =>
				this.follow(session, session.relayStreams, listener, (stream) =>
					this.startRelay(stream, workspaceId),
				),
			userConfig: (repository, signal) =>
				this.answering.answer(session, signal, () =>
					ok(this.users.get(repository) ?? { email: null, name: null }),
				),
		};
	}

	/** Adds a stream that starts on a microtask, or when the link returns. */
	private follow<Event>(
		session: MemorySessionState,
		streams: Set<MemoryStream<Event>>,
		listener: (event: Event) => void,
		start: (stream: MemoryStream<Event>) => void,
	): () => void {
		if (session.closed) return () => undefined;
		const stream: MemoryStream<Event> = { listener, ready: false, sequence: 0 };
		streams.add(stream);
		queueMicrotask(() => {
			if (!session.closed && streams.has(stream) && this.answering.connected) start(stream);
		});
		return () => {
			streams.delete(stream);
		};
	}

	private startSummary(stream: MemoryStream<GitSummaryEvent>, workspaceId: string): void {
		if (stream.ready) return;
		stream.ready = true;
		this.sendSummary(stream, workspaceId);
	}

	private startRelay(stream: MemoryStream<GitRelayEvent>, workspaceId: string): void {
		if (stream.ready) return;
		stream.ready = true;
		for (const prompt of this.promptsOf(workspaceId).values()) {
			stream.listener({ kind: 'prompt', prompt });
		}
	}

	private run(
		session: MemorySessionState,
		requestId: string,
		mutation: GitMutation,
		signal: AbortSignal,
	): Promise<FileResult<GitMutationOutput>> {
		const table = this.recordedOf(session.workspaceId);
		const existing = table.get(requestId);
		if (existing !== undefined) {
			return existing.result === null
				? followRun(existing.promise, signal)
				: Promise.resolve(existing.result);
		}
		let reachedGit = false;
		const record = (result: FileResult<GitMutationOutput>): void => {
			const recorded = table.get(requestId);
			if (recorded !== undefined) recorded.result = result;
		};
		const promise = this.answering.answer(session, signal, () => {
			reachedGit = true;
			this.ran.push({ mutation, requestId, workspaceId: session.workspaceId });
			const result = this.handler(mutation, session.workspaceId);
			// Recorded as it runs, so an outcome asked before the reply arrives sees it.
			record(result);
			return result;
		});
		table.set(requestId, { promise, result: null });
		void promise.then((result) => {
			if (reachedGit) return;
			// The link dropped before the change reached Git, so the same ID may be sent again.
			if (result.kind === 'error' && result.error.code === 'remote-disconnected') {
				table.delete(requestId);
				return;
			}
			record(result);
		});
		return promise;
	}

	private statusPage(
		session: MemorySessionState,
		request: {
			readonly cursor?: string | undefined;
			readonly repository: string;
			readonly revision: number;
		},
	): FileResult<{ readonly cursor: string | null; readonly entries: readonly GitStatusEntry[] }> {
		let position: MemoryStatusCursor;
		if (request.cursor === undefined) {
			position = { offset: 0, repository: request.repository, revision: request.revision, session };
		} else {
			const continued = this.statusCursors.get(request.cursor);
			if (continued?.session !== session) return failure('invalid-request');
			this.statusCursors.delete(request.cursor);
			position = continued;
		}
		const status = this.statuses.get(position.repository);
		if (status === undefined) return failure('not-found');
		if (position.revision < status.revision) return failure('stale-revision');
		if (position.revision > status.revision) return failure('invalid-request');
		const end = position.offset + this.pageSize;
		const entries = status.entries.slice(position.offset, end);
		if (end >= status.entries.length) return ok({ cursor: null, entries });
		const cursor = `status-${this.nextCursor}`;
		this.nextCursor += 1;
		this.statusCursors.set(cursor, { ...position, offset: end });
		return ok({ cursor, entries });
	}

	private sendSummary(stream: MemoryStream<GitSummaryEvent>, workspaceId: string): void {
		stream.sequence += 1;
		stream.listener({
			kind: 'summary',
			sequence: stream.sequence,
			summary: this.summaries.get(workspaceId) ?? NO_REPOSITORIES,
		});
	}

	private sendRelay(workspaceId: string, event: GitRelayEvent): void {
		for (const session of this.sessionsOf(workspaceId)) {
			for (const stream of session.relayStreams) {
				if (stream.ready) stream.listener(event);
			}
		}
	}

	private promptsOf(workspaceId: string): Map<string, GitPrompt> {
		let prompts = this.prompts.get(workspaceId);
		if (prompts === undefined) {
			prompts = new Map();
			this.prompts.set(workspaceId, prompts);
		}
		return prompts;
	}

	private recordedOf(workspaceId: string): Map<string, MemoryRecordedRun> {
		let table = this.recorded.get(workspaceId);
		if (table === undefined) {
			table = new Map();
			this.recorded.set(workspaceId, table);
		}
		return table;
	}

	private defaultOutput(mutation: GitMutation): FileResult<GitMutationOutput> {
		switch (mutation.op) {
			case 'clean':
				return ok({
					items: mutation.paths.map((path) => ({ error: null, path })),
					kind: 'cleaned',
				});
			case 'commit':
				this.commits += 1;
				return ok({
					commit: this.commits.toString(16).padStart(COMMIT_DIGITS, '0'),
					kind: 'committed',
				});
			case 'init':
				return ok({ kind: 'initialized', repository: mutation.path });
			default:
				return ok({ kind: 'done' });
		}
	}
}
