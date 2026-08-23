import { describe, expect, it } from 'vitest';

import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
} from '../desktop-project-worktree-contract';

describe('Desktop Project Worktree contract', () => {
	it('accepts discovery and creation messages without renderer filesystem paths', () => {
		expect(
			DesktopProjectWorktreeRendererCommandSchema.parse({
				preferredDirectoryId: 'directory-1',
				projectId: 'project-1',
				requestId: 'worktree:1',
				type: 'desktop-project-worktree-discovery-requested',
			}),
		).not.toHaveProperty('path');
		expect(
			DesktopProjectWorktreeRendererCommandSchema.parse({
				baseRef: 'main',
				branchName: 'orchbee/workspace-1',
				createWorkspace: true,
				projectId: 'project-1',
				repositoryId: 'repository-1',
				requestId: 'worktree:2',
				sourceDirectoryId: 'directory-1',
				type: 'desktop-project-worktree-creation-requested',
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
						},
					],
				},
				requestId: 'worktree:1',
				type: 'desktop-project-worktree-discovery-completed',
			}).success,
		).toBe(true);
		expect(
			DesktopProjectWorktreeMainEventSchema.safeParse({
				code: 'branch-already-exists',
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
				projectId: 'project-1',
				repositoryId: 'repository-1',
				requestId: 'worktree:2',
				sourceDirectoryId: 'directory-1',
				type: 'desktop-project-worktree-creation-requested',
			}).success,
		).toBe(false);
	});
});
