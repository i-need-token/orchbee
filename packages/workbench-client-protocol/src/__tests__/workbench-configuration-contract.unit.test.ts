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

const profile = (name: string) =>
	JSON.stringify({
		rows: [
			[
				{
					name,
					style: { fg: null, bold: null, dim: null },
					rules: Array.from({ length: 4 }, () => ({
						condition: { kind: 'Equals', value: '\\'.repeat(3600) },
						ignore_case: false,
						style: { fg: null, bold: null, dim: null },
						hide: false,
					})),
				},
			],
		],
		row_gap: 0,
	});

describe('Site configuration publication', () => {
	it('fits two independently bounded escaped sidebar profiles in the shared frame', () => {
		const snapshot = {
			revision: '1',
			keysProfile: 'x'.repeat(20000),
			windowTitle: '',
			notificationProfile: '[ui.toast]',
			agentSidebarProfile: profile('agent'),
			spacesSidebarProfile: profile('workspace'),
			manifestCheck: true,
			defaultShell: '',
			shellMode: 'auto',
			restartRequired: [],
		};
		const document = {
			kind: 'workbench.configuration.result',
			protocolVersion: 1,
			requestId: '00000000000000000000000000000001',
			siteId: '00000000000000000000000000000002',
			outcome: { kind: 'current', snapshot },
		};
		expect(new TextEncoder().encode(JSON.stringify(document)).length).toBeGreaterThan(128 * 1024);
		expect(WorkbenchResponseSchema.safeParse(document).success).toBe(true);
		snapshot.spacesSidebarProfile = 'x'.repeat(32769);
		expect(WorkbenchResponseSchema.safeParse(document).success).toBe(false);
	});
	it('rejects unpaired Unicode before admitting a shell snapshot', () => {
		expect(
			WorkbenchResponseSchema.safeParse({
				kind: 'workbench.configuration.result',
				protocolVersion: 1,
				requestId: '00000000000000000000000000000001',
				siteId: '00000000000000000000000000000002',
				outcome: {
					kind: 'current',
					snapshot: {
						revision: '0',
						keysProfile: '[keys]',
						windowTitle: '',
						notificationProfile: '[ui.toast]',
						agentSidebarProfile: '{}',
						spacesSidebarProfile: '{}',
						manifestCheck: true,
						restartRequired: [],
						defaultShell: '\ud800',
						shellMode: 'auto',
					},
				},
			}).success,
		).toBe(false);
	});
	it('matches the shared Rust configuration corpus and closed shapes', async () => {
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
							'../../../../schemas/workbench/v1/workbench-configuration-conformance-v1.json',
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
							'../../../../schemas/workbench/v1/workbench-configuration.schema.json',
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
