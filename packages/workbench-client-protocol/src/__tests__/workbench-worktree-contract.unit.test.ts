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
	validJson: z.array(
		z.strictObject({
			name: z.string(),
			direction: z.enum(['request', 'response']),
			json: z.string(),
			canonical: z.unknown(),
		}),
	),
});

describe('retained Worktree operations', () => {
	it('shares strict documents and semantic failures with the independent Rust binding', async () => {
		const corpus = corpusSchema.parse(
			JSON.parse(
				await readFile(
					new URL(
						'../../../../schemas/workbench/v1/workbench-worktree-conformance-v1.json',
						import.meta.url,
					),
					'utf8',
				),
			),
		);
		const schema = z
			.record(z.string(), z.unknown())
			.parse(
				JSON.parse(
					await readFile(
						new URL(
							'../../../../schemas/workbench/v1/workbench-worktree.schema.json',
							import.meta.url,
						),
						'utf8',
					),
				),
			);
		const validate = new Ajv2020({ allErrors: true, strict: false }).compile(schema);
		for (const test of corpus.valid) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect(binding.safeParse(test.document).success).toBe(true);
			expect(validate(test.document)).toBe(true);
		}
		for (const test of corpus.invalid) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect(binding.safeParse(test.document).success).toBe(false);
		}
		for (const test of corpus.invalid.filter((entry) => entry.boundary === 'schema')) {
			expect(validate(test.document)).toBe(false);
		}
		for (const test of corpus.validJson) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect(binding.parse(JSON.parse(test.json))).toEqual(test.canonical);
		}
	});
});
