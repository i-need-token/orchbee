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
		reason: z.enum([
			'complete',
			'cancelled',
			'unsupported',
			'capacity',
			'invalid-request',
			'stopping',
			'outcome-unknown',
		]),
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

const readCorpus = async (name: string): Promise<unknown> =>
	JSON.parse(
		await readFile(
			new URL(`../../../../../../schemas/workbench/v1/${name}`, import.meta.url),
			'utf8',
		),
	);
/** The frozen base contract: what a carrier admits when its answer lists no capability. */
const corpus = corpusSchema.parse(await readCorpus('workbench-multiplex-conformance-v1.json'));
/** What a carrier whose answer lists `stream-outcome-unknown` admits besides the base corpus. */
const outcomeUnknownCorpus = corpusSchema
	.extend({ capability: z.literal('stream-outcome-unknown') })
	.parse(await readCorpus('workbench-multiplex-outcome-unknown-conformance-v1.json'));
/** What a carrier whose answer lists `files` admits besides the base corpus: OBFB records. */
const filesCorpus = corpusSchema
	.extend({ capability: z.literal('files') })
	.parse(await readCorpus('workbench-multiplex-files-conformance-v1.json'));
const OUTCOME_UNKNOWN = { files: false, streamOutcomeUnknown: true } as const;
/** `files` requires `stream-outcome-unknown`, so a carrier that admits it admits both. */
const FILES = { files: true, streamOutcomeUnknown: true } as const;

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

	it.each(outcomeUnknownCorpus.valid)(
		'agrees on $name only under its capability',
		({ hex: frame, message }) => {
			const expected: MultiplexMessage = message;
			expect(new MultiplexDecoder(OUTCOME_UNKNOWN).push(frame)).toEqual([expected]);
			expect(encodeMultiplexMessage(expected)).toEqual(frame);
			expect(() => new MultiplexDecoder().push(frame)).toThrow(
				expect.objectContaining({ name: 'MultiplexProtocolError' }),
			);
		},
	);

	it.each(outcomeUnknownCorpus.invalid)('refuses $name under its capability', ({ hex: frame }) => {
		expect(() => new MultiplexDecoder(OUTCOME_UNKNOWN).push(frame)).toThrow(
			expect.objectContaining({ name: 'MultiplexProtocolError' }),
		);
	});

	it.each(filesCorpus.valid)(
		'agrees on $name only under its capability',
		({ hex: frame, message }) => {
			const expected: MultiplexMessage = message;
			expect(new MultiplexDecoder(FILES).push(frame)).toEqual([expected]);
			expect(encodeMultiplexMessage(expected)).toEqual(frame);
			expect(() => new MultiplexDecoder(OUTCOME_UNKNOWN).push(frame)).toThrow(
				expect.objectContaining({ name: 'MultiplexProtocolError' }),
			);
		},
	);

	it.each(filesCorpus.invalid)('refuses $name under its capability', ({ hex: frame }) => {
		expect(() => new MultiplexDecoder(FILES).push(frame)).toThrow(
			expect.objectContaining({ name: 'MultiplexProtocolError' }),
		);
	});
});
