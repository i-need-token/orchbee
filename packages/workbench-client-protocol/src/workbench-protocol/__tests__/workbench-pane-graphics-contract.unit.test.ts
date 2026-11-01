import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';
const entry = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	boundary: z.enum(['shape', 'semantic']).optional(),
	document: z.unknown(),
});
const read = async (name: string) =>
	JSON.parse(
		await readFile(
			new URL(`../../../../../schemas/workbench/v1/${name}.json`, import.meta.url),
			'utf8',
		),
	) as unknown;
it('shares graphics image, resource and stream shapes with Rust and JSON Schema', async () => {
	const corpus = z
		.strictObject({ contractVersion: z.literal(1), valid: z.array(entry), invalid: z.array(entry) })
		.parse(await read('workbench-pane-graphics-conformance-v1'));
	const ajv = new Ajv2020({ strict: true, allErrors: true });
	ajv.addFormat('orchbee-uint64', {
		type: 'string',
		validate: (value) => /^(0|[1-9][0-9]*)$/u.test(value) && BigInt(value) <= 18446744073709551615n,
	});
	const referenced = await Promise.all(
		['workbench', 'workbench-client-view'].map((name) => read(name + '.schema')),
	);
	for (const schema of referenced) ajv.addSchema(z.record(z.string(), z.unknown()).parse(schema));
	const validate = ajv.compile(
		z.record(z.string(), z.unknown()).parse(await read('workbench-pane-graphics.schema')),
	);
	for (const [cases, accepted] of [
		[corpus.valid, true],
		[corpus.invalid, false],
	] as const)
		for (const sample of cases) {
			const binding =
				sample.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect({ name: sample.name, accepted: binding.safeParse(sample.document).success }).toEqual({
				name: sample.name,
				accepted,
			});
			expect({ name: sample.name, accepted: validate(sample.document) }).toEqual({
				name: sample.name,
				accepted: accepted || sample.boundary === 'semantic',
			});
		}
});
