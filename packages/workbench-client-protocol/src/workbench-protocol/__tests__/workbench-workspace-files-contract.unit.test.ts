import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';
import {
	WORKSPACE_FILES_RESULT_BYTES,
	WORKSPACE_FILES_SEARCH_REQUEST_BYTES,
} from '../workbench-workspace-files-contract';
const caseSchema = z.strictObject({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
describe('Workspace files and Git change reads', () => {
	it('matches the shared Rust workspace files corpus and closed shapes', async () => {
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
							'../../../../../schemas/workbench/v1/workbench-workspace-files-conformance-v1.json',
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
							'../../../../../schemas/workbench/v1/workbench-workspace-files.schema.json',
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

	// The encoded limits are the Rust crate's own, so a whole chunk or a replacement naming many
	// matches is not refused by the window after the Server sent or accepted it.
	it('carries a chunk and a replacement as large as the Server allows', () => {
		const id = (value: number): string => value.toString(16).padStart(32, '0');
		const identity = { protocolVersion: 1, requestId: id(1), sessionId: id(2), workspaceId: id(3) };
		const result = (text: string): unknown => ({
			...identity,
			kind: 'workbench.workspace-files.result',
			outcome: {
				byteLength: text.length,
				content: { kind: 'text', nextOffset: null, offset: 0, text },
				kind: 'file',
				version: null,
			},
		});
		const chunk = result('x'.repeat(256 * 1024));
		expect(JSON.stringify(chunk).length).toBeLessThan(WORKSPACE_FILES_RESULT_BYTES);
		expect(WorkbenchResponseSchema.safeParse(chunk).success).toBe(true);
		const replace = (files: number): unknown => ({
			...identity,
			kind: 'workbench.workspace-files',
			operation: {
				files: Array.from({ length: files }, (_file, index) => ({
					matches: Array.from({ length: 50 }, (_match, line) => ({ column: 0, line: line + 1 })),
					path: `src/${String(index).padStart(400, '0')}.txt`,
				})),
				kind: 'replace-matches',
				query: {
					matchCase: false,
					pattern: 'needle',
					preserveCase: false,
					regex: false,
					replacement: 'pin',
					wholeWord: false,
				},
			},
		});
		const large = replace(256);
		expect(JSON.stringify(large).length).toBeLessThan(WORKSPACE_FILES_SEARCH_REQUEST_BYTES);
		expect(JSON.stringify(large).length).toBeGreaterThan(64 * 1024);
		expect(WorkbenchRequestSchema.safeParse(large).success).toBe(true);
	});
});
