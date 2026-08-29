import { describe, expect, it } from 'vitest';

import { parseSemanticCommandCliArguments } from '../parseSemanticCommandCliArguments';

const endpointId = 'ab'.repeat(16);

describe('parseSemanticCommandCliArguments', () => {
	it('parses every accepted command family with exact global selectors', () => {
		const cases = [
			{ arguments: ['status'], kind: 'status' },
			{ arguments: ['operation', 'list'], kind: 'operation-list' },
			{
				arguments: ['operation', 'describe', 'workbench.project.list'],
				kind: 'operation-describe',
			},
			{
				arguments: [
					'operation',
					'invoke',
					'workbench.terminal.create',
					'--input-file',
					'-',
					'--detach',
					'--request-id',
					'cli:request:1',
				],
				kind: 'operation-invoke',
			},
			{ arguments: ['execution', 'get', 'cli:request:1'], kind: 'execution-get' },
			{ arguments: ['execution', 'wait', 'execution:1'], kind: 'execution-wait' },
			{ arguments: ['execution', 'cancel', 'execution:1'], kind: 'execution-cancel' },
		] as const;

		for (const candidate of cases) {
			const parsed = parseSemanticCommandCliArguments([
				...candidate.arguments,
				'--target',
				endpointId,
				'--output',
				'text',
			]);
			expect(parsed).toMatchObject({
				command: { endpointId, kind: candidate.kind, output: 'text' },
				kind: 'parsed',
			});
		}
	});

	it('rejects unknown, duplicate, missing, and cross-command options', () => {
		for (const arguments_ of [
			[],
			['unknown'],
			['status', '--detach'],
			['status', '--target', endpointId, '--target', endpointId],
			['operation', 'invoke', 'workbench.terminal.create'],
			['execution', 'get', 'bad identity/with spaces'],
			['status', '--target', 'not-an-endpoint'],
		]) {
			expect(parseSemanticCommandCliArguments(arguments_)).toEqual({
				code: 'cli-usage-invalid',
				kind: 'error',
			});
		}
		expect(parseSemanticCommandCliArguments(['status', '--output', 'yaml'])).toEqual({
			code: 'output-selection-invalid',
			kind: 'error',
		});
	});
});
