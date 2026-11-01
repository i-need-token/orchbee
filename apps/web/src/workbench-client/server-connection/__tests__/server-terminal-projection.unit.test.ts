import {
	AttachmentIdSchema,
	CURRENT_PROTOCOL_VERSION,
	CanonicalUint64TextSchema,
	MessageKind,
	type MessageKindValue,
	PayloadEncoding,
	encodeProtocolEnvelope,
	encodeSelectionExportChunk,
} from '@orchbee/terminal-protocol';
import type { ProjectedTerminalCommand } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { type ServerCarrier, ServerConnection } from '../server-connection';
import {
	type ServerTerminalProjection,
	openServerTerminalProjection,
} from '../server-terminal-projection';
import { MultiplexDecoder, encodeMultiplexMessage } from '../workbench-multiplex-codec';

const ATTACHMENT_ID = 'a'.repeat(32);
const CONTROLLER_ID = 'b'.repeat(32);
const CREATION_ID = 'c'.repeat(32);
const HOST_EPOCH = 'd'.repeat(32);
const TERMINAL_ID = 'e'.repeat(32);

interface ControlRequest {
	readonly action: {
		readonly inputKind?: string;
		readonly kind: string;
		readonly sequence?: string;
		readonly [field: string]: unknown;
	};
	readonly kind: string;
	readonly requestId: string;
}

type Outcome = Record<string, unknown>;

function encodeJsonFrame(value: object): Uint8Array {
	const payload = new TextEncoder().encode(JSON.stringify({ ...value, protocolVersion: 1 }));
	const frame = new Uint8Array(8 + payload.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(new TextEncoder().encode('OBCS'));
	view.setUint16(4, 1);
	view.setUint16(6, payload.byteLength);
	frame.set(payload, 8);
	return frame;
}

/** Answers OBCS requests over the real multiplex codec; a test scripts only the control replies. */
class ScriptedServer implements ServerCarrier {
	readonly controlRequests: ControlRequest[] = [];
	readonly #decoder = new MultiplexDecoder();
	readonly #answerControl: (request: ControlRequest) => Outcome;
	readonly #pollRecords: Uint8Array[] = [];
	/** Scripts a selection action's answer, which a test may hold back with a promise. */
	answerSelection: (request: ControlRequest) => Uint8Array | Promise<Uint8Array> = (request) =>
		encodeViewerResult(request.requestId, { kind: 'feedback-accepted' });
	polls = 0;
	readonly selectionRequests: ControlRequest[] = [];
	#receive: ((chunk: Uint8Array) => void) | null = null;

	constructor(answerControl: (request: ControlRequest) => Outcome) {
		this.#answerControl = answerControl;
	}

	close(): void {
		this.#receive = null;
	}

	enqueuePollRecord(record: Uint8Array): void {
		this.#pollRecords.push(record);
	}

	get pollRecordsWaiting(): number {
		return this.#pollRecords.length;
	}

	/** Hands the Host's record over the way a hint would announce it. */
	deliverPollRecord(record: Uint8Array): void {
		this.enqueuePollRecord(record);
		this.hintRecordReady();
	}

	hintRecordReady(): void {
		this.#receive?.(
			encodeMultiplexMessage({
				kind: 'viewer-record-ready',
				viewerId: BigInt(`0x${ATTACHMENT_ID}`),
			}),
		);
	}

	send(bytes: Uint8Array): void {
		for (const message of this.#decoder.push(bytes)) {
			if (message.kind !== 'stream-open') continue;
			const request = JSON.parse(
				new TextDecoder().decode(message.request.subarray(8)),
			) as ControlRequest;
			void Promise.resolve(this.#answer(request)).then((record) => {
				this.#receive?.(
					encodeMultiplexMessage({
						frameKind: 'result',
						kind: 'stream-frame',
						record,
						sequence: 1n,
						streamId: message.streamId,
					}),
				);
			});
		}
	}

	subscribe(receive: (chunk: Uint8Array) => void): () => void {
		this.#receive = receive;
		return () => {
			this.#receive = null;
		};
	}

	#answer(request: ControlRequest): Uint8Array | Promise<Uint8Array> {
		const { requestId } = request;
		if (request.action.kind.startsWith('selection-')) {
			this.selectionRequests.push(request);
			return this.answerSelection(request);
		}
		if (request.kind === 'terminal.control') {
			if (request.action.kind === 'acquire') {
				return encodeJsonFrame({
					kind: 'terminal.control.result',
					outcome: { controllerId: CONTROLLER_ID, kind: 'acquired' },
					requestId,
				});
			}
			this.controlRequests.push(request);
			return encodeJsonFrame({
				kind: 'terminal.control.result',
				outcome: this.#answerControl(request),
				requestId,
			});
		}
		if (request.action.kind === 'attach-binding') {
			return encodeJsonFrame({
				kind: 'terminal.viewer.result',
				outcome: {
					attachmentId: ATTACHMENT_ID,
					creationId: CREATION_ID,
					hostEpoch: HOST_EPOCH,
					hostTerminalId: TERMINAL_ID,
					kind: 'attached',
				},
				requestId,
			});
		}
		if (request.action.kind === 'poll') this.polls += 1;
		const record = request.action.kind === 'poll' ? this.#pollRecords.shift() : undefined;
		if (record !== undefined) return encodeViewerRecord(requestId, record);
		return encodeJsonFrame({
			kind: 'terminal.viewer.result',
			outcome: { kind: 'empty' },
			requestId,
		});
	}
}

