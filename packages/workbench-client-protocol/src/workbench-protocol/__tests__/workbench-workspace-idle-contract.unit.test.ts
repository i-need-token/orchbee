import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';

const caseSchema = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
const corpusSchema = z.strictObject({
	contractVersion: z.literal(1),
	valid: z.array(caseSchema),
	invalid: z.array(caseSchema),
});
const json = async (name: string) =>
	z
		.record(z.string(), z.unknown())
		.parse(
			JSON.parse(
				await readFile(
					new URL(`../../../../../schemas/workbench/v1/${name}`, import.meta.url),
					'utf8',
				),
			),
		);

describe('Workspace idle', () => {
	it('shares strict documents and bounds with the independent Rust binding', async () => {
		const corpus = corpusSchema.parse(await json('workbench-workspace-idle-conformance-v1.json'));
		const ajv = new Ajv2020({ allErrors: true, strict: false });
		ajv.addFormat(
			'orchbee-uint64',
			(value: string) =>
				/^(?:0|[1-9][0-9]{0,19})$/u.test(value) && BigInt(value) <= 18_446_744_073_709_551_615n,
		);
		ajv.addSchema(await json('workbench.schema.json'));
		const validate = ajv.compile(await json('workbench-workspace-idle.schema.json'));
		const binds = (test: z.infer<typeof caseSchema>) =>
			(test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema).safeParse(
				test.document,
			).success;
		const mismatches: string[] = [];
		for (const test of corpus.valid) {
			if (!binds(test)) mismatches.push(`${test.name}: binding refuses`);
			if (!validate(test.document)) mismatches.push(`${test.name}: schema refuses`);
		}
		for (const test of corpus.invalid) {
			if (binds(test)) mismatches.push(`${test.name}: binding admits`);
			if (test.boundary === 'schema' && validate(test.document)) {
				mismatches.push(`${test.name}: schema admits`);
			}
		}
		expect(mismatches).toEqual([]);
	});
});
