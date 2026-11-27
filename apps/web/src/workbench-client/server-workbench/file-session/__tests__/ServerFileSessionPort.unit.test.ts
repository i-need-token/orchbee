import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

import type { FileRootEvent, FileWorkspaceSession } from '../../../../file-session';
import { ServerConnectionEndedError } from '../../../server-connection';
import { ServerFileSessionPort } from '../ServerFileSessionPort';
import type { ServerFileConnection } from '../server-file-exchange';

type EndListener = Parameters<ServerFileConnection['onEnded']>[0];
type SubscriptionHandlers = Parameters<ServerFileConnection['subscribe']>[2];

const schema = z
	.record(z.string(), z.unknown())
	.parse(
		JSON.parse(
			await readFile(
				new URL('../../../../../../../schemas/files/v1/files.schema.json', import.meta.url),
				'utf8',
			),
		),
	);
const validateRequest = new Ajv2020({ strict: false, validateFormats: false }).compile(schema);
const requestJson = z.looseObject({ op: z.string() });

const encoder = new TextEncoder();
const WORKSPACE = '0123456789abcdef0123456789abcdef';
const LINE_ENDINGS = { lf: 2, none: 1 } as const;
const ENTRY = { mtimeMs: 1, name: 'a.txt', readonly: false, size: 3, symlink: false, type: 'file' };

function obfb(kind: number, body: Uint8Array): Uint8Array {
	const record = new Uint8Array(12 + body.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x46, 0x42]);
	view.setUint16(4, 1);
	view.setUint16(6, kind);
	view.setUint32(8, body.byteLength);
	record.set(body, 12);
	return record;
}

function result(json: unknown): Uint8Array {
	return obfb(2, encoder.encode(JSON.stringify(json)));
}

function notice(json: unknown): Uint8Array {
	return obfb(3, encoder.encode(JSON.stringify(json)));
}

function chunk(
	transfer: unknown,
	offset: number,
	data: string,
	end?: keyof typeof LINE_ENDINGS,
): Uint8Array {
	const bytes = encoder.encode(data);
	const body = new Uint8Array(18 + bytes.byteLength);
	const view = new DataView(body.buffer);
	view.setBigUint64(0, BigInt(z.string().parse(transfer)));
	view.setBigUint64(8, BigInt(offset));
	view.setUint8(16, end === undefined ? 0 : 1);
	view.setUint8(17, end === undefined ? 0 : LINE_ENDINGS[end]);
	body.set(bytes, 18);
	return obfb(5, body);
}

const snapshot = (path: string): Uint8Array =>
	notice({
		caseSensitive: true,
		os: 'linux',
		root: { path, state: 'present' },
		separator: '/',
		watch: 'full',
	});

interface PendingExchange {
	readonly laneId: bigint | undefined;
	readonly reject: (error: Error) => void;
	readonly request: z.output<typeof requestJson>;
	readonly resolve: (record: Uint8Array) => void;
}

interface OpenSubscription {
	closed: boolean;
	readonly handlers: SubscriptionHandlers;
}

/** Reads a request record as the Server would, after checking it against the schema. */
function readRequest(record: Uint8Array): z.output<typeof requestJson> {
	const json: unknown = JSON.parse(new TextDecoder().decode(record.subarray(12)));
	if (!validateRequest(json)) throw new Error(`Invalid request: ${JSON.stringify(json)}`);
	return requestJson.parse(json);
}

/** A connection whose requests the test answers, in whatever order it chooses. */
class FakeConnection implements ServerFileConnection {
	readonly closedLanes: bigint[] = [];
	readonly pending: PendingExchange[] = [];
	readonly sent: z.output<typeof requestJson>[] = [];
	readonly subscriptions: OpenSubscription[] = [];
	readonly #ended = new Set<EndListener>();
	#lane = 0n;
	#over = false;

