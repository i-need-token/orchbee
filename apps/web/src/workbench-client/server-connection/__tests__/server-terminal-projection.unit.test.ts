import {
	CURRENT_PROTOCOL_VERSION,
	MessageKind,
	type MessageKindValue,
	PayloadEncoding,
	encodeProtocolEnvelope,
} from '@orchbee/terminal-protocol';
import type { ProjectedTerminalCommand } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

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

	send(bytes: Uint8Array): void {
		for (const message of this.#decoder.push(bytes)) {
			if (message.kind !== 'stream-open') continue;
			const request = JSON.parse(
				new TextDecoder().decode(message.request.subarray(8)),
			) as ControlRequest;
			queueMicrotask(() => {
				this.#receive?.(
					encodeMultiplexMessage({
						frameKind: 'result',
						kind: 'stream-frame',
						record: this.#answer(request),
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

	#answer(request: ControlRequest): Uint8Array {
		const { requestId } = request;
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
		if (request.action.kind === 'attach') {
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
		const record = request.action.kind === 'poll' ? this.#pollRecords.shift() : undefined;
		if (record !== undefined) return encodeViewerRecord(requestId, record);
		return encodeJsonFrame({
			kind: 'terminal.viewer.result',
			outcome: { kind: 'empty' },
			requestId,
		});
	}
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

const THEME = {
	colorScheme: 'dark',
	defaultBackgroundRgb: 0x10_10_10,
	defaultCursorRgb: null,
	defaultForegroundRgb: 0xe0_e0_e0,
	palette: Array.from({ length: 256 }, (_, index) => index),
} as const;

const GEOMETRY = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 80, rows: 24 };

async function open(server: ScriptedServer): Promise<{
	readonly events: { readonly kind: string; readonly [key: string]: unknown }[];
	readonly projection: ServerTerminalProjection;
}> {
	const projection = await openServerTerminalProjection(
		new ServerConnection(server),
		{ kind: 'created', terminalCreateRequestId: 'f'.repeat(32) },
		GEOMETRY,
		{ ...THEME, palette: [...THEME.palette] },
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
		case 'host-colors':
			return { kind: 'host-colors-accepted' };
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
		await settled(() => server.controlRequests.length >= 2);

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
		server.enqueuePollRecord(
			hostEvent(MessageKind.snapshotFrame, PayloadEncoding.terminalFrame, new Uint8Array([9, 9])),
		);
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
});
