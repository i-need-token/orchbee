import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

import type { GitRelayEvent, GitSummaryEvent } from '../../../../file-session';
import { ServerConnectionEndedError, ServerStreamRefusedError } from '../../../server-connection';
import { ServerFileSessionPort } from '../ServerFileSessionPort';
import type { ServerFileConnection } from '../server-file-exchange';

type EndListener = Parameters<ServerFileConnection['onEnded']>[0];
type SubscriptionHandlers = Parameters<ServerFileConnection['subscribe']>[2];

const validateRequest = new Ajv2020({ strict: false, validateFormats: false }).compile(
	z
		.record(z.string(), z.unknown())
		.parse(
			JSON.parse(
				await readFile(
					new URL('../../../../../../../schemas/files/v1/files.schema.json', import.meta.url),
					'utf8',
				),
			),
		),
);
const requestJson = z.looseObject({ op: z.string() });
type RequestJson = z.output<typeof requestJson>;

const encoder = new TextEncoder();
const WORKSPACE = '0123456789abcdef0123456789abcdef';
const REQUEST_ID = 'fedcba9876543210fedcba9876543210';
const REPOSITORY = '/home/dev/project';
const PROMPT = {
	id: '00000000000000000000000000000001',
	kind: 'password',
	repository: REPOSITORY,
	text: "Password for 'https://dev@example.com': ",
} as const;

function obfb(kind: number, json: unknown): Uint8Array {
	const body = encoder.encode(JSON.stringify(json));
	const record = new Uint8Array(12 + body.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x46, 0x42]);
	view.setUint16(4, 1);
	view.setUint16(6, kind);
	view.setUint32(8, body.byteLength);
	record.set(body, 12);
	return record;
}

const result = (json: unknown): Uint8Array => obfb(2, json);
const notice = (json: unknown): Uint8Array => obfb(3, json);

function summary(revision: string): unknown {
	return {
		candidates: [],
		git: { state: 'ok', version: '2.47.1' },
		repositories: [
			{
				counts: { index: 0, merge: 0, untracked: 1, workingTree: 2 },
				head: { branch: 'main', detached: false, unborn: false },
				kind: 'root',
				operation: 'none',
				repository: REPOSITORY,
				revision,
				stale: false,
				truncated: false,
				warm: false,
			},
		],
		repositoriesTruncated: false,
		unsafe: [],
	};
}

interface PendingExchange {
	readonly laneId: bigint | undefined;
	readonly reject: (error: Error) => void;
	readonly request: RequestJson;
	readonly resolve: (record: Uint8Array) => void;
}

interface OpenSubscription {
	closed: boolean;
	readonly handlers: SubscriptionHandlers;
	readonly request: RequestJson;
}

/** A connection whose requests the test answers, checking each against the schema. */
class FakeConnection implements ServerFileConnection {
	readonly closedLanes: bigint[] = [];
	readonly pending: PendingExchange[] = [];
	readonly sent: RequestJson[] = [];
	readonly subscriptions: OpenSubscription[] = [];
	readonly #ended = new Set<EndListener>();
	#lane = 0n;

	exchange(request: Uint8Array, options: { readonly laneId?: bigint } = {}): Promise<Uint8Array> {
		const json: unknown = JSON.parse(new TextDecoder().decode(request.subarray(12)));
		if (!validateRequest(json)) throw new Error(`Invalid request: ${JSON.stringify(json)}`);
		const parsed = requestJson.parse(json);
		this.sent.push(parsed);
		return new Promise((resolve, reject) => {
			this.pending.push({ laneId: options.laneId, reject, request: parsed, resolve });
		});
	}

	subscribe(
		_mode: Parameters<ServerFileConnection['subscribe']>[0],
		request: Uint8Array,
		handlers: SubscriptionHandlers,
	): ReturnType<ServerFileConnection['subscribe']> {
		const parsed = requestJson.parse(JSON.parse(new TextDecoder().decode(request.subarray(12))));
		const subscription: OpenSubscription = { closed: false, handlers, request: parsed };
		this.subscriptions.push(subscription);
		return {
			close: () => {
				subscription.closed = true;
			},
			resnapshot: () => undefined,
		};
	}

