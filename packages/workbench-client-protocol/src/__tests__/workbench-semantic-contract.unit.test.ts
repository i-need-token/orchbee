import { readFile } from 'node:fs/promises';

import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	SiteIdSchema,
	WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES,
	WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES,
	WORKBENCH_MAXIMUM_DIRECTORIES_PER_PROJECT,
	WorkbenchDeltaSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchNavigationSchema,
	WorkbenchPanelNodeSchema,
	WorkbenchPresentationSchema,
	WorkbenchProtocolDocumentSchema,
	WorkbenchRevisionSchema,
	WorkbenchSnapshotReadIntentSchema,
	WorkbenchSnapshotSchema,
	WorkbenchWorkspaceSchema,
	applyWorkbenchDelta,
	parseWorkbenchDelta,
	parseWorkbenchSnapshot,
} from '../workbench-semantic-contract';

const schemaUrl = new URL(
	'../../../../schemas/workbench/v1/workbench.schema.json',
	import.meta.url,
);
const corpusUrl = new URL(
	'../../../../schemas/workbench/v1/workbench-conformance-v1.json',
	import.meta.url,
);

const jsonObjectSchema = z.record(z.string(), z.unknown());
const transitionOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('applied'), snapshot: z.string().min(1) }),
	z.strictObject({
		kind: z.literal('rejected'),
		reason: z.enum([
			'base-revision-mismatch',
			'duplicate-change-target',
			'invalid-projection',
			'non-increasing-revision',
			'site-mismatch',
		]),
	}),
]);
const corpusSchema = z.strictObject({
	contractVersion: z.literal(1),
	documents: z.record(z.string().min(1), z.unknown()),
	invalidDocuments: z.array(
		z.strictObject({
			boundary: z.enum(['schema', 'semantic']),
			name: z.string().min(1),
			value: z.unknown(),
		}),
	),
	transitions: z.array(
		z.strictObject({
			delta: z.string().min(1),
			initial: z.string().min(1),
			name: z.string().min(1),
			outcome: transitionOutcomeSchema,
		}),
	),
	validDocuments: z.array(z.string().min(1)),
});

async function readContractArtifacts() {
	const [schemaText, corpusText] = await Promise.all([
		readFile(schemaUrl, 'utf8'),
		readFile(corpusUrl, 'utf8'),
	]);
	return {
		corpus: corpusSchema.parse(JSON.parse(corpusText)),
		schema: jsonObjectSchema.parse(JSON.parse(schemaText)),
	};
}

function resolveDocument(documents: Readonly<Record<string, unknown>>, name: string): unknown {
	const value = documents[name];
	if (value === undefined) throw new Error(`Missing Workbench conformance document: ${name}`);
	return value;
}

function isCanonicalUint64(value: string): boolean {
	if (!/^(?:0|[1-9][0-9]{0,19})$/u.test(value)) return false;
	try {
		return BigInt(value) <= 18_446_744_073_709_551_615n;
	} catch {
		return false;
	}
}

function opaqueIdentity(value: number): string {
	return value.toString(16).padStart(32, '0');
}

function panel(depth: number, identity: { next: number }): unknown {
	if (depth === 1) {
		const panelId = opaqueIdentity(identity.next);
		identity.next += 1;
		const presentationId = opaqueIdentity(identity.next);
		identity.next += 1;
		return {
			childPresentations: [],
			kind: 'panel',
			panelId,
			primaryPresentation: { binding: null, label: 'Terminal', presentationId },
		};
	}
	const splitId = opaqueIdentity(identity.next);
	identity.next += 1;
	const firstSlotId = opaqueIdentity(identity.next);
	identity.next += 1;
	const secondSlotId = opaqueIdentity(identity.next);
	identity.next += 1;
	return {
		children: [
			{ node: panel(depth - 1, identity), slotId: firstSlotId },
			{ node: panel(1, identity), slotId: secondSlotId },
		],
		kind: 'split',
		orientation: 'horizontal',
		splitId,
	};
}

function snapshotWithPanelDepth(depth: number): unknown {
	return {
		kind: 'workbench.snapshot',
		navigation: {
			projects: [],
			standaloneTerminalContextId: '33333333333333333333333333333333',
		},
		protocolVersion: 1,
		revision: '1',
		siteId: '11111111111111111111111111111111',
		terminalContexts: [
			{
				groups: [],
				tabs: [
					{
						groupId: null,
						label: 'Terminal',
						pinned: true,
						root: panel(depth, { next: 100 }),
						tabId: '88888888888888888888888888888888',
					},
				],
				terminalContextId: '33333333333333333333333333333333',
			},
		],
	};
}

