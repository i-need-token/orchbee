import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { expect, it } from 'vitest';
import { z } from 'zod';
import {
	WorkbenchRequestSchema,
	WorkbenchResponseSchema,
} from '../workbench-command-contract';

const read = async (name: string): Promise<unknown> =>
	JSON.parse(
		await readFile(
			new URL(`../../../../../schemas/workbench/v1/${name}.json`, import.meta.url),
			'utf8',
		),
	);
it('shares closed common-setting saves and uncertain outcomes with Rust and JSON Schema', async () => {
	const entry = z.strictObject({
		name: z.string(),
		direction: z.enum(['request', 'response']),
		document: z.unknown(),
	});
	const corpus = z
		.strictObject({ contractVersion: z.literal(1), valid: z.array(entry), invalid: z.array(entry) })
		.parse(await read('workbench-settings-conformance-v1'));
	const ajv = new Ajv2020({ strict: true, allErrors: true });
	ajv.addFormat('orchbee-uint64', {
		type: 'string',
		validate: (value) => /^(0|[1-9][0-9]*)$/u.test(value) && BigInt(value) <= 18446744073709551615n,
	});
	ajv.addSchema(z.record(z.string(), z.unknown()).parse(await read('user-preferences.schema')));
	const validate = ajv.compile(
		z.record(z.string(), z.unknown()).parse(await read('workbench-settings.schema')),
	);
	for (const [cases, accepted] of [
		[corpus.valid, true],
		[corpus.invalid, false],
	] as const) {
		for (const sample of cases) {
			const binding =
				sample.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect({ name: sample.name, accepted: binding.safeParse(sample.document).success }).toEqual({
				name: sample.name,
				accepted,
			});
			expect({ name: sample.name, accepted: validate(sample.document) }).toEqual({
				name: sample.name,
				accepted,
			});
		}
	}
});
