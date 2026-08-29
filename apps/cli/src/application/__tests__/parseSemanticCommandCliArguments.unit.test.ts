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

	it('parses the release-matched Skill without a coordinator selector', () => {
		expect(parseSemanticCommandCliArguments(['--skill'])).toEqual({
			command: { kind: 'skill' },
			kind: 'parsed',
		});
	});

	it('maps high-level Workbench commands to exact owner operations and inputs', () => {
		const cases = [
			{
				arguments: ['project', 'list', '--limit', '20', '--cursor', 'next-project'],
				input: { cursor: 'next-project', limit: 20 },
				kind: 'project-list',
				operationId: 'workbench.project.list',
			},
			{
				arguments: ['directory', 'list', '--project', 'project-1'],
				input: { limit: 100, projectId: 'project-1' },
				kind: 'directory-list',
				operationId: 'workbench.directory.list',
			},
			{
				arguments: ['workspace', 'list', '--project', 'project-1'],
				input: { limit: 100, projectId: 'project-1' },
				kind: 'workspace-list',
				operationId: 'workbench.workspace.list',
			},
			{
				arguments: ['terminal', 'list', '--standalone'],
				input: { limit: 100, location: { kind: 'standalone' } },
				kind: 'terminal-list',
				operationId: 'workbench.terminal.list',
			},
			{
				arguments: ['terminal', 'list', '--project', 'project-1', '--workspace', 'workspace-1'],
				input: {
					limit: 100,
					location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
				},
				kind: 'terminal-list',
				operationId: 'workbench.terminal.list',
			},
			{
				arguments: [
					'terminal',
					'create',
					'--project',
					'project-1',
					'--workspace',
					'workspace-1',
					'--directory',
					'directory-1',
					'--detach',
					'--request-id',
					'cli:request:terminal-create',
				],
				input: {
					directoryId: 'directory-1',
					location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
				},
				kind: 'terminal-create',
				operationId: 'workbench.terminal.create',
			},
		] as const;

		for (const candidate of cases) {
			expect(parseSemanticCommandCliArguments(candidate.arguments)).toMatchObject({
				command: {
					input: candidate.input,
					kind: candidate.kind,
					operation: { id: candidate.operationId },
				},
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
			['--skill', '--output', 'json'],
			['project', 'list', '--limit', '0'],
			['project', 'list', '--limit', '201'],
			['directory', 'list'],
			['workspace', 'list', '--workspace', 'workspace-1'],
			['terminal', 'list'],
			['terminal', 'list', '--standalone', '--project', 'project-1'],
			['terminal', 'list', '--project', 'project-1'],
			['terminal', 'create', '--standalone', '--cursor', 'not-accepted'],
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
