import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import { DesktopProjectWorktreeClient } from '../DesktopProjectWorktreeClient';

describe('DesktopProjectWorktreeClient', () => {
	it('discovers, creates, and adopts through typed request identities', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopProjectWorktreeRendererCommandSchema.parse(event.data);
			if (command.type === 'desktop-project-worktree-discovery-requested') {
				channel.port2.postMessage(
					DesktopProjectWorktreeMainEventSchema.parse({
						discovery: { managedRoot: '/managed', repositories: [] },
						requestId: command.requestId,
						type: 'desktop-project-worktree-discovery-completed',
					}),
				);
				return;
			}
			channel.port2.postMessage(
				command.type === 'desktop-project-worktree-adoption-requested'
					? DesktopProjectWorktreeMainEventSchema.parse({
							requestId: command.requestId,
							result: { directoryId: 'directory-3', workspaceId: 'workspace-3' },
							type: 'desktop-project-worktree-adoption-completed',
						})
					: DesktopProjectWorktreeMainEventSchema.parse({
							requestId: command.requestId,
							result: { directoryId: 'directory-2', workspaceId: 'workspace-2' },
							type: 'desktop-project-worktree-creation-completed',
						}),
			);
		});
		channel.port2.start();
		let request = 0;
		const client = new DesktopProjectWorktreeClient(
			channel.port1,
			() => `request-${++request}` as never,
		);

		await expect(
			client.discover({ preferredDirectoryId: null, sourceWorkspaceId: 'workspace-1' }),
		).resolves.toEqual({
			discovery: { managedRoot: '/managed', repositories: [] },
			kind: 'discovered',
		});
		await expect(
			client.create({
				baseRef: 'main',
				branchName: 'feature',
				createWorkspace: true,
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).resolves.toEqual({
			directoryId: 'directory-2',
			kind: 'created',
			workspaceId: 'workspace-2',
		});
		await expect(
			client.adopt({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
				worktreeId: 'worktree-1',
			}),
		).resolves.toEqual({
			directoryId: 'directory-3',
			kind: 'adopted',
			workspaceId: 'workspace-3',
		});
		client.dispose();
		channel.port2.close();
	});
});
