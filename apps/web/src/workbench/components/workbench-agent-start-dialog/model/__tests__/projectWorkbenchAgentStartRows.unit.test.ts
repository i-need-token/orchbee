import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import { projectWorkbenchAgentStartRows } from '../projectWorkbenchAgentStartRows';

const stateSchema = z.enum(['cant-check', 'checking', 'found', 'not-found', 'unknown']);
const groupSchema = z.enum(['installed', 'not-checked', 'not-found', 'recent']);

/** The shared Agent start rows corpus, which the TUI's picker satisfies too. */
const corpus = z
	.strictObject({
		cases: z.array(
			z.strictObject({
				catalog: z.object({
					agents: z.array(
						z.object({
							command: z.string(),
							productName: z.string(),
							providerId: z.string(),
							state: stateSchema,
						}),
					),
					lookup: z.object({
						checkedAt: z.number().nullable(),
						status: z.enum(['complete', 'failed', 'running', 'unsupported']),
					}),
					recent: z.array(z.string()),
				}),
				expanded: z.boolean(),
				expected: z.strictObject({
					rows: z.array(
						z.discriminatedUnion('kind', [
							z.strictObject({ group: groupSchema, kind: z.literal('heading') }),
							z.strictObject({
								detail: z.enum(['cant-check', 'command', 'not-found']),
								group: groupSchema,
								kind: z.literal('provider'),
								providerId: z.string(),
							}),
							z.strictObject({ count: z.number(), kind: z.literal('fold') }),
						]),
					),
					status: z.enum(['couldnt-check', 'looking']).nullable(),
				}),
				name: z.string(),
				query: z.string(),
			}),
		),
		contractVersion: z.literal(1),
	})
	.parse(
		JSON.parse(
			await readFile(
				new URL(
					'../../../../../../../../schemas/workbench/v1/agent-start-rows-conformance-v1.json',
					import.meta.url,
				),
				'utf8',
			),
		),
	);

describe('Start Agent rows', () => {
	it.each(corpus.cases.map((entry) => [entry.name, entry] as const))(
		'%s',
		(_name, { catalog, expanded, expected, query }) => {
			expect(projectWorkbenchAgentStartRows(catalog, query, expanded)).toStrictEqual(expected);
		},
	);
});
