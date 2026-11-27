import type {
	FileContent,
	FileReadOptions,
	FileResult,
	GitMutation,
	GitMutationOutput,
	GitPort,
	GitPromptAnswer,
	GitRelayEvent,
	GitRequestOutcome,
	GitSummaryEvent,
} from '../../../file-session';
import { FILES_OPERATIONS, FILES_PUT, type FilesReply } from '../../server-connection';
import type { ServerFileChannel, ServerFileServiceFollower } from './ServerFileChannel';
import { ServerGitRelayStream } from './ServerGitRelayStream';
import { ServerGitSummaryStream } from './ServerGitSummaryStream';
import { readServerTransfer } from './readServerTransfer';
import {
	CANCELLED,
	fileFailure,
	pathBatches,
	unansweredError,
	untilAborted,
} from './server-file-exchange';

type GitRun<Value> = Promise<FileResult<Value>>;
type PathsMutation = Extract<
	GitMutation,
	{ readonly op: 'clean' | 'discard' | 'stage' | 'unstage' }
>;

/**
 * How long a change's Result may take once sent. Changes to one repository run one at a time, so a
 * change may wait behind a push or a commit whose message editor stays open for hours.
 */
const MUTATION_DEADLINE_MILLISECONDS = 9 * 60 * 60 * 1000;
/** The paths one `git.check-ignore` takes, by the backend's bound. */
const PATHS_PER_CHECK_IGNORE = 1000;
const REQUEST_ID = /^[0-9a-f]{32}$/;
const utf8 = new TextEncoder();

/**
 * The request ID of one request of a change that needs several, as many paths do: the first keeps
 * the change's ID and the others derive theirs from it, so running the change again with the same
 * ID repeats only the requests whose outcome the backend does not hold.
 */
function batchRequestId(requestId: string, index: number): string {
	if (index === 0) return requestId;
	return (BigInt(`0x${requestId}`) ^ BigInt(index)).toString(16).padStart(32, '0');
}

/**
 * One Workspace's Source Control over a Server connection: its summary and relay streams, its reads,
 * and its changes. A change runs on a lane of its own, so a caller that gives it up closes the lane
 * and the Server stops the Git process; reads settle `cancelled` at once and their Results are
 * dropped. Focus is told only when it changes, and told again once a restarted service answers.
 */
export class ServerGitPort implements GitPort, ServerFileServiceFollower {
	readonly #abort = new AbortController();
	readonly #channel: ServerFileChannel;
	#closed = false;
	/** What the backend was last told of focus; null before the first call. */
	#focused: boolean | null = null;
	readonly #relay: ServerGitRelayStream;
	readonly #summary: ServerGitSummaryStream;
	readonly #unfollow: () => void;
	readonly #workspaceId: string;

	constructor(channel: ServerFileChannel, workspaceId: string) {
		this.#channel = channel;
		this.#workspaceId = workspaceId;
		this.#summary = new ServerGitSummaryStream(channel, workspaceId);
		this.#relay = new ServerGitRelayStream(channel, workspaceId);
		this.#unfollow = channel.follow(this);
	}

	subscribe(listener: (event: GitSummaryEvent) => void): () => void {
		return this.#summary.subscribe(listener);
	}

	subscribeRelay(listener: (event: GitRelayEvent) => void): () => void {
		return this.#relay.subscribe(listener);
	}

	focus(focused: boolean): void {
		if (this.#closed || this.#focused === focused) return;
		this.#focused = focused;
		this.#tellFocus(focused);
	}

	statusPage(
		request: Parameters<GitPort['statusPage']>[0],
		signal: AbortSignal,
	): ReturnType<GitPort['statusPage']> {
		const operation = FILES_OPERATIONS['git.status-page'];
		return this.#read(
			signal,
			operation.encode({
				cursor: request.cursor,
				op: 'git.status-page',
				repository: request.repository,
				revision: String(request.revision),
				workspaceId: this.#workspaceId,
			}),
			(record) => operation.decode(record),
		);
	}

