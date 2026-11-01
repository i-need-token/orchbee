import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	type ClientServerRequest,
	decodeClientServerResponse,
	encodeClientServerPaste,
	encodeClientServerRequest,
} from '../client-server-carrier';

const BOUND_RESULTS = new Set([
	'terminal.create.result',
	'terminal.viewer.result',
	'terminal.control.result',
]);
const messageSchema = z.looseObject({
	boundary: z.string().optional(),
	direction: z.string(),
	name: z.string(),
	payload: z.string(),
});
const corpus = z
	.looseObject({ invalidMessages: z.array(messageSchema), validMessages: z.array(messageSchema) })
	.parse(
		JSON.parse(
			await readFile(
				new URL(
					'../../../../../../schemas/client-server/v1/client-server-conformance-v1.json',
					import.meta.url,
				),
				'utf8',
			),
		),
	);
const schema: unknown = JSON.parse(
	await readFile(
		new URL(
			'../../../../../../schemas/client-server/v1/client-server.schema.json',
			import.meta.url,
		),
		'utf8',
	),
);
const validateMessage = new Ajv2020({ strict: false, validateFormats: false }).compile(
	z.record(z.string(), z.unknown()).parse(schema),
);

function boundResult(payload: string): boolean {
	try {
		const { kind } = z.looseObject({ kind: z.string() }).parse(JSON.parse(payload));
		return BOUND_RESULTS.has(kind);
	} catch {
		return false;
	}
}
const encoder = new TextEncoder();

/** Frames one corpus payload as a Server would, so it reaches the decoder a connection uses. */
function admitsResponse(payload: string): boolean {
	const body = encoder.encode(payload);
	const frame = new Uint8Array(8 + body.byteLength);
	const view = new DataView(frame.buffer);
	frame.set([0x4f, 0x42, 0x43, 0x53]); // OBCS
	view.setUint16(4, 1);
	view.setUint16(6, body.byteLength);
	frame.set(body, 8);
	try {
		decodeClientServerResponse(frame);
		return true;
	} catch {
		return false;
	}
}
const ID = 'a'.repeat(32);
const ids = { attachmentId: ID, controllerId: 'b'.repeat(32), sequence: '1' };
const geometry = { cellPixelHeight: 20, cellPixelWidth: 10, columns: 80, rows: 24 };

describe('Client-Server carrier binding', () => {
	const valid = corpus.validMessages.filter(
		(message) => message.direction === 'server-to-client' && boundResult(message.payload),
	);
	const invalid = corpus.invalidMessages.filter(
		(message) =>
			message.boundary === 'schema' &&
			message.direction === 'server-to-client' &&
			boundResult(message.payload),
	);

	it('covers the corpus', () => {
		expect(valid.length).toBeGreaterThan(10);
		expect(invalid.length).toBeGreaterThan(10);
	});

	it.each(valid)('admits $name', ({ payload }) => {
		expect(admitsResponse(payload)).toBe(true);
	});

	it.each(invalid)('refuses $name', ({ payload }) => {
		expect(admitsResponse(payload)).toBe(false);
	});

	it.each<ClientServerRequest>([
		{
			cwd: null,
			kind: 'terminal.create',
			requestId: ID,
			startMode: { geometry, kind: 'immediate' },
			workerConstraint: null,
		},
		{
			action: {
				kind: 'attach-binding',
				sessionId: ID,
				terminalId: ID,
				workerId: ID,
			},
			kind: 'terminal.viewer',
			requestId: ID,
		},
		{ action: { attachmentId: ID, kind: 'poll' }, kind: 'terminal.viewer', requestId: ID },
		{
			action: {
				attachmentId: ID,
				generation: '1',
				kind: 'frame-applied',
				revision: '1',
				sequence: '1',
			},
			kind: 'terminal.viewer',
			requestId: ID,
		},
		{
			action: {
				attachmentId: ID,
				detail: 'render',
				generation: '1',
				kind: 'read-viewport-rows',
				maximumBytes: 112,
				origin: { kind: 'tail', rowsBefore: 0 },
				requestSequence: '1',
				revision: '1',
				rowCount: 1,
			},
			kind: 'terminal.viewer',
			requestId: ID,
		},
		{ action: { attachmentId: ID, kind: 'acquire' }, kind: 'terminal.control', requestId: ID },
		{ action: { attachmentId: ID, kind: 'takeover' }, kind: 'terminal.control', requestId: ID },
		{ action: { attachmentId: ID, kind: 'clear' }, kind: 'terminal.control', requestId: ID },
		{
			action: { ...ids, inputKind: 'key', kind: 'semantic-input', payload: '00ff' },
			kind: 'terminal.control',
			requestId: ID,
		},
		{
			action: { ...ids, geometry, kind: 'resize' },
			kind: 'terminal.control',
			requestId: ID,
		},
	])('encodes a request the schema admits: $kind $action.kind', (request) => {
		const frame = encodeClientServerRequest(request);
		expect(new TextDecoder().decode(frame.subarray(0, 4))).toBe('OBCS');
		const document: unknown = JSON.parse(new TextDecoder().decode(frame.subarray(8)));
		validateMessage(document);
		expect(validateMessage.errors ?? []).toEqual([]);
		expect(new DataView(frame.buffer).getUint16(6)).toBe(frame.byteLength - 8);
	});

	it('lays out a paste record for the Server', () => {
		const record = encodeClientServerPaste({
			attachmentId: 'a'.repeat(32),
			content: Uint8Array.of(1, 2),
			controllerId: 'b'.repeat(32),
			requestId: 'c'.repeat(32),
			sequence: 7n,
		});
		expect(Buffer.from(record).toString('hex')).toBe(
			`4f424351000100000000003a${'c'.repeat(32)}${'a'.repeat(32)}${'b'.repeat(32)}00000000000000070102`,
		);
	});

	it('decodes a Host record and refuses an unknown one', () => {
		const hostRecord = new Uint8Array(64).fill(9);
		const framed = new Uint8Array(12 + 16 + 64);
		framed.set([0x4f, 0x42, 0x53, 0x52, 0, 1, 0, 0, 0, 0, 0, 80]);
		framed.fill(0xcc, 12, 28);
		framed.set(hostRecord, 28);
		expect(decodeClientServerResponse(framed)).toEqual({
			hostRecord,
			kind: 'terminal.viewer.record',
			requestId: 'cc'.repeat(16),
		});
		expect(() => decodeClientServerResponse(Uint8Array.of(1, 2, 3, 4))).toThrow(
			'Unknown response record.',
		);
	});
});
