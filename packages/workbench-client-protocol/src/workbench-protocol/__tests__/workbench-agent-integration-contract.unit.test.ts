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
const read = async (path: string): Promise<unknown> =>
	JSON.parse(await readFile(new URL(`../../../../../schemas/${path}`, import.meta.url), 'utf8'));
it('runs Agent integration jobs across Rust, TypeScript and JSON shape boundaries', async () => {
	const corpus = z
		.strictObject({ contractVersion: z.literal(1), valid: z.array(entry), invalid: z.array(entry) })
		.parse(await read('workbench/v1/workbench-agent-integration-conformance-v1.json'));
	const ajv = new Ajv2020({ allErrors: true, strict: true });
	ajv.addFormat('orchbee-uint64', {
		type: 'string',
		validate: (value) => /^(0|[1-9][0-9]*)$/u.test(value) && BigInt(value) <= 18446744073709551615n,
	});
	const references = await Promise.all(
		[
			'workbench/v1/workbench.schema.json',
			'agent-integration/v1/agent-integration.schema.json',
		].map(read),
	);
	for (const schema of references) ajv.addSchema(z.record(z.string(), z.unknown()).parse(schema));
	const validate = ajv.compile(
		z
			.record(z.string(), z.unknown())
			.parse(await read('workbench/v1/workbench-agent-integration.schema.json')),
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
