import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import type {
	WorkbenchCommand,
	WorkbenchCommandResult,
	WorkbenchTabMoveDirection,
} from '../workbench-command-contract';
import {
	SiteIdSchema,
	WorkbenchDirectoryIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchProjectIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkerIdSchema,
} from '../workbench-semantic-contract';
import {
	WorkbenchRequestSchema,
	WorkbenchResponseSchema,
	WorkbenchTerminalGeometrySchema,
	parseWorkbenchRequest,
	parseWorkbenchResponse,
} from '../workbench-command-contract';

const caseSchema = z.object({
	name: z.string(),
	direction: z.enum(['request', 'response']),
	document: z.unknown(),
	boundary: z.enum(['schema', 'semantic']).optional(),
});
const corpusSchema = z.object({
	contractVersion: z.literal(1),
	valid: z.array(caseSchema),
	invalid: z.array(caseSchema),
	validJson: z.array(
		z.object({
			name: z.string(),
			direction: z.enum(['request', 'response']),
			json: z.string(),
			canonicalJson: z.string(),
			canonical: z.unknown(),
		}),
	),
	invalidJson: z.array(
		z.object({
			name: z.string(),
			direction: z.enum(['request', 'response']),
			json: z.string(),
			boundary: z.enum(['schema', 'wire']).default('wire'),
		}),
	),
});
const objectSchema = z.record(z.string(), z.unknown());

