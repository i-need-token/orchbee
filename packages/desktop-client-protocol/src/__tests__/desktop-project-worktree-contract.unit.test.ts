import { describe, expect, it } from 'vitest';

import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
} from '../desktop-project-worktree-contract';

describe('Desktop Project Worktree contract', () => {
	it('accepts discovery, creation, and adoption messages without renderer filesystem paths', () => {
		expect(
			DesktopProjectWorktreeRendererCommandSchema.parse({
				preferredDirectoryId: 'directory-1',
				sourceWorkspaceId: 'workspace-1',
				requestId: 'worktree:1',
				type: 'desktop-project-worktree-discovery-requested',
			}),
		).not.toHaveProperty('path');
		expect(
			DesktopProjectWorktreeRendererCommandSchema.parse({
				baseRef: 'main',
				branchName: 'orchbee/workspace-1',
				createWorkspace: true,
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				requestId: 'worktree:2',
				sourceDirectoryId: 'directory-1',
				type: 'desktop-project-worktree-creation-requested',
			}),
		).not.toHaveProperty('path');
		expect(
			DesktopProjectWorktreeRendererCommandSchema.parse({
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				requestId: 'worktree:3',
				sourceDirectoryId: 'directory-1',
				type: 'desktop-project-worktree-adoption-requested',
				worktreeId: 'worktree-1',
			}),
		).not.toHaveProperty('path');
	});

	it('accepts a discovered multi-repository catalog and a typed failure', () => {
		expect(
			DesktopProjectWorktreeMainEventSchema.safeParse({
				discovery: {
					managedRoot: '/Users/person/.orchbee/worktrees',
					repositories: [
						{
							defaultBaseRef: 'main',
							label: 'orchbee',
							localBaseRefs: ['main', 'feature'],
							repositoryId: 'repository-1',
							sourceDirectoryId: 'directory-1',
							suggestedBranchName: 'orchbee/workspace-1',
							worktreeInventory: {
								kind: 'available',
								worktrees: [{ label: 'feature/one', worktreeId: 'worktree-1' }],
							},
						},
					],
				},
				requestId: 'worktree:1',
				type: 'desktop-project-worktree-discovery-completed',
			}).success,
		).toBe(true);
		expect(
			DesktopProjectWorktreeMainEventSchema.safeParse({
				requestId: 'worktree:3',
				result: { directoryId: 'discovered-worktree-1', workspaceId: 'workspace-2' },
				type: 'desktop-project-worktree-adoption-completed',
			}).success,
		).toBe(true);
		expect(
			DesktopProjectWorktreeMainEventSchema.safeParse({
				code: 'worktree-retained',
				operation: 'creation',
				requestId: 'worktree:2',
				type: 'desktop-project-worktree-failed',
			}).success,
		).toBe(true);
	});

	it('rejects line-separated Git arguments', () => {
		expect(
			DesktopProjectWorktreeRendererCommandSchema.safeParse({
				baseRef: 'main\n--force',
				branchName: 'orchbee/workspace-1',
				createWorkspace: true,
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				requestId: 'worktree:2',
				sourceDirectoryId: 'directory-1',
				type: 'desktop-project-worktree-creation-requested',
			}).success,
		).toBe(false);
		expect(
			DesktopProjectWorktreeRendererCommandSchema.safeParse({
				path: '/renderer/must-not-submit-this',
				sourceWorkspaceId: 'workspace-1',
				repositoryId: 'repository-1',
				requestId: 'worktree:3',
				sourceDirectoryId: 'directory-1',
				type: 'desktop-project-worktree-adoption-requested',
				worktreeId: 'worktree-1',
			}).success,
		).toBe(false);
	});
});