	exchange(request: Uint8Array, options: { readonly laneId?: bigint } = {}): Promise<Uint8Array> {
		const json = readRequest(request);
		this.sent.push(json);
		if (this.#over) return Promise.reject(new ServerConnectionEndedError({ kind: 'closed' }));
		return new Promise((resolve, reject) => {
			this.pending.push({ laneId: options.laneId, reject, request: json, resolve });
		});
	}

	subscribe(
		_mode: Parameters<ServerFileConnection['subscribe']>[0],
		request: Uint8Array,
		handlers: SubscriptionHandlers,
	): ReturnType<ServerFileConnection['subscribe']> {
		this.sent.push(readRequest(request));
		const subscription: OpenSubscription = { closed: false, handlers };
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
		this.#over = true;
		for (const exchange of this.pending.splice(0)) {
			exchange.reject(new ServerConnectionEndedError({ kind: 'closed' }));
		}
		for (const listener of this.#ended) listener({ kind: 'closed' });
	}

	/** Answers the first pending request of `op`. */
	answer(op: string, record: Uint8Array): z.output<typeof requestJson> {
		const exchange = this.take(op);
		exchange.resolve(record);
		return exchange.request;
	}

	take(op: string): PendingExchange {
		const index = this.pending.findIndex((exchange) => exchange.request.op === op);
		const exchange = this.pending[index];
		if (exchange === undefined) throw new Error(`No pending ${op}.`);
		this.pending.splice(index, 1);
		return exchange;
	}

	ops(): readonly string[] {
		return this.pending.map((exchange) => exchange.request.op);
	}

	/** The paths of every request of `op` sent so far, one list per request. */
	pathsOf(op: string): readonly unknown[] {
		return this.sent.filter((request) => request.op === op).map((request) => request['paths']);
	}
}

/** Lets every queued continuation run. */
async function settle(): Promise<void> {
	for (let turn = 0; turn < 50; turn += 1) await Promise.resolve();
}

function openSession(remote = false): {
	connection: FakeConnection;
	session: FileWorkspaceSession;
} {
	const connection = new FakeConnection();
	const port = new ServerFileSessionPort({
		capabilities: ['files', 'agent-confirmation'],
		connection,
		machine: 'local',
		remote,
	});
	return { connection, session: port.open(WORKSPACE) };
}

function follow(session: FileWorkspaceSession): FileRootEvent[] {
	const events: FileRootEvent[] = [];
	session.subscribe((event) => events.push(event));
	return events;
}

afterEach(() => {
	vi.useRealTimers();
});

describe('ServerFileSessionPort', () => {
	it('gaps every follower of a root stream that lost records and starts them over', async () => {
		const { connection, session } = openSession();
		const first = follow(session);
		const second = follow(session);
		await settle();
		expect(connection.subscriptions).toHaveLength(1);
		const stream = connection.subscriptions[0]?.handlers;
		stream?.onFrame('snapshot', snapshot('/work'));
		stream?.onFrame(
			'update',
			notice({ changes: [{ directory: false, path: '/work/a', type: 'added' }], kind: 'changes' }),
		);
		stream?.onGap(2n);
		stream?.onFrame('snapshot', snapshot('/elsewhere'));
		const late = follow(session);
		await settle();

		const root = (path: string) => ({ path, state: 'present' });
		const rules = { caseSensitive: true, os: 'linux', separator: '/' };
		const expected = [
			{ kind: 'snapshot', root: root('/work'), rules, sequence: 1 },
			{
				changes: [{ directory: false, path: '/work/a', type: 'added' }],
				kind: 'update',
				rescan: [],
				root: null,
				sequence: 2,
				watch: null,
			},
			{ kind: 'gap' },
			{ kind: 'snapshot', root: root('/elsewhere'), rules, sequence: 3 },
		];
		expect(first).toEqual(expected);
		expect(second).toEqual(expected);
		expect(late).toEqual([{ kind: 'snapshot', root: root('/elsewhere'), rules, sequence: 1 }]);
		expect(connection.subscriptions).toHaveLength(1);
	});

	it('opens the root stream again after a restart and grants its files again', async () => {
		vi.useFakeTimers();
		const { connection, session } = openSession();
		const events = follow(session);
		await settle();
		connection.subscriptions[0]?.handlers.onFrame('snapshot', snapshot('/work'));
		const granting = session.grants.grant(
			'/outside/a.txt',
			'terminal-link',
			new AbortController().signal,
		);
		await settle();
		connection.answer('fs.grant', result({ entry: ENTRY }));
		expect((await granting).kind).toBe('ok');

		connection.subscriptions[0]?.handlers.onFrame(
			'result',
			result({ error: { code: 'unavailable', reason: 'restarting' } }),
		);
		expect(connection.subscriptions[0]?.closed).toBe(true);
		expect(events.at(-1)).toEqual({ kind: 'gap' });
		await vi.advanceTimersByTimeAsync(999);
		expect(connection.subscriptions).toHaveLength(1);
		await vi.advanceTimersByTimeAsync(1);
		await settle();
		expect(connection.subscriptions).toHaveLength(2);

		connection.subscriptions[1]?.handlers.onFrame('snapshot', snapshot('/work'));
		await settle();
		expect(events.at(-1)).toMatchObject({ kind: 'snapshot', sequence: 2 });
		expect(connection.answer('fs.grant', result({ entry: ENTRY }))).toMatchObject({
			path: '/outside/a.txt',
			source: 'terminal-link',
		});
		await settle();
		session.grants.ungrant(['/outside/a.txt']);
		await settle();
		expect(connection.pathsOf('fs.ungrant')).toEqual([['/outside/a.txt']]);
	});

	it('settles what was in flight when the connection ends and opens nothing again', async () => {
		vi.useFakeTimers();
		const { connection, session } = openSession();
		const events = follow(session);
		await settle();
		connection.subscriptions[0]?.handlers.onFrame('snapshot', snapshot('/work'));
		const signal = new AbortController().signal;
		const listing = session.tree.list({ path: '/work' }, signal);
		const reading = session.content.read({ limit: 1024, mode: 'text', path: '/work/a' }, signal);
		await settle();
		expect(connection.ops()).toEqual(['fs.list', 'fs.read', 'fs.read-chunk']);

		connection.end();
		expect(await listing).toEqual({ error: { code: 'remote-disconnected' }, kind: 'error' });
		expect(await reading).toEqual({ error: { code: 'remote-disconnected' }, kind: 'error' });
		expect(connection.closedLanes).toEqual([1n]);
		expect(events.at(-1)).toEqual({ kind: 'gap' });
		await vi.advanceTimersByTimeAsync(60_000);
		const sent = connection.sent.length;
		session.close();
		await settle();
		expect(connection.subscriptions).toHaveLength(1);
		expect(connection.sent).toHaveLength(sent);
	});

	it('lists a folder again only once its release has been answered', async () => {
		const { connection, session } = openSession();
		const signal = new AbortController().signal;
		const first = session.tree.list({ path: '/work/src' }, signal);
		await settle();
		connection.answer('fs.list', result({ entries: [] }));
		expect((await first).kind).toBe('ok');

		session.tree.release(['/work/src']);
		await settle();
		expect(connection.ops()).toEqual(['fs.release']);
		const again = session.tree.list({ path: '/work/src' }, signal);
		await settle();
		expect(connection.ops()).toEqual(['fs.release']);
		connection.answer('fs.release', result({}));
		await settle();
		expect(connection.ops()).toEqual(['fs.list']);
		connection.answer('fs.list', result({ entries: [] }));
		expect((await again).kind).toBe('ok');

		// A folder released while its listing runs is released once the listing lands.
		const unwanted = session.tree.list({ path: '/work/docs' }, signal);
		await settle();
		session.tree.release(['/work/docs']);
		connection.answer('fs.list', result({ entries: [] }));
		await unwanted;
		await settle();
		expect(connection.pathsOf('fs.release')).toEqual([['/work/src'], ['/work/docs']]);

		session.close();
		await settle();
		expect(connection.pathsOf('fs.release')).toEqual([
			['/work/src'],
			['/work/docs'],
			['/work/src'],
		]);
	});

	it('ungrants each grant once, including one that lands after its caller gave up', async () => {
		const { connection, session } = openSession();
		const abandoned = new AbortController();
		const giving = session.grants.grant('/outside/given-up', 'terminal-link', abandoned.signal);
		await settle();
		abandoned.abort();
		expect(await giving).toEqual({ error: { code: 'cancelled' }, kind: 'error' });
		connection.answer('fs.grant', result({ entry: ENTRY }));
		await settle();
		expect(connection.pathsOf('fs.ungrant')).toEqual([['/outside/given-up']]);

		const signal = new AbortController().signal;
		for (const granting of [
			session.grants.grant('/outside/twice', 'os-open', signal),
			session.grants.grant('/outside/twice', 'restored', signal),
		]) {
			await settle();
			connection.answer('fs.grant', result({ entry: ENTRY }));
			expect((await granting).kind).toBe('ok');
		}
		session.grants.ungrant(['/outside/twice']);
		const closing = session.grants.grant('/outside/closing', 'diff', signal);
		await settle();
		session.close();
		expect(await closing).toEqual({ error: { code: 'cancelled' }, kind: 'error' });
		connection.answer('fs.grant', result({ entry: ENTRY }));
		await settle();
		expect(connection.pathsOf('fs.ungrant')).toEqual([
			['/outside/given-up'],
			['/outside/twice'],
			['/outside/twice'],
			['/outside/closing'],
		]);
	});

	it('assembles a read in offset order whatever order its facts and chunks arrive in', async () => {
		const { connection, session } = openSession(true);
		const signal = new AbortController().signal;
		const facts = (size: number) =>
			result({ binary: false, bom: false, encoding: 'utf8', mtimeMs: 7, size });

		// The first chunk can answer before the read's facts.
		const small = session.content.read({ limit: 1024, mode: 'text', path: '/work/a' }, signal);
		await settle();
		const smallRead = connection.take('fs.read');
		connection.answer('fs.read-chunk', chunk(smallRead.request['transfer'], 0, 'hi\n', 'lf'));
		smallRead.resolve(facts(3));
		expect(await small).toEqual({
			kind: 'ok',
			value: {
				bom: false,
				changed: false,
				encoding: 'utf8',
				kind: 'text',
				lineEnding: 'lf',
				text: 'hi\n',
				version: { mtimeMs: 7, size: 3 },
			},
		});

		// A chunk past the end can fail before the last chunk arrives; it is never consumed.
		const full = 'a'.repeat(32_768);
		const exact = session.content.read({ limit: 65_536, mode: 'text', path: '/work/b' }, signal);
		await settle();
		const exactRead = connection.answer('fs.read', facts(32_768));
		await settle();
		const [head, past] = [connection.take('fs.read-chunk'), connection.take('fs.read-chunk')];
		expect([head.request['offset'], past.request['offset']]).toEqual(['0', '32768']);
		past.resolve(result({ error: { code: 'invalid-request' } }));
		head.resolve(chunk(exactRead['transfer'], 0, full, 'none'));
		expect(await exact).toMatchObject({ kind: 'ok', value: { text: full } });

		// Bytes arrive in any order.
		const bytes = session.content.read({ limit: 65_536, mode: 'bytes', path: '/work/c' }, signal);
		await settle();
		const bytesRead = connection.answer('fs.read', facts(32_770));
		await settle();
		const [start, rest] = [connection.take('fs.read-chunk'), connection.take('fs.read-chunk')];
		rest.resolve(chunk(bytesRead['transfer'], 32_768, 'zz', 'none'));
		start.resolve(chunk(bytesRead['transfer'], 0, full));
		const read = await bytes;
		expect(read.kind === 'ok' && read.value.kind === 'bytes' && read.value.bytes.byteLength).toBe(
			32_770,
		);
		expect(connection.closedLanes).toEqual([1n, 2n, 3n]);
	});

	it('reports a binary text read without the chunks the Server refused for it', async () => {
		const { connection, session } = openSession();
		const reading = session.content.read(
			{ limit: 1024, mode: 'text', path: '/work/image.png' },
			new AbortController().signal,
		);
		await settle();
		connection.answer('fs.read-chunk', result({ error: { code: 'invalid-request' } }));
		await settle();
		connection.answer(
			'fs.read',
			result({ binary: true, bom: false, encoding: null, mtimeMs: 7, size: 900 }),
		);
		expect(await reading).toEqual({
			kind: 'ok',
			value: { kind: 'binary', version: { mtimeMs: 7, size: 900 } },
		});
	});
});