function encodeViewerResult(requestId: string, outcome: Outcome): Uint8Array {
	return encodeJsonFrame({ kind: 'terminal.viewer.result', outcome, requestId });
}

/** The OBSR frame the Server sends for a poll that moved one Host record. */
function encodeViewerRecord(requestId: string, hostRecord: Uint8Array): Uint8Array {
	const frame = new Uint8Array(12 + 16 + hostRecord.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(new TextEncoder().encode('OBSR'));
	view.setUint16(4, 1);
	view.setUint32(8, 16 + hostRecord.byteLength);
	for (let index = 0; index < 16; index += 1) {
		frame[12 + index] = Number.parseInt(requestId.slice(index * 2, index * 2 + 2), 16);
	}
	frame.set(hostRecord, 28);
	return frame;
}

function hostEvent(
	kind: MessageKindValue,
	payloadEncoding: string,
	payload: Uint8Array,
): Uint8Array {
	const encoded = encodeProtocolEnvelope({
		correlationId: null,
		event: true,
		hostEpoch: HOST_EPOCH,
		kind,
		optionalMessage: false,
		payload,
		payloadEncoding,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: false,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

const SNAPSHOT = hostEvent(
	MessageKind.snapshotFrame,
	PayloadEncoding.terminalFrame,
	new Uint8Array([9, 9]),
);

function hostResponse(kind: MessageKindValue, value: object): Uint8Array {
	const encoded = encodeProtocolEnvelope({
		correlationId: 'f'.repeat(32),
		event: false,
		hostEpoch: HOST_EPOCH,
		kind,
		optionalMessage: false,
		payload: new TextEncoder().encode(JSON.stringify(value)),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: true,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function exportChunk(
	exportSequence: bigint,
	chunkSequence: number,
	text: string,
	final: boolean,
): Uint8Array {
	const encoded = encodeSelectionExportChunk({
		attachmentId: AttachmentIdSchema.parse(ATTACHMENT_ID),
		bytes: new TextEncoder().encode(text),
		chunkSequence,
		exportSequence,
		final,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return hostEvent(MessageKind.selectionExportChunk, PayloadEncoding.rawBytes, encoded.value);
}

function exportStarted(byteLength: number): Uint8Array {
	return hostResponse(MessageKind.selectionExportStarted, {
		attachmentId: ATTACHMENT_ID,
		byteLength,
		exportSequence: '1',
		format: 'plain',
	});
}

function exportFinished(status: 'cancelled' | 'completed', byteLength: number): Uint8Array {
	return hostEvent(
		MessageKind.selectionExportFinished,
		PayloadEncoding.controlJson,
		new TextEncoder().encode(
			JSON.stringify({ attachmentId: ATTACHMENT_ID, byteLength, exportSequence: '1', status }),
		),
	);
}

const REQUEST_SEQUENCE = CanonicalUint64TextSchema.parse('5');

function copyCommand(): ProjectedTerminalCommand {
	return {
		kind: 'selection-copy',
		payload: {
			attachmentId: AttachmentIdSchema.parse(ATTACHMENT_ID),
			requestSequence: REQUEST_SEQUENCE,
		},
	};
}

const GEOMETRY = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 80, rows: 24 };
const BINDING = { sessionId: 'a'.repeat(32), terminalId: 'b'.repeat(32), workerId: 'c'.repeat(32) };

async function open(server: ScriptedServer): Promise<{
	readonly events: { readonly kind: string; readonly [key: string]: unknown }[];
	readonly projection: ServerTerminalProjection;
}> {
	const projection = await openServerTerminalProjection(
		new ServerConnection(server),
		BINDING,
		GEOMETRY,
	);
	const events: { readonly kind: string; readonly [key: string]: unknown }[] = [];
	projection.transport.subscribe(
		(event) => events.push(event as (typeof events)[number]),
		() => events.push({ kind: 'unavailable' }),
	);
	return { events, projection };
}

function textInput(projection: ServerTerminalProjection, text: string): ProjectedTerminalCommand {
	return {
		kind: 'text-input',
		payload: {
			attachmentId: projection.bootstrap.attachmentId,
			capabilityRevision: projection.bootstrap.capabilityRevision,
			text,
		},
	};
}

async function settled(condition: () => boolean): Promise<void> {
	const deadline = Date.now() + 2_000;
	while (!condition()) {
		if (Date.now() > deadline) throw new Error('The condition did not settle in time.');
		await new Promise((resolve) => {
			setTimeout(resolve, 1);
		});
	}
}

/** What a healthy Server answers each control action with. */
function accepted(request: ControlRequest): Outcome {
	switch (request.action.kind) {
		case 'resize':
			return { geometry: GEOMETRY, kind: 'resized' };
		default:
			return { kind: 'input-accepted' };
	}
}

describe('openServerTerminalProjection', () => {
	it('retries a busy Server with new identities without letting later input overtake', async () => {
		let busyReplies = 2;
		const server = new ScriptedServer((request) => {
			if (request.action.inputKind === 'text' && busyReplies > 0) {
				busyReplies -= 1;
				return { kind: 'unavailable', reason: 'server-busy' };
			}
			return accepted(request);
		});
		const { events, projection } = await open(server);

		projection.transport.send(textInput(projection, 'a'));
		projection.transport.send(textInput(projection, 'b'));
		await settled(
			() => server.controlRequests.filter((r) => r.action.inputKind === 'text').length === 4,
		);

		const texts = server.controlRequests.filter((r) => r.action.inputKind === 'text');
		const sequences = texts.map((r) => BigInt(r.action.sequence ?? '0'));
		expect(sequences.every((value, index) => value > (sequences[index - 1] ?? 0n))).toBe(true);
		expect(new Set(texts.map((r) => r.requestId)).size).toBe(4);
		// The first three carry the same record; only the last one is the later input.
		const payloads = texts.map((r) => (r.action as { readonly payload?: string }).payload);
		expect(new Set(payloads.slice(0, 3)).size).toBe(1);
		expect(payloads[3]).not.toBe(payloads[0]);
		expect(events.filter((event) => event.kind === 'failure')).toEqual([]);
		projection.transport.close();
	});

	it('becomes read-only when another Client took over, without reporting a failure', async () => {
		const server = new ScriptedServer((request) =>
			request.action.inputKind === 'text'
				? { kind: 'rejected', reason: 'not-controller' }
				: accepted(request),
		);
		const { events, projection } = await open(server);
		await settled(() => server.controlRequests.some((r) => r.action.kind === 'resize'));

		projection.transport.send(textInput(projection, 'a'));
		await settled(() => events.some((e) => e.kind === 'resize-lease-changed'));
		projection.transport.send(textInput(projection, 'b'));
		await new Promise((resolve) => {
			setTimeout(resolve, 5);
		});

		const capabilityChange = events.find(
			(event) => event.kind === 'attachment-input-capability-changed',
		);
		expect(capabilityChange?.['capabilities']).toMatchObject({
			capabilities: { input: false, read: true, resize: false },
			capabilityRevision: '2',
			resizeLease: false,
		});
		expect(server.controlRequests.filter((r) => r.action.inputKind === 'text')).toHaveLength(1);
		expect(events.filter((event) => event.kind === 'failure')).toEqual([]);
		projection.transport.close();
	});

	it('answers every selection change, keeping a Host failure code the renderer acts on', async () => {
		const server = new ScriptedServer(accepted);
		const answers = [
			(): Uint8Array =>
				hostResponse(MessageKind.selectionChanged, {
					attachmentId: ATTACHMENT_ID,
					state: 'none',
				}),
			(): Uint8Array =>
				hostResponse(MessageKind.typedFailure, {
					code: 'stale-generation',
					message: 'SelectionGesture failed',
					retryable: true,
				}),
			(): Uint8Array =>
				hostResponse(MessageKind.selectionChanged, {
					attachmentId: 'f'.repeat(32),
					state: 'none',
				}),
		];
		server.answerSelection = (request) => {
			const answer = answers.shift();
			return answer === undefined
				? encodeViewerResult(request.requestId, { kind: 'rejected', reason: 'host-rejected' })
				: encodeViewerRecord(request.requestId, answer());
		};
		const { events, projection } = await open(server);
		const gesture: ProjectedTerminalCommand = {
			kind: 'selection-gesture',
			payload: {
				action: 'cancel',
				attachmentId: AttachmentIdSchema.parse(ATTACHMENT_ID),
				generation: CanonicalUint64TextSchema.parse('9'),
			},
		};

		for (const _ of [1, 2, 3, 4]) projection.transport.send(gesture);
		const selectionEvents = (): typeof events =>
			events.filter((event) => event.kind === 'selection-changed' || event.kind === 'failure');
		await settled(() => selectionEvents().length === 4);

		expect(server.selectionRequests[0]?.action).toEqual({
			action: 'cancel',
			attachmentId: ATTACHMENT_ID,
			generation: '9',
			kind: 'selection-gesture',
		});
		const failure = (code: string, retryable: boolean): object => ({
			failure: expect.objectContaining({ code, operation: 'selection', retryable }) as object,
			kind: 'failure',
		});
		expect(selectionEvents()).toEqual([
			{ event: { attachmentId: ATTACHMENT_ID, state: 'none' }, kind: 'selection-changed' },
			failure('stale-generation', true),
			failure('invalid-selection-response', false),
			failure('host-rejected', false),
		]);
		projection.transport.close();
	});

	it('assembles an export from polled chunks, even one that beats the export answer', async () => {
		const server = new ScriptedServer(accepted);
		let releaseStarted: (record: Uint8Array) => void = () => undefined;
		const acknowledged = (): readonly unknown[] =>
			server.selectionRequests
				.filter((request) => request.action.kind === 'selection-export-chunk-applied')
				.map((request) => request.action['chunkSequence']);
		server.answerSelection = (request) => {
			if (request.action.kind !== 'selection-export') {
				return encodeViewerResult(request.requestId, { kind: 'feedback-accepted' });
			}
			return new Promise((resolve) => {
				releaseStarted = (record) => resolve(encodeViewerRecord(request.requestId, record));
			});
		};
		const { events, projection } = await open(server);

		projection.transport.send(copyCommand());
		await settled(() => server.selectionRequests.length === 1);
		expect(server.selectionRequests[0]?.action).toEqual({
			attachmentId: ATTACHMENT_ID,
			exportSequence: '1',
			format: 'plain',
			kind: 'selection-export',
		});

		server.deliverPollRecord(exportChunk(1n, 0, 'he', false));
		await settled(() => server.pollRecordsWaiting === 0);
		await new Promise((resolve) => {
			setTimeout(resolve, 5);
		});
		expect(acknowledged()).toEqual([]);

		releaseStarted(exportStarted(5));
		await settled(() => acknowledged().length === 1);
		server.deliverPollRecord(exportChunk(1n, 1, 'llo', true));
		await settled(() => acknowledged().length === 2);
		expect(acknowledged()).toEqual([0, 1]);
		server.deliverPollRecord(exportFinished('completed', 5));
		await settled(() => events.some((event) => event.kind === 'selection-copy-content'));

		const delivered = events.find((event) => event.kind === 'selection-copy-content');
		expect(delivered?.['requestSequence']).toBe(REQUEST_SEQUENCE);
		expect(new TextDecoder().decode(delivered?.['content'] as ArrayBuffer)).toBe('hello');
		expect(events.filter((event) => event.kind === 'failure')).toEqual([]);
		projection.transport.close();
	});

	it('cancels an export a chunk broke, and then ignores what was already on its way', async () => {
		const server = new ScriptedServer(accepted);
		server.answerSelection = (request) =>
			request.action.kind === 'selection-export'
				? encodeViewerRecord(request.requestId, exportStarted(5))
				: encodeViewerResult(request.requestId, { kind: 'feedback-accepted' });
		const { events, projection } = await open(server);

		projection.transport.send(copyCommand());
		await settled(() => server.selectionRequests.length === 1);
		server.deliverPollRecord(exportChunk(1n, 7, 'hello', true));
		await settled(() => server.selectionRequests.length === 2);
		server.deliverPollRecord(exportFinished('cancelled', 5));
		await settled(() => server.pollRecordsWaiting === 0);
		await new Promise((resolve) => {
			setTimeout(resolve, 5);
		});

		expect(server.selectionRequests[1]?.action).toEqual({
			attachmentId: ATTACHMENT_ID,
			exportSequence: '1',
			kind: 'selection-export-cancel',
			reason: 'replaced',
		});
		const failures = events.filter((event) => event.kind === 'failure');
		expect(failures).toHaveLength(1);
		expect(failures[0]?.['failure']).toMatchObject({
			code: 'invalid-copy-chunk',
			operation: 'copy',
			requestSequence: REQUEST_SEQUENCE,
		});
		projection.transport.close();
	});

	it('reports an export another Client replaced as a copy that can be tried again', async () => {
		const server = new ScriptedServer(accepted);
		server.answerSelection = (request) => encodeViewerRecord(request.requestId, exportStarted(5));
		const { events, projection } = await open(server);

		projection.transport.send(copyCommand());
		await settled(() => server.selectionRequests.length === 1);
		server.deliverPollRecord(exportFinished('cancelled', 5));
		await settled(() => events.some((event) => event.kind === 'failure'));

		expect(events.find((event) => event.kind === 'failure')?.['failure']).toMatchObject({
			code: 'copy-cancelled',
			operation: 'copy',
			requestSequence: REQUEST_SEQUENCE,
			retryable: true,
		});
		projection.transport.close();
	});

	it('forwards a polled frame and reports a terminal outcome once', async () => {
		const server = new ScriptedServer(accepted);
		const exited = hostEvent(
			MessageKind.terminalExited,
			PayloadEncoding.controlJson,
			new TextEncoder().encode(
				JSON.stringify({
					exit: { code: 0, kind: 'exit-code' },
					finalRevision: '7',
					identity: { creationId: CREATION_ID, hostEpoch: HOST_EPOCH, terminalId: TERMINAL_ID },
				}),
			),
		);
		server.enqueuePollRecord(SNAPSHOT);
		server.enqueuePollRecord(exited);
		server.enqueuePollRecord(exited);

		const { events, projection } = await open(server);
		await settled(() => events.some((event) => event.kind === 'terminal-exited'));
		await new Promise((resolve) => {
			setTimeout(resolve, 5);
		});

		const received = events.find((event) => event.kind === 'frame')?.['frame'];
		expect(received).toEqual(new Uint8Array([9, 9]));
		expect(events.filter((event) => event.kind === 'terminal-exited')).toHaveLength(1);
		projection.transport.close();
	});

	it('takes no record before anyone is subscribed, because Poll would lose it', async () => {
		const server = new ScriptedServer(accepted);
		server.enqueuePollRecord(SNAPSHOT);
		const projection = await openServerTerminalProjection(
			new ServerConnection(server),
			BINDING,
			GEOMETRY,
		);

		server.hintRecordReady();
		await settled(() => server.controlRequests.some((r) => r.action.kind === 'resize'));
		await new Promise((resolve) => {
			setTimeout(resolve, 5);
		});
		expect(server.polls).toBe(0);

		const events: { readonly kind: string }[] = [];
		projection.transport.subscribe(
			(event) => events.push(event as (typeof events)[number]),
			() => undefined,
		);
		await settled(() => events.some((event) => event.kind === 'frame'));
		projection.transport.close();
	});

	it('follows hints once they have proved themselves, keeping a fallback for a lost one', async () => {
		vi.useFakeTimers();
		try {
			const server = new ScriptedServer(accepted);
			const { events, projection } = await open(server);
			const frames = (): number => events.filter((event) => event.kind === 'frame').length;
			const pollsDuring = async (milliseconds: number): Promise<number> => {
				const before = server.polls;
				await vi.advanceTimersByTimeAsync(milliseconds);
				return server.polls - before;
			};

			// No hint has arrived and a full frame is owed, so the short deadline runs.
			expect(await pollsDuring(80)).toBeGreaterThanOrEqual(5);

			// A hint polls at once, with no timer involved.
			server.enqueuePollRecord(SNAPSHOT);
			server.hintRecordReady();
			await vi.advanceTimersByTimeAsync(0);
			expect(frames()).toBe(1);

			// Hints now replace the short deadline; only the fallback finds a record whose hint was lost.
			server.enqueuePollRecord(SNAPSHOT);
			expect(await pollsDuring(249)).toBe(0);
			expect(frames()).toBe(1);
			await vi.advanceTimersByTimeAsync(1);
			expect(frames()).toBe(2);

			// Asking for another full frame brings the short deadline back until it arrives.
			projection.transport.send({
				kind: 'need-snapshot',
				payload: {
					attachmentId: projection.bootstrap.attachmentId,
					reason: 'decoder-rejected',
					sequence: CanonicalUint64TextSchema.parse('1'),
				},
			});
			expect(await pollsDuring(80)).toBeGreaterThanOrEqual(5);

			projection.transport.close();
		} finally {
			vi.useRealTimers();
		}
	});
});
