import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import {
	WorkbenchRequestSchema,
	WorkbenchResponseSchema,
} from './conformance/workbench-command-contract';
import { ConfigurationDiagnosticsSchema } from './conformance/workbench-configuration-contract';
const caseSchema = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
const NOTIFICATION_PROFILE_BYTES = 16 * 1024;

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
const configurationResult = (notificationProfile: string) => ({
	kind: 'workbench.configuration.result',
	protocolVersion: 1,
	requestId: '00000000000000000000000000000001',
	sessionId: '00000000000000000000000000000002',
	outcome: {
		kind: 'current',
		snapshot: {
			revision: '1',
			diagnostics: { items: [], omittedCount: 0 },
			keysProfile: '[keys]',
			windowTitle: '',
			notificationProfile,
			agentSidebarProfile: '{}',
			spacesSidebarProfile: '{}',
			manifestCheck: true,
			focusNewPanel: true,
			worktreesDirectory: null,
			newCwd: 'follow',
		},
	},
});

describe('Session configuration publication', () => {
	it('admits exactly 16 KiB of ASCII notification profile and rejects the next byte', () => {
		expect(
			WorkbenchResponseSchema.safeParse(configurationResult('x'.repeat(NOTIFICATION_PROFILE_BYTES)))
				.success,
		).toBe(true);
		expect(
			WorkbenchResponseSchema.safeParse(
				configurationResult('x'.repeat(NOTIFICATION_PROFILE_BYTES + 1)),
			).success,
		).toBe(false);
	});
	it('uses UTF-8 bytes rather than JavaScript string length for notification profiles', () => {
		const within = '界'.repeat(5461);
		const over = '界'.repeat(5462);
		expect(within.length).toBeLessThan(NOTIFICATION_PROFILE_BYTES);
		expect(over.length).toBeLessThan(NOTIFICATION_PROFILE_BYTES);
		expect(new TextEncoder().encode(within)).toHaveLength(16383);
		expect(new TextEncoder().encode(over)).toHaveLength(16386);
		expect(WorkbenchResponseSchema.safeParse(configurationResult(within)).success).toBe(true);
		expect(WorkbenchResponseSchema.safeParse(configurationResult(over)).success).toBe(false);
	});
	it('fits two independently bounded escaped sidebar profiles in the shared frame', () => {
		const snapshot = {
			revision: '1',
			diagnostics: { items: [], omittedCount: 0 },
			keysProfile: 'x'.repeat(20000),
			windowTitle: '',
			notificationProfile: '[ui.toast]',
			agentSidebarProfile: profile('agent'),
			spacesSidebarProfile: profile('workspace'),
			manifestCheck: true,
			focusNewPanel: true,
			worktreesDirectory: null,
			newCwd: 'follow',
		};
		const document = {
			kind: 'workbench.configuration.result',
			protocolVersion: 1,
			requestId: '00000000000000000000000000000001',
			sessionId: '00000000000000000000000000000002',
			outcome: { kind: 'current', snapshot },
		};
		expect(new TextEncoder().encode(JSON.stringify(document)).length).toBeGreaterThan(128 * 1024);
		expect(WorkbenchResponseSchema.safeParse(document).success).toBe(true);
		snapshot.spacesSidebarProfile = 'x'.repeat(32769);
		expect(WorkbenchResponseSchema.safeParse(document).success).toBe(false);
	});
	it('rejects unpaired Unicode before admitting a snapshot', () => {
		expect(
			WorkbenchResponseSchema.safeParse({
				kind: 'workbench.configuration.result',
				protocolVersion: 1,
				requestId: '00000000000000000000000000000001',
				sessionId: '00000000000000000000000000000002',
				outcome: {
					kind: 'current',
					snapshot: {
						revision: '0',
						diagnostics: { items: [], omittedCount: 0 },
						keysProfile: '[keys]',
						windowTitle: '\ud800',
						notificationProfile: '[ui.toast]',
						agentSidebarProfile: '{}',
						spacesSidebarProfile: '{}',
						manifestCheck: true,
						focusNewPanel: true,
						worktreesDirectory: null,
						newCwd: 'follow',
					},
				},
			}).success,
		).toBe(false);
	});
	it('bounds and canonically orders configuration diagnostics by Unicode scalar value', () => {
		const entry = (field: string, reason = 'wrong-type') => ({
			field,
			reason,
			disposition: 'defaulted' as const,
		});
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry('\ue000'), entry('\u{10000}')],
				omittedCount: 65536,
			}).success,
		).toBe(true);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry('\u{10000}'), entry('\ue000')],
				omittedCount: 0,
			}).success,
		).toBe(false);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry('keys.help', 'invalid-binding'), entry('keys.help', 'wrong-type')],
				omittedCount: 0,
			}).success,
		).toBe(true);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry('keys.help', 'wrong-type'), entry('keys.help', 'invalid-binding')],
				omittedCount: 0,
			}).success,
		).toBe(false);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry('keys.help'), entry('keys.help')],
				omittedCount: 0,
			}).success,
		).toBe(false);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry(`${'界'.repeat(85)}a`)],
				omittedCount: 0,
			}).success,
		).toBe(true);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: [entry('界'.repeat(86))],
				omittedCount: 0,
			}).success,
		).toBe(false);
		expect(
			ConfigurationDiagnosticsSchema.safeParse({
				items: Array.from({ length: 129 }, (_, index) =>
					entry(`keys.${index.toString().padStart(3, '0')}`),
				),
				omittedCount: 0,
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
