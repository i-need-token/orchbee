import { describe, expect, it } from 'vitest';

import {
	DesktopRemoteTerminalCreationMainEventSchema,
	DesktopRemoteTerminalCreationRendererCommandSchema,
} from '../desktop-remote-terminal-creation-contract';

describe('Desktop remote-terminal creation contract', () => {
	it('carries only canonical Project and Workspace identity in a creation request', () => {
		const command = DesktopRemoteTerminalCreationRendererCommandSchema.parse({
			projectId: 'project-1',
			requestId: 'remote-terminal:1',
			type: 'desktop-remote-terminal-creation-requested',
			workspaceId: 'workspace-1',
		});

		expect(command).toEqual({
			projectId: 'project-1',
			requestId: 'remote-terminal:1',
			type: 'desktop-remote-terminal-creation-requested',
			workspaceId: 'workspace-1',
		});
		expect(command).not.toHaveProperty('environmentId');
		expect(command).not.toHaveProperty('path');
		expect(command).not.toHaveProperty('placement');
	});

	it('correlates cancellation and every bounded settlement class', () => {
		expect(
			DesktopRemoteTerminalCreationRendererCommandSchema.safeParse({
				requestId: 'remote-terminal:1',
				type: 'desktop-remote-terminal-creation-cancel-requested',
			}).success,
		).toBe(true);

		for (const result of [
			{
				kind: 'created',
				panelId: 'panel-1',
				presentationId: 'presentation-1',
				projectId: 'project-1',
				revision: '2',
				tabId: 'tab-1',
				terminalContextId: 'terminal-context-1',
				workspaceId: 'workspace-1',
			},
			{ kind: 'cancelled' },
			{ kind: 'failed', reason: 'environment-not-connected' },
			{ kind: 'indeterminate', reason: 'durability-unconfirmed' },
		]) {
			expect(
				DesktopRemoteTerminalCreationMainEventSchema.safeParse({
					requestId: 'remote-terminal:1',
					result,
					type: 'desktop-remote-terminal-creation-settled',
				}).success,
			).toBe(true);
		}
	});

	it('rejects renderer-supplied authority and raw failure details', () => {
		expect(
			DesktopRemoteTerminalCreationRendererCommandSchema.safeParse({
				environmentId: 'environment-1',
				projectId: 'project-1',
				requestId: 'remote-terminal:2',
				type: 'desktop-remote-terminal-creation-requested',
				workspaceId: 'workspace-1',
			}).success,
		).toBe(false);
		expect(
			DesktopRemoteTerminalCreationMainEventSchema.safeParse({
				requestId: 'remote-terminal:2',
				result: {
					detail: 'ssh password rejected by 10.0.0.1',
					kind: 'failed',
					reason: 'terminal-host-unavailable',
				},
				type: 'desktop-remote-terminal-creation-settled',
			}).success,
		).toBe(false);
	});
});
