import {
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import { EnvironmentIdSchema } from '../../environment';
import { resolveDesktopSshEnvironmentRemoteTerminalCreationTarget } from '../resolveDesktopSshEnvironmentRemoteTerminalCreationTarget';

const environmentId = EnvironmentIdSchema.parse('environment:00000000000000000000000000000001');

describe('resolveDesktopSshEnvironmentRemoteTerminalCreationTarget', () => {
	it('derives only the canonical remote Workspace target', () => {
		const document = remoteDocument();

		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				document,
				'project-1',
				'workspace-1',
			),
		).toMatchObject({
			directoryPath: '/srv/orchbee',
			environmentId,
			kind: 'resolved',
		});
		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				document,
				'project-missing',
				'workspace-1',
			),
		).toEqual({ kind: 'rejected', reason: 'project-not-found' });
		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				document,
				'project-1',
				'workspace-missing',
			),
		).toEqual({ kind: 'rejected', reason: 'workspace-not-found' });
	});

	it('rejects a local primary Directory instead of deriving local fallback authority', () => {
		const document = remoteDocument();
		const project = document.projectWorkspace.projects[0];
		const directory = project?.directories[0];
		if (project === undefined || directory === undefined) {
			throw new Error('Expected the remote Project fixture.');
		}
		const localDocument: WorkbenchSharedDocument = {
			...document,
			projectWorkspace: {
				...document.projectWorkspace,
				projects: [
					{
						...project,
						directories: [{ ...directory, available: true, placement: { kind: 'local' } }],
					},
				],
			},
		};

		expect(
			resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
				localDocument,
				'project-1',
				'workspace-1',
			),
		).toEqual({ kind: 'rejected', reason: 'workspace-environment-mismatch' });
	});
});

function remoteDocument(): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 2,
			nextTerminalContextSequence: 3,
			nextWorkspaceSequence: 2,
			projects: [
				{
					directories: [
						{
							available: false,
							git: null,
							id: 'environment-directory-1',
							kind: 'folder',
							name: 'orchbee',
							path: '/srv/orchbee',
							placement: { environmentId, kind: 'environment' },
							source: 'selected',
						},
					],
					id: 'project-1',
					launchConfigurations: [],
					name: 'orchbee',
					primaryDirectoryId: 'environment-directory-1',
					workspaces: [
						{
							id: 'workspace-1',
							launchConfigurationId: null,
							name: 'orchbee',
							primaryDirectoryId: 'environment-directory-1',
							terminalContextId: 'terminal-context-2',
						},
					],
				},
			],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('4'),
		terminalContexts: [
			createEmptyWorkbenchSharedTerminalContext('terminal-context-1'),
			createEmptyWorkbenchSharedTerminalContext('terminal-context-2'),
		],
	});
}
