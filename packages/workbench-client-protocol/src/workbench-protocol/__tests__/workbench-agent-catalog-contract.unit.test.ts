import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';
import { WorkbenchAgentStartProviderSchema } from '../workbench-semantic-contract';

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
const groupSchema = z.enum(['recent', 'installed', 'not-checked', 'not-found']);
const rowsCorpusSchema = z.strictObject({
	contractVersion: z.literal(1),
	cases: z
		.array(
			z.strictObject({
				name: z.string().min(1),
				catalog: z.unknown(),
				query: z.string(),
				expanded: z.boolean(),
				expected: z.strictObject({
					status: z.enum(['looking', 'couldnt-check']).nullable(),
					rows: z.array(
						z.discriminatedUnion('kind', [
							z.strictObject({ kind: z.literal('heading'), group: groupSchema }),
							z.strictObject({
								kind: z.literal('provider'),
								providerId: WorkbenchAgentStartProviderSchema,
								group: groupSchema,
								detail: z.enum(['command', 'not-found', 'cant-check']),
							}),
							z.strictObject({
								kind: z.literal('fold'),
								count: z.number().int().min(1),
								holds: z.enum(['not-found', 'cant-check', 'both']),
							}),
						]),
					),
				}),
			}),
		)
		.min(1),
});
/** The members the parity checks read, once the catalog schema has admitted the whole catalog. */
const catalogMembersSchema = z.object({
	recent: z.array(WorkbenchAgentStartProviderSchema),
	agents: z.array(z.object({ providerId: WorkbenchAgentStartProviderSchema })),
});
const CATALOG_SCHEMA_ID =
	'https://orchbee.dev/schemas/workbench/v1/workbench-agent-catalog.schema.json';
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
const catalogSchemas = async () => {
	const ajv = new Ajv2020({ allErrors: true, strict: false });
	ajv.addFormat(
		'orchbee-uint64',
		(value: string) =>
			/^(?:0|[1-9][0-9]{0,19})$/u.test(value) && BigInt(value) <= 18_446_744_073_709_551_615n,
	);
	const schemas = await Promise.all(
		[
			'workbench.schema.json',
			'workbench-client-view.schema.json',
			'workbench-pane-graphics.schema.json',
			'workbench-command.schema.json',
			'workbench-agent-catalog.schema.json',
		].map(json),
	);
	ajv.addSchema(schemas);
	return ajv;
};

const binds = (test: z.infer<typeof caseSchema>) =>
	(test.direction === 'request' ? WorkbenchRequestSchema : WorkbenchResponseSchema).safeParse(
		test.document,
	).success;

describe('Agent catalog', () => {
	it('shares strict documents and semantic failures with the independent Rust binding', async () => {
		const corpus = corpusSchema.parse(await json('workbench-agent-catalog-conformance-v1.json'));
		const validate = (await catalogSchemas()).getSchema(CATALOG_SCHEMA_ID);
		if (validate === undefined) throw new Error('Agent catalog schema missing');
		const mismatches: string[] = [];
		for (const test of corpus.valid) {
			if (!binds(test)) mismatches.push(`${test.name}: binding refuses`);
			if (!validate(test.document)) mismatches.push(`${test.name}: schema refuses`);
		}
		for (const test of corpus.invalid) {
			if (binds(test)) mismatches.push(`${test.name}: binding admits`);
			const schemaAdmits = validate(test.document);
			if (test.boundary === 'schema' && schemaAdmits) {
				mismatches.push(`${test.name}: schema admits`);
			}
			if (test.boundary === 'semantic' && !schemaAdmits) {
				mismatches.push(`${test.name}: schema refuses`);
			}
		}
		expect(mismatches).toEqual([]);
	});

	it('keeps the Agent start rows parity corpus well formed', async () => {
		const corpus = rowsCorpusSchema.parse(await json('agent-start-rows-conformance-v1.json'));
		const validate = (await catalogSchemas()).getSchema(`${CATALOG_SCHEMA_ID}#/$defs/catalog`);
		if (validate === undefined) throw new Error('Agent catalog definition missing');
		const order: readonly string[] = WorkbenchAgentStartProviderSchema.options;
		const problems = corpus.cases.flatMap((test, index) => {
			const found: string[] = [];
			if (corpus.cases.findIndex((other) => other.name === test.name) !== index) {
				found.push('repeated name');
			}
			if (!validate(test.catalog)) found.push('catalog refused by the schema');
			const catalog = catalogMembersSchema.parse(test.catalog);
			const providers = catalog.agents.map((row) => row.providerId);
			const positions = providers.map((provider) => order.indexOf(provider));
			if (positions.some((position, at) => at > 0 && position <= (positions[at - 1] ?? -1))) {
				found.push('rows out of catalog order');
			}
			if (!catalog.recent.every((provider) => providers.includes(provider))) {
				found.push('recent names a provider without a row');
			}
			const headings: string[] = [];
			test.expected.rows.forEach((row, at) => {
				if (row.kind === 'heading') {
					if (headings.includes(row.group)) found.push(`heading ${row.group} repeated`);
					headings.push(row.group);
				} else if (row.kind === 'provider') {
					const detailFits = (row.detail === 'command') === (row.group !== 'not-found');
					if (headings.at(-1) !== row.group || !providers.includes(row.providerId) || !detailFits) {
						found.push(`row ${at}`);
					}
				} else if (
					at !== test.expected.rows.length - 1 ||
					row.count > providers.length ||
					(row.holds === 'both' && row.count < 2)
				) {
					found.push('fold');
				}
			});
			return found.map((problem) => `${test.name}: ${problem}`);
		});
		expect(problems).toEqual([]);
	});
});