describe('Workbench cross-language semantic contract', () => {
	it('keeps the JSON Schema oracle and TypeScript binding aligned on the canonical corpus', async () => {
		const { corpus, schema } = await readContractArtifacts();
		const ajv = new Ajv2020({ allErrors: true, strict: true });
		ajv.addFormat('orchbee-uint64', { type: 'string', validate: isCanonicalUint64 });
		const validateSchema = ajv.compile(schema);

		for (const name of corpus.validDocuments) {
			const value = resolveDocument(corpus.documents, name);
			const schemaAccepted = validateSchema(value);
			expect({ name, schemaAccepted }).toEqual({ name, schemaAccepted: true });
			expect({
				bindingAccepted: WorkbenchProtocolDocumentSchema.safeParse(value).success,
				name,
			}).toEqual({ bindingAccepted: true, name });
		}

		for (const fixture of corpus.invalidDocuments.filter(({ boundary }) => boundary === 'schema')) {
			const schemaAccepted = validateSchema(fixture.value);
			expect({ name: fixture.name, schemaAccepted }).toEqual({
				name: fixture.name,
				schemaAccepted: false,
			});
		}
		for (const fixture of corpus.invalidDocuments) {
			expect({
				bindingAccepted: WorkbenchProtocolDocumentSchema.safeParse(fixture.value).success,
				name: fixture.name,
			}).toEqual({ bindingAccepted: false, name: fixture.name });
		}
	});

	it('applies every canonical delta transition atomically', async () => {
		const { corpus } = await readContractArtifacts();
		for (const fixture of corpus.transitions) {
			const initial = parseWorkbenchSnapshot(resolveDocument(corpus.documents, fixture.initial));
			const delta = parseWorkbenchDelta(resolveDocument(corpus.documents, fixture.delta));
			const initialBefore = JSON.stringify(initial);
			const result = applyWorkbenchDelta(initial, delta);

			const expected =
				fixture.outcome.kind === 'applied'
					? {
							kind: 'applied' as const,
							snapshot: parseWorkbenchSnapshot(
								resolveDocument(corpus.documents, fixture.outcome.snapshot),
							),
						}
					: { kind: 'rejected' as const, reason: fixture.outcome.reason };
			expect({ name: fixture.name, result }).toEqual({ name: fixture.name, result: expected });
			expect({
				initialAfter: JSON.stringify(initial),
				initialBefore,
				name: fixture.name,
			}).toEqual({ initialAfter: initialBefore, initialBefore, name: fixture.name });
		}
	});

	it('uses exact identity, revision, strictness, and immutable-value boundaries', async () => {
		const { corpus } = await readContractArtifacts();
		const snapshot = parseWorkbenchSnapshot(resolveDocument(corpus.documents, 'snapshot-full-r2'));

		expect(SiteIdSchema.safeParse('11111111111111111111111111111111').success).toBe(true);
		expect(SiteIdSchema.safeParse('00000000000000000000000000000000').success).toBe(false);
		expect(WorkbenchRevisionSchema.safeParse('18446744073709551615').success).toBe(true);
		expect(WorkbenchRevisionSchema.safeParse('18446744073709551616').success).toBe(false);
		expect(Object.isFrozen(snapshot)).toBe(true);
		expect(Object.isFrozen(snapshot.navigation)).toBe(true);
		expect(Object.isFrozen(snapshot.terminalContexts)).toBe(true);
		expect(Object.isFrozen(snapshot.terminalContexts[0]?.tabs)).toBe(true);
		expect(
			WorkbenchSnapshotReadIntentSchema.safeParse({
				kind: 'workbench.snapshot.read',
				protocolVersion: 1,
				requestId: '22222222222222222222222222222222',
				transport: 'ipc',
			}).success,
		).toBe(false);
	});

	it('publishes byte caps independently from decoded value validation', () => {
		expect(WORKBENCH_MAXIMUM_ENCODED_INTENT_BYTES).toBe(16 * 1024);
		expect(WORKBENCH_MAXIMUM_ENCODED_PROJECTION_BYTES).toBe(1024 * 1024);
		expect(WorkbenchSnapshotSchema.safeParse(null).success).toBe(false);
		expect(WorkbenchDeltaSchema.safeParse(null).success).toBe(false);
	});

	it('enforces Unicode labels and collection and tree boundaries', () => {
		const presentation = {
			binding: null,
			label: '😀'.repeat(512),
			presentationId: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		};
		expect(WorkbenchPresentationSchema.safeParse(presentation).success).toBe(true);
		expect(
			WorkbenchPresentationSchema.safeParse({ ...presentation, label: '😀'.repeat(513) }).success,
		).toBe(false);

		const projects = Array.from({ length: 256 }, (_value, index) => ({
			directories: [],
			label: `Project ${index + 1}`,
			projectId: opaqueIdentity(index + 1),
			workspaces: [],
		}));
		expect(
			WorkbenchNavigationSchema.safeParse({
				projects,
				standaloneTerminalContextId: 'ffffffffffffffffffffffffffffffff',
			}).success,
		).toBe(true);
		expect(
			WorkbenchNavigationSchema.safeParse({
				projects: [
					...projects,
					{ directories: [], label: 'Overflow', projectId: opaqueIdentity(257), workspaces: [] },
				],
				standaloneTerminalContextId: 'ffffffffffffffffffffffffffffffff',
			}).success,
		).toBe(false);

		const change = {
			kind: 'terminal-context-removed',
			terminalContextId: '33333333333333333333333333333333',
		};
		const delta = {
			baseRevision: '1',
			changes: Array.from({ length: 1024 }, () => change),
			kind: 'workbench.delta',
			protocolVersion: 1,
			revision: '2',
			siteId: '11111111111111111111111111111111',
		};
		expect(WorkbenchDeltaSchema.safeParse(delta).success).toBe(true);
		expect(
			WorkbenchDeltaSchema.safeParse({ ...delta, changes: [...delta.changes, change] }).success,
		).toBe(false);
		expect(WorkbenchPanelNodeSchema.safeParse(panel(16, { next: 100 })).success).toBe(true);
		expect(WorkbenchSnapshotSchema.safeParse(snapshotWithPanelDepth(16)).success).toBe(true);
		expect(WorkbenchSnapshotSchema.safeParse(snapshotWithPanelDepth(17)).success).toBe(false);
	});

	it('enforces Directory path, collection, identity order, and same-Project reference invariants', () => {
		const directory = (directoryId: string, path = '/work/orchbee') => ({
			directoryId,
			path,
			workerId: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
		});
		const snapshot = (projects: readonly unknown[]) => ({
			kind: 'workbench.snapshot',
			navigation: {
				projects,
				standaloneTerminalContextId: '33333333333333333333333333333333',
			},
			protocolVersion: 1,
			revision: '2',
			siteId: '11111111111111111111111111111111',
			terminalContexts: [
				{ groups: [], tabs: [], terminalContextId: '33333333333333333333333333333333' },
				{ groups: [], tabs: [], terminalContextId: '66666666666666666666666666666666' },
			],
		});
		const workspace = {
			label: 'Main',
			primaryDirectoryId: '22222222222222222222222222222222',
			terminalContextId: '66666666666666666666666666666666',
			workspaceId: '55555555555555555555555555555555',
		};
		const project = {
			directories: [directory('22222222222222222222222222222222')],
			label: 'Orchbee',
			projectId: '44444444444444444444444444444444',
			workspaces: [workspace],
		};

		expect(WorkbenchSnapshotSchema.safeParse(snapshot([project])).success).toBe(true);
		expect(
			WorkbenchSnapshotSchema.safeParse(
				snapshot([
					{
						...project,
						directories: [
							directory('22222222222222222222222222222222'),
							directory('11111111111111111111111111111112'),
						],
					},
				]),
			).success,
		).toBe(false);
		expect(
			WorkbenchSnapshotSchema.safeParse(
				snapshot([
					{ ...project, workspaces: [{ ...workspace, primaryDirectoryId: null }] },
					{
						directories: [directory('22222222222222222222222222222222')],
						label: 'Duplicate owner',
						projectId: '77777777777777777777777777777777',
						workspaces: [],
					},
				]),
			).success,
		).toBe(false);
		expect(
			WorkbenchSnapshotSchema.safeParse(
				snapshot([
					{ ...project, directories: [] },
					{
						directories: [directory('22222222222222222222222222222222')],
						label: 'Other owner',
						projectId: '77777777777777777777777777777777',
						workspaces: [],
					},
				]),
			).success,
		).toBe(false);
		expect(
			WorkbenchWorkspaceSchema.safeParse({
				label: 'Missing required nullable field',
				terminalContextId: '66666666666666666666666666666666',
				workspaceId: '55555555555555555555555555555555',
			}).success,
		).toBe(false);

		const directories = Array.from(
			{ length: WORKBENCH_MAXIMUM_DIRECTORIES_PER_PROJECT },
			(_value, index) => directory(opaqueIdentity(index + 1), '/'),
		);
		expect(
			WorkbenchNavigationSchema.safeParse({
				projects: [{ ...project, directories, workspaces: [] }],
				standaloneTerminalContextId: 'ffffffffffffffffffffffffffffffff',
			}).success,
		).toBe(true);
		expect(
			WorkbenchNavigationSchema.safeParse({
				projects: [
					{ ...project, directories: [...directories, directory(opaqueIdentity(257), '/')] },
				],
				standaloneTerminalContextId: 'ffffffffffffffffffffffffffffffff',
			}).success,
		).toBe(false);

		expect(WorkbenchDirectoryPathSchema.safeParse('/' + '🐝'.repeat(1023)).success).toBe(true);
		expect(WorkbenchDirectoryPathSchema.safeParse('/' + '🐝'.repeat(1024)).success).toBe(false);
		expect(WorkbenchDirectoryPathSchema.safeParse('relative/path').success).toBe(false);
		expect(WorkbenchDirectoryPathSchema.safeParse('/nul\0path').success).toBe(false);
	});
});
