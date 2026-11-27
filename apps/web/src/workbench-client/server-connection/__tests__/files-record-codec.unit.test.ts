import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import { checkFilesRecord } from '../files-record-codec';

const hex = z.string().transform((value) => Uint8Array.from(Buffer.from(value, 'hex')));
const place = z.enum(['request-open', 'events-open', 'result-frame', 'notice-frame']);
/** The record cases of the File Bar corpus, which the Rust binding checks with the same bytes. */
const records = z
	.looseObject({
		capability: z.literal('files'),
		contractVersion: z.literal(1),
		records: z.strictObject({
			invalid: z.array(z.strictObject({ hex, name: z.string(), place })),
			valid: z.array(z.strictObject({ hex, name: z.string(), place, record: z.unknown() })),
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

describe('OBFB record codec', () => {
	it.each(records.valid)('admits $name where it travels', ({ hex: record, place: where }) => {
		expect(checkFilesRecord(record, where)).toBe(true);
	});

	it.each(records.invalid)('refuses $name', ({ hex: record, place: where }) => {
		expect(checkFilesRecord(record, where)).toBe(false);
	});
});
