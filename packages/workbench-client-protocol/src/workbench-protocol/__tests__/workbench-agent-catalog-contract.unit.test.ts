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

describe('Agent catalog', () => {
	it('shares strict documents and semantic failures with the independent Rust binding', async () => {
		const corpus = corpusSchema.parse(await json('workbench-agent-catalog-conformance-v1.json'));
		const ajv = new Ajv2020({ allErrors: true, strict: false });
		ajv.addFormat(
			'orchbee-uint64',
			(value: string) =>
				/^(?:0|[1-9][0-9]{0,19})$/u.test(value) && BigInt(value) <= 18_446_744_073_709_551_615n,
		);
		for (const name of [
			'workbench.schema.json',
			'workbench-client-view.schema.json',
			'workbench-pane-graphics.schema.json',
			'workbench-command.schema.json',
		]) {
			ajv.addSchema(await json(name));
		}
		const validate = ajv.compile(await json('workbench-agent-catalog.schema.json'));
		for (const test of corpus.valid) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect(binding.safeParse(test.document).success, test.name).toBe(true);
			expect(validate(test.document), test.name).toBe(true);
		}
		for (const test of corpus.invalid) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect(binding.safeParse(test.document).success, test.name).toBe(false);
		}
		for (const test of corpus.invalid.filter((entry) => entry.boundary === 'schema')) {
			expect(validate(test.document), test.name).toBe(false);
		}
		for (const test of corpus.invalid.filter((entry) => entry.boundary === 'semantic')) {
			expect(validate(test.document), test.name).toBe(true);
		}
	});
});
