import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	type FilesServerRecord,
	checkFilesRecord,
	decodeFilesRecord,
	encodeFilesRequestRecord,
} from '../files-record-codec';

const hex = z.string().transform((value) => Uint8Array.from(Buffer.from(value, 'hex')));
const place = z.enum(['request-open', 'events-open', 'result-frame', 'notice-frame']);
/** The record cases of the File Bar corpus, which the Rust binding checks with the same bytes. */
const records = z
	.looseObject({
		capability: z.literal('files'),
		contractVersion: z.literal(1),
		records: z.strictObject({
			invalid: z.array(z.strictObject({ hex, name: z.string(), place })),
			valid: z.array(
				z.strictObject({
					hex,
					name: z.string(),
					place,
					record: z.looseObject({ json: z.string().optional(), kind: z.string() }),
				}),
			),
		}),
	})
	.parse(
		JSON.parse(
			await readFile(
				new URL('../../../../../../schemas/files/v1/files-conformance-v1.json', import.meta.url),
				'utf8',
			),
		),
	).records;
const utf8 = new TextDecoder();

/** A decoded record in the corpus's spelling: u64 values as decimal text and bytes as hex. */
function spell(record: FilesServerRecord): unknown {
	if (record.kind !== 'chunk') return { json: utf8.decode(record.body), kind: record.kind };
	const { data, end, offset, transfer } = record.chunk;
	return {
		data: Buffer.from(data).toString('hex'),
		end:
			end === null
				? null
				: {
						changed: end.changed,
						digest: end.digest === null ? null : Buffer.from(end.digest).toString('hex'),
						lineEnding: end.lineEnding,
					},
		kind: 'chunk',
		offset: offset.toString(),
		transfer: transfer.toString(),
	};
}

describe('OBFB record codec', () => {
	it.each(records.valid)('admits $name where it travels', ({ hex: record, place: where }) => {
		expect(checkFilesRecord(record, where)).toBe(true);
	});

	it.each(records.invalid)('refuses $name', ({ hex: record, place: where }) => {
		expect(checkFilesRecord(record, where)).toBe(false);
	});

	it.each(
		records.valid.filter(
			({ place: where }) => where === 'result-frame' || where === 'notice-frame',
		),
	)('decodes $name as the corpus states', ({ hex: record, place: where, record: decoded }) => {
		const read = decodeFilesRecord(
			record,
			where === 'result-frame' ? 'result-frame' : 'notice-frame',
		);

		expect(read === null ? null : spell(read)).toEqual(decoded);
	});

	it.each(records.valid.filter(({ record }) => record.kind === 'request'))(
		'frames $name to its bytes',
		({ hex: record, record: { json } }) => {
			expect(encodeFilesRequestRecord(new TextEncoder().encode(json))).toEqual(record);
		},
	);
});
