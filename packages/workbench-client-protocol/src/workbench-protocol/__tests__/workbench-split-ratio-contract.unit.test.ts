import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { WorkbenchRequestSchema, WorkbenchResponseSchema } from '../workbench-command-contract';
import { WorkbenchPanelNodeSchema } from '../workbench-semantic-contract';

const id = (value: number) => value.toString(16).padStart(32, '0');
const leaf = (value: number) => ({
	kind: 'panel',
	panelId: id(value),
	primaryPresentation: {
		customLabel: null,
		presentationId: id(value + 1),
		label: 'shell',
		binding: null,
	},
	childPresentations: [],
});
const split = {
	kind: 'split',
	splitId: id(1),
	orientation: 'horizontal',
	children: [
		{ slotId: id(2), node: leaf(3) },
		{ slotId: id(5), node: leaf(6) },
	],
};
const request = {
	kind: 'workbench.command',
	protocolVersion: 1,
	requestId: id(10),
	sessionId: id(11),
	expectedRevision: '12',
	action: {
		kind: 'layout.set-split-ratio',
		terminalContextId: id(13),
		tabId: id(14),
		split: { splitId: id(1), firstSlotId: id(2), secondSlotId: id(5) },
		previousRatio: 0.5,
		ratio: 0.7,
	},
};

async function validators() {
	const files = [
		'workbench.schema.json',
		'workbench-client-view.schema.json',
		'workbench-panel-graphics.schema.json',
		'workbench-command.schema.json',
	];
	const texts = await Promise.all(
		files.map(async (file) =>
			readFile(new URL(`../../../../../schemas/workbench/v1/${file}`, import.meta.url), 'utf8'),
		),
	);
	const ajv = new Ajv2020({ strict: true });
	ajv.addFormat('orchbee-uint64', {
		type: 'string',
		validate: (value) => /^(0|[1-9][0-9]*)$/u.test(value) && BigInt(value) <= 18446744073709551615n,
	});
	for (const text of texts)
		ajv.addSchema(z.record(z.string(), z.unknown()).parse(JSON.parse(text)));
	return {
		node: ajv.compile({ $ref: 'urn:orchbee:schema:workbench:v1#/$defs/panelNode' }),
		command: ajv.compile({ $ref: 'urn:orchbee:schema:workbench-command:v1' }),
	};
}
describe('canonical shared split ratio', () => {
	it('admits legacy absence and finite bounded ratios independently in Zod and JSON Schema', async () => {
		const { node, command } = await validators();
		expect(WorkbenchPanelNodeSchema.parse(split)).toEqual(split);
		expect(node(split)).toBe(true);
		for (const ratio of [0.1, 0.1001, 0.5, 0.8999, 0.9]) {
			const graph = { ...split, ratio };
			expect(WorkbenchPanelNodeSchema.parse(graph)).toEqual(graph);
			expect(node(graph)).toBe(true);
			const mutation = { ...request, action: { ...request.action, ratio } };
			expect(WorkbenchRequestSchema.parse(mutation)).toEqual(mutation);
			expect(command(mutation)).toBe(true);
		}
		for (const ratio of [null, '0.7', 0.0999, 0.9001, -1, 1]) {
			expect(WorkbenchPanelNodeSchema.safeParse({ ...split, ratio }).success).toBe(false);
			expect(node({ ...split, ratio })).toBe(false);
			for (const field of ['ratio', 'previousRatio']) {
				const mutation = { ...request, action: { ...request.action, [field]: ratio } };
				expect(WorkbenchRequestSchema.safeParse(mutation).success).toBe(false);
				expect(command(mutation)).toBe(false);
			}
		}
		for (const ratio of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
			expect(WorkbenchPanelNodeSchema.safeParse({ ...split, ratio }).success).toBe(false);
			expect(
				WorkbenchRequestSchema.safeParse({ ...request, action: { ...request.action, ratio } })
					.success,
			).toBe(false);
		}
	});
	it('requires the complete ordered identity and carries the canonical result', async () => {
		const { command } = await validators();
		const result = {
			kind: 'workbench.command.result',
			protocolVersion: 1,
			requestId: id(10),
			outcome: { kind: 'split-ratio-configured', revision: '13' },
		};
		expect(WorkbenchResponseSchema.parse(result)).toEqual(result);
		expect(command(result)).toBe(true);
		for (const invalid of [
			{ ...request, action: { ...request.action, unexpected: true } },
			{ ...request, action: { ...request.action, split: { splitId: id(1), firstSlotId: id(2) } } },
		]) {
			expect(WorkbenchRequestSchema.safeParse(invalid).success).toBe(false);
			expect(command(invalid)).toBe(false);
		}
		expect(
			WorkbenchRequestSchema.safeParse({
				...request,
				action: { ...request.action, split: { ...request.action.split, secondSlotId: id(2) } },
			}).success,
		).toBe(false);
	});
});
