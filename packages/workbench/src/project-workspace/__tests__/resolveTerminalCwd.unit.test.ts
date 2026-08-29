import { describe, expect, it } from 'vitest';

import type { WorkbenchDirectory } from '../project-workspace-state';
import { resolveTerminalCwd } from '../resolveTerminalCwd';

const DIRECTORIES: readonly WorkbenchDirectory[] = [
	{
		available: true,
		git: null,
		id: 'directory-a',
		kind: 'folder',
		name: 'alpha',
		path: '/projects/alpha',
		placement: { kind: 'local' },
		source: 'selected',
	},
	{
		available: true,
		git: null,
		id: 'directory-b',
		kind: 'folder',
		name: 'beta',
		path: '/projects/beta',
		placement: { kind: 'local' },
		source: 'created',
	},
	{
		available: false,
		git: null,
		id: 'directory-c',
		kind: 'folder',
		name: 'missing',
		path: '/projects/missing',
		placement: { kind: 'local' },
		source: 'selected',
	},
	{
		available: false,
		git: null,
		id: 'directory-remote',
		kind: 'folder',
		name: 'remote',
		path: '/srv/remote',
		placement: {
			environmentId: 'environment:00000000000000000000000000000001',
			kind: 'environment',
		},
		source: 'selected',
	},
];

describe('resolveTerminalCwd', () => {
	it('applies the complete accepted precedence order', () => {
		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				explicitDirectoryId: 'directory-b',
				projectPrimaryDirectoryId: 'directory-b',
				splitCwd: '/split',
				systemCwd: '/system',
				workspacePrimaryDirectoryId: 'directory-a',
			}),
		).toEqual({ cwd: '/projects/beta', kind: 'resolved', source: 'explicit-directory' });

		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				projectPrimaryDirectoryId: 'directory-b',
				splitCwd: '/split',
				systemCwd: '/system',
				workspacePrimaryDirectoryId: 'directory-a',
			}),
		).toEqual({ cwd: '/split', kind: 'resolved', source: 'split-terminal' });

		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				projectPrimaryDirectoryId: 'directory-b',
				systemCwd: '/system',
				workspacePrimaryDirectoryId: 'directory-a',
			}),
		).toEqual({
			cwd: '/projects/alpha',
			kind: 'resolved',
			source: 'workspace-primary-directory',
		});

		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				projectPrimaryDirectoryId: 'directory-b',
				systemCwd: '/system',
				workspacePrimaryDirectoryId: 'directory-c',
			}),
		).toEqual({
			cwd: '/projects/beta',
			kind: 'resolved',
			source: 'project-primary-directory',
		});

		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				projectPrimaryDirectoryId: 'directory-c',
				systemCwd: '/system',
			}),
		).toEqual({ cwd: '/system', kind: 'resolved', source: 'system' });
	});

	it('rejects an unavailable or unknown explicit Directory instead of silently falling back', () => {
		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				explicitDirectoryId: 'directory-c',
				systemCwd: '/system',
			}),
		).toEqual({ kind: 'rejected', reason: 'explicit-directory-unavailable' });
		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				explicitDirectoryId: 'unknown',
				systemCwd: '/system',
			}),
		).toEqual({ kind: 'rejected', reason: 'explicit-directory-not-found' });
	});

	it('rejects a remote Workspace Directory instead of falling back locally', () => {
		expect(
			resolveTerminalCwd({
				directories: DIRECTORIES,
				projectPrimaryDirectoryId: 'directory-a',
				systemCwd: '/system',
				workspacePrimaryDirectoryId: 'directory-remote',
			}),
		).toEqual({ kind: 'rejected', reason: 'workspace-directory-remote' });
	});
});