describe('Workbench command contract', () => {
	it('preserves an independently constructed Workspace primary Directory acceptance', () => {
		const request: WorkbenchCommand = {
			kind: 'workbench.command',
			protocolVersion: 1,
			requestId: WorkbenchRequestIdSchema.parse('11111111111111111111111111111111'),
			siteId: SiteIdSchema.parse('22222222222222222222222222222222'),
			expectedRevision: WorkbenchRevisionSchema.parse('7'),
			action: {
				kind: 'workspace.primary-directory.accept',
				projectId: WorkbenchProjectIdSchema.parse('33333333333333333333333333333333'),
				workspaceId: WorkbenchWorkspaceIdSchema.parse('44444444444444444444444444444444'),
				path: WorkbenchDirectoryPathSchema.parse('/work/../work/orchbee'),
			},
		};
		const result: WorkbenchCommandResult = {
			kind: 'workbench.command.result',
			protocolVersion: 1,
			requestId: request.requestId,
			outcome: {
				kind: 'workspace-primary-directory-accepted',
				revision: WorkbenchRevisionSchema.parse('8'),
				directoryId: WorkbenchDirectoryIdSchema.parse('55555555555555555555555555555555'),
				workerId: WorkerIdSchema.parse('66666666666666666666666666666666'),
			},
		};

		expect(parseWorkbenchRequest(JSON.parse(JSON.stringify(request)))).toEqual(request);
		expect(parseWorkbenchResponse(JSON.parse(JSON.stringify(result)))).toEqual(result);
	});

	it.each<WorkbenchTabMoveDirection>(['previous', 'next', 'first', 'last'])(
		'preserves an independently constructed typed %s Move and its exact result',
		(direction) => {
			const request: WorkbenchCommand = {
				kind: 'workbench.command',
				protocolVersion: 1,
				requestId: WorkbenchRequestIdSchema.parse('11111111111111111111111111111111'),
				siteId: SiteIdSchema.parse('22222222222222222222222222222222'),
				expectedRevision: WorkbenchRevisionSchema.parse('7'),
				action: {
					kind: 'tab.move',
					terminalContextId: WorkbenchTerminalContextIdSchema.parse(
						'33333333333333333333333333333333',
					),
					tabId: WorkbenchTabIdSchema.parse('44444444444444444444444444444444'),
					direction,
				},
			};
			const result: WorkbenchCommandResult = {
				kind: 'workbench.command.result',
				protocolVersion: 1,
				requestId: request.requestId,
				outcome: {
					kind: 'tab-moved',
					revision: WorkbenchRevisionSchema.parse('8'),
					terminalContextId: WorkbenchTerminalContextIdSchema.parse(
						'33333333333333333333333333333333',
					),
					tabId: WorkbenchTabIdSchema.parse('44444444444444444444444444444444'),
				},
			};
			expect(parseWorkbenchRequest(JSON.parse(JSON.stringify(request)))).toEqual(request);
			expect(parseWorkbenchResponse(JSON.parse(JSON.stringify(result)))).toEqual(result);
		},
	);

	it('agrees with the schema-authored Rust/TypeScript conformance corpus', async () => {
		const [projectionText, commandText, corpusText] = await Promise.all([
			readFile(
				new URL('../../../../schemas/workbench/v1/workbench.schema.json', import.meta.url),
				'utf8',
			),
			readFile(
				new URL('../../../../schemas/workbench/v1/workbench-command.schema.json', import.meta.url),
				'utf8',
			),
			readFile(
				new URL(
					'../../../../schemas/workbench/v1/workbench-command-conformance-v1.json',
					import.meta.url,
				),
				'utf8',
			),
		]);
		const corpus = corpusSchema.parse(JSON.parse(corpusText));
		const ajv = new Ajv2020({ strict: true });
		ajv.addFormat(
			'orchbee-uint64',
			(value: string) =>
				/^(?:0|[1-9][0-9]{0,19})$/u.test(value) && BigInt(value) <= 18_446_744_073_709_551_615n,
		);
		ajv.addSchema(objectSchema.parse(JSON.parse(projectionText)));
		const validate = ajv.compile(objectSchema.parse(JSON.parse(commandText)));
		for (const testCase of corpus.valid) {
			expect({ name: testCase.name, valid: validate(testCase.document) }).toEqual({
				name: testCase.name,
				valid: true,
			});
			const parsed =
				testCase.direction === 'request'
					? parseWorkbenchRequest(testCase.document)
					: parseWorkbenchResponse(testCase.document);
			expect({ name: testCase.name, parsed }).toEqual({
				name: testCase.name,
				parsed: testCase.document,
			});
		}
		for (const testCase of corpus.invalid) {
			const schema =
				testCase.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect({ name: testCase.name, valid: schema.safeParse(testCase.document).success }).toEqual({
				name: testCase.name,
				valid: false,
			});
			expect({ name: testCase.name, valid: validate(testCase.document) }).toEqual({
				name: testCase.name,
				valid: testCase.boundary === 'semantic',
			});
		}
		for (const testCase of corpus.validJson) {
			const value: unknown = JSON.parse(testCase.json);
			expect({ name: testCase.name, valid: validate(value) }).toEqual({
				name: testCase.name,
				valid: true,
			});
			const parsed =
				testCase.direction === 'request'
					? parseWorkbenchRequest(value)
					: parseWorkbenchResponse(value);
			expect({ name: testCase.name, parsed }).toEqual({
				name: testCase.name,
				parsed: testCase.canonical,
			});
			expect({ name: testCase.name, canonicalJson: JSON.stringify(parsed) }).toEqual({
				name: testCase.name,
				canonicalJson: testCase.canonicalJson,
			});
		}
		for (const testCase of corpus.invalidJson.filter((value) => value.boundary === 'schema')) {
			const value: unknown = JSON.parse(testCase.json);
			const schema =
				testCase.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema;
			expect({ name: testCase.name, valid: schema.safeParse(value).success }).toEqual({
				name: testCase.name,
				valid: false,
			});
			expect({ name: testCase.name, valid: validate(value) }).toEqual({
				name: testCase.name,
				valid: false,
			});
		}
	});

	it('enforces every Workbench terminal geometry boundary and semantic product', () => {
		for (const geometry of [
			{ columns: 1, rows: 1, cellPixelWidth: 1, cellPixelHeight: 1 },
			{ columns: 1, rows: 1, cellPixelWidth: 4096, cellPixelHeight: 4096 },
			{ columns: 4096, rows: 64, cellPixelWidth: 15, cellPixelHeight: 1023 },
			{ columns: 255, rows: 255, cellPixelWidth: 257, cellPixelHeight: 257 },
			{ columns: 80, rows: 24, cellPixelWidth: 0, cellPixelHeight: 0 },
			{ columns: 4096, rows: 64, cellPixelWidth: 0, cellPixelHeight: 0 },
		]) {
			expect({
				geometry,
				valid: WorkbenchTerminalGeometrySchema.safeParse(geometry).success,
			}).toEqual({ geometry, valid: true });
		}

		for (const geometry of [
			{ columns: 0, rows: 1, cellPixelWidth: 1, cellPixelHeight: 1 },
			{ columns: 1, rows: 0, cellPixelWidth: 1, cellPixelHeight: 1 },
			{ columns: 1, rows: 1, cellPixelWidth: 0, cellPixelHeight: 1 },
			{ columns: 1, rows: 1, cellPixelWidth: 1, cellPixelHeight: 0 },
			{ columns: 4097, rows: 1, cellPixelWidth: 1, cellPixelHeight: 1 },
			{ columns: 1, rows: 4097, cellPixelWidth: 1, cellPixelHeight: 1 },
			{ columns: 1, rows: 1, cellPixelWidth: 4097, cellPixelHeight: 1 },
			{ columns: 1, rows: 1, cellPixelWidth: 1, cellPixelHeight: 4097 },
			{ columns: 4096, rows: 65, cellPixelWidth: 1, cellPixelHeight: 1 },
			{ columns: 4096, rows: 1, cellPixelWidth: 16, cellPixelHeight: 1 },
			{ columns: 1, rows: 4096, cellPixelWidth: 1, cellPixelHeight: 16 },
		]) {
			expect({
				geometry,
				valid: WorkbenchTerminalGeometrySchema.safeParse(geometry).success,
			}).toEqual({ geometry, valid: false });
		}
	});

	it('preserves exact Unicode labels and rejects the next code point', () => {
		const request = {
			kind: 'workbench.command',
			protocolVersion: 1,
			requestId: '11111111111111111111111111111111',
			siteId: '22222222222222222222222222222222',
			expectedRevision: '0',
			action: { kind: 'project.create', label: '  exact  ' },
		};
		expect(parseWorkbenchRequest(request)).toEqual(request);
		request.action.label = '\u{1F41D}'.repeat(512);
		expect(WorkbenchRequestSchema.safeParse(request).success).toBe(true);
		request.action.label += 'x';
		expect(WorkbenchRequestSchema.safeParse(request).success).toBe(false);
	});
});
