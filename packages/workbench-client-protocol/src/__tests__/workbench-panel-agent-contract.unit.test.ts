import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import {
	WorkbenchRequestSchema,
	WorkbenchResponseSchema,
} from './conformance/workbench-command-contract';
const caseSchema = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
describe('Panel Agent reports', () => {
	it('rejects unpaired title surrogates while admitting complete Unicode and explicit clear', () => {
		const document = (title: string | null) => ({
			kind: 'workbench.panel-agent.result',
			protocolVersion: 1,
			requestId: '1'.repeat(32),
			siteId: '2'.repeat(32),
			outcome: {
				kind: 'titles',
				workbenchRevision: '1',
				presentations: [
					{
						binding: {
							presentationId: '3'.repeat(32),
							generation: '4'.repeat(32),
							hostEpoch: '5'.repeat(32),
							hostTerminalId: '6'.repeat(32),
						},
						observation: { kind: 'observed', revision: '1', title },
					},
				],
			},
		});
		for (const title of ['⠋ 修复🙂标题', null]) {
			expect(WorkbenchResponseSchema.safeParse(document(title)).success).toBe(true);
		}
		expect(WorkbenchResponseSchema.safeParse(document('bad\ud800')).success).toBe(false);
	});

	it('matches the shared Rust report corpus and closed shapes', async () => {
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
							'../../../../schemas/workbench/v1/workbench-panel-agent-conformance-v1.json',
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
							'../../../../schemas/workbench/v1/workbench-panel-agent.schema.json',
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
			binding.parse(test.document);
			expect({ name: test.name, accepted: validate(test.document) }).toEqual({
				name: test.name,
				accepted: true,
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
