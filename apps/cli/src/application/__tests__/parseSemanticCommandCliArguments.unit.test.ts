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
				arguments: ['tab', 'list', '--standalone'],
				input: { limit: 100, location: { kind: 'standalone' } },
				kind: 'tab-list',
				operationId: 'workbench.tab.list',
			},
			{
				arguments: [
					'panel',
					'list',
					'--project',
					'project-1',
					'--workspace',
					'workspace-1',
					'--tab',
					'tab-1',
				],
				input: {
					limit: 100,
					location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
					tabId: 'tab-1',
				},
				kind: 'panel-list',
				operationId: 'workbench.panel.list',
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

	it('maps high-level Agent reads and waits to exact owner operations and inputs', () => {
		const cases = [
			{
				arguments: ['agent', 'list', '--all', '--limit', '25', '--cursor', 'next-agent'],
				input: { cursor: 'next-agent', includeEnded: true, limit: 25 },
				kind: 'agent-list',
				operationId: 'agent.session.list',
			},
			{
				arguments: ['agent', 'get', 'session-1'],
				input: { sessionId: 'session-1' },
				kind: 'agent-get',
				operationId: 'agent.session.get',
			},
			{
				arguments: ['agent', 'explain', 'session-1'],
				input: { sessionId: 'session-1' },
				kind: 'agent-explain',
				operationId: 'agent.session.explain',
			},
			{
				arguments: [
					'agent',
					'wait',
					'session-1',
					'--until',
					'needs-attention',
					'--until',
					'succeeded',
					'--after-revision',
					'42',
					'--timeout',
					'30000',
					'--detach',
					'--request-id',
					'cli:request:agent-wait',
				],
				input: {
					afterRevision: 42,
					sessionId: 'session-1',
					timeoutMilliseconds: 30_000,
					until: ['needs-attention', 'succeeded'],
				},
				kind: 'agent-wait',
				operationId: 'agent.session.wait',
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
			['tab', 'list'],
			['tab', 'list', '--standalone', '--tab', 'tab-1'],
			['panel', 'list', '--standalone'],
			['panel', 'list', '--standalone', '--tab', ''],
			['terminal', 'list'],
			['terminal', 'list', '--standalone', '--project', 'project-1'],
			['terminal', 'list', '--project', 'project-1'],
			['terminal', 'create', '--standalone', '--cursor', 'not-accepted'],
			['agent', 'list', '--limit', '101'],
			['agent', 'get'],
			['agent', 'get', 'bad identity/with spaces'],
			['agent', 'get', 'session-1', '--all'],
			['agent', 'wait', 'session-1', '--timeout', '0'],
			['agent', 'wait', 'session-1', '--timeout', '3600001'],
			['agent', 'wait', 'session-1', '--after-revision', '-1'],
			['agent', 'wait', 'session-1', '--until', 'done'],
			['agent', 'wait', 'session-1', '--until', 'idle', '--until', 'idle'],
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
