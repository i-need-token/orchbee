import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';
const entry = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
it('reads a Workspace current directory across Rust, TypeScript and JSON shape boundaries', async () => {
	const read = async (name: string): Promise<unknown> =>
		JSON.parse(
			await readFile(
				new URL(`../../../../../schemas/workbench/v1/${name}`, import.meta.url),
				'utf8',
			),
		);
	const corpus = z
		.strictObject({ contractVersion: z.literal(1), valid: z.array(entry), invalid: z.array(entry) })
		.parse(await read('workbench-current-directory-conformance-v1.json'));
	const ajv = new Ajv2020({ allErrors: true, strict: true });
	ajv.addFormat('orchbee-uint64', {
		type: 'string',
		validate: (value) => /^(0|[1-9][0-9]*)$/u.test(value) && BigInt(value) <= 18446744073709551615n,
	});
	ajv.addSchema(z.record(z.string(), z.unknown()).parse(await read('workbench.schema.json')));
	const validate = ajv.compile(
		z.record(z.string(), z.unknown()).parse(await read('workbench-current-directory.schema.json')),
	);
	for (const [cases, accepted] of [
		[corpus.valid, true],
		[corpus.invalid, false],
	] as const)
		for (const test of cases) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect({ name: test.name, accepted: binding.safeParse(test.document).success }).toEqual({
				name: test.name,
				accepted,
			});
			expect({ name: test.name, accepted: validate(test.document) }).toEqual({
				name: test.name,
				accepted: accepted || test.boundary === 'semantic',
			});
		}
});