	openLane(): bigint {
		this.#lane += 1n;
		return this.#lane;
	}

	closeLane(laneId: bigint): void {
		this.closedLanes.push(laneId);
	}

	onEnded(listener: EndListener): () => void {
		this.#ended.add(listener);
		return () => this.#ended.delete(listener);
	}

	end(): void {
		for (const exchange of this.pending.splice(0)) {
			exchange.reject(new ServerConnectionEndedError({ kind: 'closed' }));
		}
		for (const listener of this.#ended) listener({ kind: 'closed' });
	}

	take(op: string): PendingExchange {
		const index = this.pending.findIndex((exchange) => exchange.request.op === op);
		const exchange = this.pending[index];
		if (exchange === undefined) throw new Error(`No pending ${op}.`);
		this.pending.splice(index, 1);
		return exchange;
	}

	stream(op: string): OpenSubscription {
		const subscription = [...this.subscriptions]
			.reverse()
			.find((candidate) => candidate.request.op === op);
		if (subscription === undefined) throw new Error(`No ${op} stream.`);
		return subscription;
	}
}

async function settle(): Promise<void> {
	for (let turn = 0; turn < 50; turn += 1) await Promise.resolve();
}

function openGit() {
	const connection = new FakeConnection();
	const port = new ServerFileSessionPort({
		capabilities: ['files'],
		connection,
		machine: 'local',
		remote: false,
	});
	const session = port.open(WORKSPACE);
	return { connection, git: session.git, session };
}

const PUSH = {
	branch: null,
	force: 'none',
	op: 'push',
	remote: null,
	repository: REPOSITORY,
	setUpstream: false,
	tags: false,
} as const;

afterEach(() => {
	vi.useRealTimers();
});

describe('ServerGitPort', () => {
	it('drops a prompt that arrives after the Workspace stopped following the relay', async () => {
		const { connection, git } = openGit();
		const events: GitRelayEvent[] = [];
		const stop = git.subscribeRelay((event) => events.push(event));
		await settle();
		const relay = connection.stream('relay.subscribe');
		relay.handlers.onFrame('snapshot', notice({ prompts: [] }));

		// The window shows another Workspace now, so the Server must route this prompt elsewhere.
		stop();
		relay.handlers.onFrame('update', notice({ kind: 'prompt', prompt: PROMPT }));
		await settle();

		expect(relay.closed).toBe(true);
		expect(events).toEqual([]);
	});

	it('cancels a push whose prompt is open by closing its lane, and closes the prompt as told', async () => {
		const { connection, git } = openGit();
		const events: GitRelayEvent[] = [];
		git.subscribeRelay((event) => events.push(event));
		await settle();
		const relay = connection.stream('relay.subscribe');
		relay.handlers.onFrame('snapshot', notice({ prompts: [] }));
		const abort = new AbortController();
		const pushed = git.run(REQUEST_ID, PUSH, abort.signal);
		await settle();
		const push = connection.take('git.push');
		relay.handlers.onFrame('update', notice({ kind: 'prompt', prompt: PROMPT }));

		abort.abort();

		expect(await pushed).toEqual({ error: { code: 'cancelled' }, kind: 'error' });
		expect(connection.closedLanes).toContain(push.laneId);
		// The Server ends the cancelled push's prompt; an answer typed meanwhile is refused.
		push.reject(new ServerStreamRefusedError('cancelled'));
		relay.handlers.onFrame('update', notice({ id: PROMPT.id, kind: 'closed' }));
		expect(events).toEqual([
			{ kind: 'prompt', prompt: { ...PROMPT, path: null } },
			{ id: PROMPT.id, kind: 'closed' },
		]);
	});

	it('keeps an open prompt across a gap and closes it once the snapshot no longer lists it', async () => {
		const { connection, git } = openGit();
		const events: GitRelayEvent[] = [];
		git.subscribeRelay((event) => events.push(event));
		await settle();
		const relay = connection.stream('relay.subscribe');
		relay.handlers.onFrame('snapshot', notice({ prompts: [PROMPT] }));

		relay.handlers.onGap(1n);
		relay.handlers.onFrame('snapshot', notice({ prompts: [PROMPT] }));
		relay.handlers.onGap(1n);
		relay.handlers.onFrame('snapshot', notice({ prompts: [] }));

		expect(events).toEqual([
			{ kind: 'prompt', prompt: { ...PROMPT, path: null } },
			{ id: PROMPT.id, kind: 'closed' },
		]);
	});

	it('settles a running change, closes open prompts and leaves a gap when the link drops', async () => {
		vi.useFakeTimers();
		const { connection, git } = openGit();
		const prompts: GitRelayEvent[] = [];
		const summaries: GitSummaryEvent[] = [];
		git.subscribeRelay((event) => prompts.push(event));
		git.subscribe((event) => summaries.push(event));
		await settle();
		connection
			.stream('relay.subscribe')
			.handlers.onFrame('snapshot', notice({ prompts: [PROMPT] }));
		connection.stream('scm.subscribe').handlers.onFrame('snapshot', notice(summary('3')));
		const pushed = git.run(REQUEST_ID, PUSH, new AbortController().signal);
		await settle();

		connection.end();

		expect(await pushed).toEqual({ error: { code: 'remote-disconnected' }, kind: 'error' });
		expect(prompts.at(-1)).toEqual({ id: PROMPT.id, kind: 'closed' });
		expect(summaries.map((event) => event.kind)).toEqual(['summary', 'gap']);
		await vi.runAllTimersAsync();
		// Nothing opens again on an ended connection.
		expect(connection.subscriptions).toHaveLength(2);
	});

	it('sends a change of many paths as requests in order, each with its own request ID', async () => {
		const { connection, git } = openGit();
		const paths = Array.from({ length: 300 }, (_, index) => `${REPOSITORY}/f${index}`);
		const staged = git.run(
			REQUEST_ID,
			{ op: 'stage', paths, repository: REPOSITORY },
			new AbortController().signal,
		);
		await settle();
		const first = connection.take('git.stage');
		expect(connection.pending).toHaveLength(0);
		first.resolve(result({}));
		await settle();
		const second = connection.take('git.stage');
		second.resolve(result({}));

		expect(await staged).toEqual({ kind: 'ok', value: { kind: 'done' } });
		expect([first.request['requestId'], second.request['requestId']]).toEqual([
			REQUEST_ID,
			'fedcba9876543210fedcba9876543211',
		]);
		expect([first.request['paths'], second.request['paths']]).toEqual([
			paths.slice(0, 256),
			paths.slice(256),
		]);
	});

	it('tells focus once per change and again after the service restarts', async () => {
		const { connection, git } = openGit();
		git.focus(true);
		git.focus(true);
		await settle();
		connection.take('client.focus').resolve(result({}));
		const listed = git.refs({ repository: REPOSITORY }, new AbortController().signal);
		await settle();
		connection
			.take('git.refs')
			.resolve(result({ error: { code: 'unavailable', reason: 'restarting' } }));
		await listed;
		const status = git.statusPage(
			{ repository: REPOSITORY, revision: 3 },
			new AbortController().signal,
		);
		await settle();
		connection.take('git.status-page').resolve(result({ entries: [] }));
		await status;
		await settle();

		expect(connection.sent.filter((request) => request.op === 'client.focus')).toEqual([
			{ focused: true, op: 'client.focus', workspaceId: WORKSPACE },
			{ focused: true, op: 'client.focus', workspaceId: WORKSPACE },
		]);
	});
});
