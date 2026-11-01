import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	MultiplexDecoder,
	type MultiplexMessage,
	encodeMultiplexMessage,
} from '../workbench-multiplex-codec';

const decimal = z.string().transform((value) => BigInt(value));
const hex = z.string().transform((value) => Uint8Array.from(Buffer.from(value, 'hex')));
const messageSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('stream-open'),
		laneId: decimal,
		mode: z.enum(['request', 'state', 'events', 'invalidations']),
		request: hex,
		streamId: decimal,
	}),
	z.strictObject({
		frameKind: z.enum(['result', 'snapshot', 'update']),
		kind: z.literal('stream-frame'),
		record: hex,
		sequence: decimal,
		streamId: decimal,
	}),
	z.strictObject({
		kind: z.literal('stream-close'),
		reason: z.enum(['complete', 'cancelled', 'unsupported', 'capacity', 'invalid-request']),
		streamId: decimal,
	}),
	z.strictObject({ kind: z.literal('gap'), streamId: decimal, through: decimal }),
	z.strictObject({
		kind: z.literal('disconnect'),
		reason: z.enum(['slow-consumer', 'capacity', 'invalid-stream', 'unavailable']),
	}),
	z.strictObject({ kind: z.literal('resnapshot'), streamId: decimal }),
	z.strictObject({ kind: z.literal('lane-open'), laneId: decimal }),
	z.strictObject({ kind: z.literal('lane-close'), laneId: decimal }),
	z.strictObject({ kind: z.literal('viewer-record-ready'), viewerId: decimal }),
]);
const corpusSchema = z.strictObject({
	contractVersion: z.literal(1),
	invalid: z.array(z.strictObject({ hex, name: z.string() })),
	valid: z.array(z.strictObject({ hex, message: messageSchema, name: z.string() })),
});

const corpus = corpusSchema.parse(
	JSON.parse(
		await readFile(
			new URL(
				'../../../../../../schemas/workbench/v1/workbench-multiplex-conformance-v1.json',
				import.meta.url,
			),
			'utf8',
		),
	),
);

describe('OBWP stream codec', () => {
	it.each(corpus.valid)('agrees with the corpus on $name', ({ hex: frame, message }) => {
		const expected: MultiplexMessage = message;
		expect(new MultiplexDecoder().push(frame)).toEqual([expected]);
		expect(encodeMultiplexMessage(expected)).toEqual(frame);
	});

	it.each(corpus.invalid)('refuses $name', ({ hex: frame }) => {
		expect(() => new MultiplexDecoder().push(frame)).toThrow(
			expect.objectContaining({ name: 'MultiplexProtocolError' }),
		);
	});

	it('reassembles messages from arbitrary carrier chunks', () => {
		const frames = corpus.valid.map(({ hex: frame }) => frame);
		const stream = Buffer.concat(frames);
		const decoder = new MultiplexDecoder();
		const decoded: MultiplexMessage[] = [];
		for (
			let offset = 0, size = 1;
			offset < stream.byteLength;
			offset += size, size = (size % 7) + 1
		) {
			decoded.push(...decoder.push(stream.subarray(offset, offset + size)));
		}
		expect(decoder.partial).toBe(false);
		expect(decoded).toEqual(corpus.valid.map(({ message }) => message));
	});
});
