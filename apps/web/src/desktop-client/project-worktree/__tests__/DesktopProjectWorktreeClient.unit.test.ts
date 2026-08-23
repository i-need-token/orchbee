import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import { DesktopProjectWorktreeClient } from '../DesktopProjectWorktreeClient';

describe('DesktopProjectWorktreeClient', () => {
	it('discovers and creates through typed request identities', async () => {
		const channel = new MessageChannel();
		channel.port2.addEventListener('message', (event) => {
			const command = DesktopProjectWorktreeRendererCommandSchema.parse(event.data);
			channel.port2.postMessage(
				command.type === 'desktop-project-worktree-discovery-requested'
					? DesktopProjectWorktreeMainEventSchema.parse({
							discovery: { managedRoot: '/managed', repositories: [] },
							requestId: command.requestId,
							type: 'desktop-project-worktree-discovery-completed',
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
			client.discover({ preferredDirectoryId: null, projectId: 'project-1' }),
		).resolves.toEqual({
			discovery: { managedRoot: '/managed', repositories: [] },
			kind: 'discovered',
		});
		await expect(
			client.create({
				baseRef: 'main',
				branchName: 'feature',
				createWorkspace: true,
				projectId: 'project-1',
				repositoryId: 'repository-1',
				sourceDirectoryId: 'directory-1',
			}),
		).resolves.toEqual({
			directoryId: 'directory-2',
			kind: 'created',
			workspaceId: 'workspace-2',
		});
		client.dispose();
		channel.port2.close();
	});
});