	show(
		request: FileReadOptions & {
			readonly path: string;
			readonly ref: string;
			readonly repository: string;
		},
		signal: AbortSignal,
	): GitRun<FileContent> {
		return this.#run(signal, (combined) =>
			readServerTransfer(this.#channel, this.#workspaceId, request, combined, (transfer) =>
				FILES_OPERATIONS['git.show'].encode({
					encoding: request.encoding,
					limit: request.limit,
					mode: request.mode,
					op: 'git.show',
					path: request.path,
					ref: request.ref,
					repository: request.repository,
					transfer,
					workspaceId: this.#workspaceId,
				}),
			),
		);
	}

	checkIgnore(
		request: { readonly paths: readonly string[]; readonly repository: string },
		signal: AbortSignal,
	): GitRun<readonly string[]> {
		return this.#run(signal, async (combined) => {
			const results = await Promise.all(
				pathBatches(request.paths, PATHS_PER_CHECK_IGNORE).map((paths) =>
					this.#channel.request(
						FILES_OPERATIONS['git.check-ignore'].encode({
							op: 'git.check-ignore',
							paths: [...paths],
							repository: request.repository,
							workspaceId: this.#workspaceId,
						}),
						(record) => FILES_OPERATIONS['git.check-ignore'].decode(record),
						{ signal: combined },
					),
				),
			);
			const ignored: string[] = [];
			for (const result of results) {
				if (result.kind === 'error') return result;
				ignored.push(...result.value);
			}
			return { kind: 'ok', value: ignored };
		});
	}

	refs(request: Parameters<GitPort['refs']>[0], signal: AbortSignal): ReturnType<GitPort['refs']> {
		const operation = FILES_OPERATIONS['git.refs'];
		return this.#read(
			signal,
			operation.encode({
				cursor: request.cursor,
				op: 'git.refs',
				repository: request.repository,
				workspaceId: this.#workspaceId,
			}),
			(record) => operation.decode(record),
		);
	}

	userConfig(repository: string, signal: AbortSignal): ReturnType<GitPort['userConfig']> {
		const operation = FILES_OPERATIONS['git.user-config'];
		return this.#read(
			signal,
			operation.encode({ op: 'git.user-config', repository, workspaceId: this.#workspaceId }),
			(record) => operation.decode(record),
		);
	}

	output(cursor: string | null, signal: AbortSignal): ReturnType<GitPort['output']> {
		const operation = FILES_OPERATIONS['git.output'];
		return this.#read(
			signal,
			operation.encode({
				cursor: cursor ?? undefined,
				op: 'git.output',
				workspaceId: this.#workspaceId,
			}),
			(record) => operation.decode(record),
		);
	}

	outcome(requestId: string, signal: AbortSignal): GitRun<GitRequestOutcome> {
		const operation = FILES_OPERATIONS['request.outcome'];
		return this.#read(
			signal,
			operation.encode({ op: 'request.outcome', requestId, workspaceId: this.#workspaceId }),
			(record) => operation.decode(record),
		);
	}

	answer(promptId: string, answer: GitPromptAnswer, signal: AbortSignal): GitRun<null> {
		const operation = FILES_OPERATIONS['relay.answer'];
		const fields = {
			op: 'relay.answer',
			prompt: promptId,
			workspaceId: this.#workspaceId,
		} as const;
		return this.#read(
			signal,
			operation.encode(
				answer.kind === 'value'
					? { ...fields, value: answer.value }
					: { ...fields, cancelled: true },
			),
			(record) => operation.decode(record),
		);
	}

	/**
	 * Runs a change on a lane of its own. A change of more paths than one request carries runs as
	 * several requests in order, stopping at the first failure; each derives its ID from `requestId`.
	 */
	run(requestId: string, mutation: GitMutation, signal: AbortSignal): GitRun<GitMutationOutput> {
		if (!REQUEST_ID.test(requestId)) return Promise.resolve(fileFailure('invalid-request'));
		return this.#run(signal, (combined) =>
			this.#onLane(combined, (laneId) => this.#mutate(laneId, requestId, mutation, combined)),
		);
	}

	close(): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#abort.abort();
		this.#unfollow();
		this.#summary.close();
		this.#relay.close();
		// The backend keeps a connection's focus until it hears otherwise.
		if (this.#focused === true) this.#tellFocus(false);
	}

	ended(): void {
		this.#summary.end();
		this.#relay.end();
	}

	lost(): void {
		// A restarted service no longer knows the focus; `served` tells it again.
	}

	served(regained: boolean): void {
		this.#summary.served();
		this.#relay.served();
		if (regained && this.#focused !== null && !this.#closed) this.#tellFocus(this.#focused);
	}

	#tellFocus(focused: boolean): void {
		void this.#channel.request(
			FILES_OPERATIONS['client.focus'].encode({
				focused,
				op: 'client.focus',
				workspaceId: this.#workspaceId,
			}),
			(record) => FILES_OPERATIONS['client.focus'].decode(record),
		);
	}

	/** Runs a request under the caller's signal and the port's, settling `cancelled` on either. */
	#run<Value>(
		signal: AbortSignal,
		request: (combined: AbortSignal) => GitRun<Value>,
	): GitRun<Value> {
		if (this.#closed) return Promise.resolve(CANCELLED);
		const combined = AbortSignal.any([signal, this.#abort.signal]);
		return untilAborted(request(combined), combined);
	}

	#read<Value>(
		signal: AbortSignal,
		record: Uint8Array | null,
		decode: (result: Uint8Array) => FilesReply<Value> | null,
	): GitRun<Value> {
		return this.#run(signal, (combined) =>
			this.#channel.request(record, decode, { signal: combined }),
		);
	}

	/** Runs `work` on a lane that a caller who gives up closes, which stops what runs on it. */
	async #onLane<Value>(
		signal: AbortSignal,
		work: (laneId: bigint) => GitRun<Value>,
	): GitRun<Value> {
		const release = await this.#channel.budget.enter('transfer', signal);
		if (release === null) return CANCELLED;
		const { connection } = this.#channel;
		let laneId: bigint;
		try {
			laneId = connection.openLane();
		} catch (error) {
			release();
			return { error: unansweredError(error), kind: 'error' };
		}
		const cancel = (): void => connection.closeLane(laneId);
		signal.addEventListener('abort', cancel, { once: true });
		try {
			return await untilAborted(work(laneId), signal);
		} finally {
			signal.removeEventListener('abort', cancel);
			connection.closeLane(laneId);
			release();
		}
	}

	#mutate(
		laneId: bigint,
		requestId: string,
		mutation: GitMutation,
		signal: AbortSignal,
	): GitRun<GitMutationOutput> {
		const send = (record: Uint8Array | null, decode: MutationDecode): GitRun<GitMutationOutput> =>
			this.#channel.request(record, decode, {
				laneId,
				signal,
				timeoutMilliseconds: MUTATION_DEADLINE_MILLISECONDS,
			});
		const workspaceId = this.#workspaceId;
		switch (mutation.op) {
			case 'clean': {
				const { mode, repository } = mutation;
				const operation = FILES_OPERATIONS['git.clean'];
				return this.#mutatePaths(requestId, mutation, (id, paths) =>
					send(
						operation.encode({
							mode,
							op: 'git.clean',
							paths,
							repository,
							requestId: id,
							workspaceId,
						}),
						(record) => operation.decode(record),
					),
				);
			}
			case 'discard': {
				const { repository } = mutation;
				const operation = FILES_OPERATIONS['git.discard'];
				return this.#mutatePaths(requestId, mutation, (id, paths) =>
					send(
						operation.encode({ op: 'git.discard', paths, repository, requestId: id, workspaceId }),
						(record) => operation.decode(record),
					),
				);
			}
			case 'stage': {
				const { repository } = mutation;
				const operation = FILES_OPERATIONS['git.stage'];
				return this.#mutatePaths(requestId, mutation, (id, paths) =>
					send(
						operation.encode({ op: 'git.stage', paths, repository, requestId: id, workspaceId }),
						(record) => operation.decode(record),
					),
				);
			}
			case 'unstage': {
				const { repository } = mutation;
				const operation = FILES_OPERATIONS['git.unstage'];
				return this.#mutatePaths(requestId, mutation, (id, paths) =>
					send(
						operation.encode({ op: 'git.unstage', paths, repository, requestId: id, workspaceId }),
						(record) => operation.decode(record),
					),
				);
			}
			case 'stage-content':
				return this.#stageContent(laneId, requestId, mutation, signal);
			case 'commit': {
				const operation = FILES_OPERATIONS['git.commit'];
				const request = {
					all: mutation.all,
					amend: mutation.amend,
					noVerify: mutation.noVerify,
					op: 'git.commit',
					repository: mutation.repository,
					requestId,
					signoff: mutation.signoff,
					workspaceId,
				} as const;
				return send(
					operation.encode(
						mutation.message.kind === 'editor'
							? { ...request, editor: true }
							: { ...request, message: mutation.message.text },
					),
					(record) => operation.decode(record),
				);
			}
			case 'fetch': {
				const operation = FILES_OPERATIONS['git.fetch'];
				const request = {
					background: mutation.background,
					op: 'git.fetch',
					prune: mutation.prune,
					repository: mutation.repository,
					requestId,
					workspaceId,
				} as const;
				const { remote } = mutation;
				return send(
					operation.encode(
						remote.kind === 'named'
							? { ...request, remote: remote.name }
							: { ...request, all: remote.kind === 'all' },
					),
					(record) => operation.decode(record),
				);
			}
			case 'pull': {
				const operation = FILES_OPERATIONS['git.pull'];
				const request = { rebase: mutation.rebase, repository: mutation.repository, requestId };
				return send(operation.encode({ ...request, op: 'git.pull', workspaceId }), (record) =>
					operation.decode(record),
				);
			}
			case 'sync': {
				const operation = FILES_OPERATIONS['git.sync'];
				const request = { rebase: mutation.rebase, repository: mutation.repository, requestId };
				return send(operation.encode({ ...request, op: 'git.sync', workspaceId }), (record) =>
					operation.decode(record),
				);
			}
			case 'push': {
				const operation = FILES_OPERATIONS['git.push'];
				return send(
					operation.encode({
						branch: mutation.branch ?? undefined,
						force: mutation.force,
						op: 'git.push',
						remote: mutation.remote ?? undefined,
						repository: mutation.repository,
						requestId,
						setUpstream: mutation.setUpstream,
						tags: mutation.tags,
						workspaceId,
					}),
					(record) => operation.decode(record),
				);
			}
			case 'init': {
				const operation = FILES_OPERATIONS['git.init'];
				return send(
					operation.encode({ op: 'git.init', path: mutation.path, requestId, workspaceId }),
					(record) => operation.decode(record),
				);
			}
			case 'mark-safe': {
				const operation = FILES_OPERATIONS['git.mark-safe'];
				return send(
					operation.encode({ op: 'git.mark-safe', path: mutation.path, requestId, workspaceId }),
					(record) => operation.decode(record),
				);
			}
		}
	}

	/** Sends a paths change in as many requests as its paths need, in order. */
	async #mutatePaths(
		requestId: string,
		mutation: PathsMutation,
		send: (requestId: string, paths: string[]) => GitRun<GitMutationOutput>,
	): GitRun<GitMutationOutput> {
		const batches = pathBatches(mutation.paths);
		if (batches.length === 0) return fileFailure('invalid-request');
		const items: Extract<GitMutationOutput, { readonly kind: 'cleaned' }>['items'][number][] = [];
		for (const [index, paths] of batches.entries()) {
			const result = await send(batchRequestId(requestId, index), [...paths]);
			if (result.kind === 'error') return result;
			if (result.value.kind === 'cleaned') items.push(...result.value.items);
		}
		return {
			kind: 'ok',
			value: mutation.op === 'clean' ? { items, kind: 'cleaned' } : { kind: 'done' },
		};
	}

	/** Sends the index content as Puts behind its opening request, the last one committing it. */
	async #stageContent(
		laneId: bigint,
		requestId: string,
		mutation: Extract<GitMutation, { readonly op: 'stage-content' }>,
		signal: AbortSignal,
	): GitRun<GitMutationOutput> {
		const data = utf8.encode(mutation.text);
		const transfer = this.#channel.nextTransfer();
		const opening = this.#channel.request(
			FILES_OPERATIONS['git.stage-content'].encode({
				op: 'git.stage-content',
				path: mutation.path,
				repository: mutation.repository,
				requestId,
				size: data.byteLength,
				transfer: transfer.toString(),
				workspaceId: this.#workspaceId,
			}),
			(record) => FILES_OPERATIONS['git.stage-content'].decode(record),
			{ laneId, signal, timeoutMilliseconds: MUTATION_DEADLINE_MILLISECONDS },
		);
		const puts: GitRun<null>[] = [];
		const length = this.#channel.chunkLength;
		let offset = 0;
		do {
			const piece = data.subarray(offset, offset + length);
			const final = offset + piece.byteLength === data.byteLength;
			puts.push(this.#put(laneId, transfer, offset, final, piece, signal));
			offset += piece.byteLength;
		} while (offset < data.byteLength);
		const [answer, ...pieces] = await Promise.all([opening, ...puts]);
		if (answer.kind === 'error') return answer;
		const failed = pieces.find((piece) => piece.kind === 'error');
		return failed?.kind === 'error' ? failed : answer;
	}

	async #put(
		laneId: bigint,
		transfer: bigint,
		offset: number,
		final: boolean,
		data: Uint8Array,
		signal: AbortSignal,
	): GitRun<null> {
		const record = FILES_PUT.encode(transfer, BigInt(offset), final, data);
		if (record === null) return fileFailure('invalid-request');
		const release = await this.#channel.budget.enter('chunk', signal);
		if (release === null) return CANCELLED;
		try {
			const reply = FILES_PUT.decode(await this.#channel.connection.exchange(record, { laneId }));
			if (reply === null) return fileFailure('failed');
			return reply;
		} catch (error) {
			return { error: unansweredError(error), kind: 'error' };
		} finally {
			release();
		}
	}
}

type MutationDecode = (record: Uint8Array) => FilesReply<GitMutationOutput> | null;
