import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_TERMINAL_CREATE_OPERATION,
	WorkbenchTerminalCreateInputSchema,
	WorkbenchTerminalCreateOutputSchema,
} from '../semantic-command-write-contracts';

describe('Workbench semantic command write contracts', () => {
	it('publishes one strict exact-target Terminal-create contract', () => {
		expect(WORKBENCH_TERMINAL_CREATE_OPERATION).toEqual({
			confirmation: 'noninteractive-allowed',
			id: 'workbench.terminal.create',
			input: { name: 'workbench.terminal-create-input', schemaVersion: 1 },
			kind: 'command',
			output: { name: 'workbench.terminal-create-output', schemaVersion: 1 },
			retry: 'same-request-while-retained',
			schemaVersion: 1,
		});
		expect(
			WorkbenchTerminalCreateInputSchema.parse({
				directoryId: 'directory-1',
				location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
			}),
		).toEqual({
			directoryId: 'directory-1',
			location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
		});
		expect(
			WorkbenchTerminalCreateInputSchema.safeParse({
				activeWorkspace: true,
				location: { kind: 'standalone' },
			}).success,
		).toBe(false);
	});

	it('requires the committed revision and managed intent identities in safe output', () => {
		const output = {
			intent: { kind: 'provision', operationId: 'operation-1' },
			panelId: 'panel-1',
			presentationId: 'presentation-1',
			revision: '8',
			tabId: 'tab-1',
			terminalContextId: 'context-1',
		};
		expect(WorkbenchTerminalCreateOutputSchema.parse(output)).toEqual(output);
		expect(
			WorkbenchTerminalCreateOutputSchema.safeParse({ ...output, revision: '07' }).success,
		).toBe(false);
		expect(
			WorkbenchTerminalCreateOutputSchema.safeParse({
				...output,
				intent: { kind: 'preview' },
			}).success,
		).toBe(false);
	});
});
