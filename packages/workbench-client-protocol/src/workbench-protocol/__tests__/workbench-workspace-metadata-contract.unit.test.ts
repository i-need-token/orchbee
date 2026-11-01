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
describe('Workspace metadata', () => {
	it('matches the shared Rust corpus and preserves every legal token key', async () => {
		const corpus = z
			.strictObject({
				contractVersion: z.literal(1),
				valid: z.array(caseSchema),
				invalid: z.array(caseSchema),
			})
			.parse(
				JSON.parse(
					await readFile(
						new URL(
							'../../../../../schemas/workbench/v1/workbench-workspace-metadata-conformance-v1.json',
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
							'../../../../../schemas/workbench/v1/workbench-workspace-metadata.schema.json',
							import.meta.url,
						),
						'utf8',
					),
				),
			);
		const ajv = new Ajv2020({ allErrors: true, strict: true });
		ajv.addFormat('orchbee-uint64', {
			type: 'string',
			validate: (value) =>
				/^(0|[1-9][0-9]*)$/u.test(value) && BigInt(value) <= 18446744073709551615n,
		});
		const validate = ajv.compile(schema);
		for (const test of corpus.valid) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			const parsed = binding.parse(test.document);
			expect({ name: test.name, accepted: validate(test.document) }).toEqual({
				name: test.name,
				accepted: true,
			});
			// Null optional fields normalize to omission, and all token properties survive.
			const canonical = z.record(z.string(), z.unknown()).parse(test.document);
			if (canonical['action'] !== undefined) {
				const action = z.record(z.string(), z.unknown()).parse(canonical['action']);
				if (action['sequence'] === null) delete action['sequence'];
				if (action['ttlMilliseconds'] === null) delete action['ttlMilliseconds'];
				canonical['action'] = action;
			}
			expect({ name: test.name, document: JSON.parse(JSON.stringify(parsed)) }).toEqual({
				name: test.name,
				document: canonical,
			});
		}
		for (const test of corpus.invalid) {
			const binding =
				test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect({ name: test.name, accepted: binding.safeParse(test.document).success }).toEqual({
				name: test.name,
				accepted: false,
			});
		}
		for (const test of corpus.invalid.filter((entry) => entry.boundary === 'schema')) {
			expect({ name: test.name, accepted: validate(test.document) }).toEqual({
				name: test.name,
				accepted: false,
			});
		}
	});
});
