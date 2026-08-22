import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_ACTION_IDS,
	workbenchProjectCreateInput,
	workbenchProjectDirectoryInput,
	workbenchProjectFolderAddInput,
	workbenchProjectInput,
	workbenchProjectRenameInput,
	workbenchProjectReorderInput,
	workbenchSearchOpenInput,
	workbenchTabStripOrientationInput,
	workbenchTerminalCreateInput,
	workbenchWorkspaceCreateInput,
	workbenchWorkspaceInput,
	workbenchWorkspaceReorderInput,
	workbenchWorkspaceUpdateInput,
} from '../workbench-action-ids';

describe('workbench action contracts', () => {
	it('owns stable semantic action ids', () => {
		expect(WORKBENCH_ACTION_IDS).toEqual({
			activateStandalone: 'workbench.location.standalone.activate',
			activateWorkspace: 'workbench.workspace.activate',
			addProjectFolder: 'workbench.project.directory.add',
			createProject: 'workbench.project.create',
			createTerminal: 'workbench.terminal.create',
			createWorkspace: 'workbench.workspace.create',
			closeActiveTerminal: 'workbench.terminal.active.close',
			deleteWorkspace: 'workbench.workspace.delete',
			duplicateWorkspace: 'workbench.workspace.duplicate',
			openSearch: 'workbench.search.open',
			openSettings: 'workbench.settings.open',
			removeProject: 'workbench.project.remove',
			removeProjectDirectory: 'workbench.project.directory.remove',
			renameProject: 'workbench.project.rename',
			reorderProject: 'workbench.project.reorder',
			reorderWorkspace: 'workbench.workspace.reorder',
			splitActivePanelBelow: 'workbench.panel.active.split-below',
			splitActivePanelRight: 'workbench.panel.active.split-right',
			setTabStripOrientation: 'workbench.tab-strip.orientation.set',
			setProjectPrimaryDirectory: 'workbench.project.primary-directory.set',
			toggleActivePanelMaximization: 'workbench.panel.active.maximization.toggle',
			toggleProjectExpansion: 'workbench.project.expansion.toggle',
			toggleSidebar: 'workbench.sidebar.toggle',
			updateWorkspace: 'workbench.workspace.update',
		});
	});

	it('trims a non-empty Folder path for Project creation', () => {
		expect(workbenchProjectCreateInput.parse({ path: '  /workspace/orchbee  ' })).toEqual({
			data: { path: '/workspace/orchbee' },
			success: true,
		});
		expect(workbenchProjectCreateInput.parse({ path: '   ' })).toEqual({
			code: 'expected-workbench-project-create-input',
			success: false,
		});
	});

	it('trims a non-empty Folder path with its Project target for Folder addition', () => {
		expect(
			workbenchProjectFolderAddInput.parse({
				path: '  /workspace/xterm.js  ',
				projectId: 'project-1',
			}),
		).toEqual({
			data: { path: '/workspace/xterm.js', projectId: 'project-1' },
			success: true,
		});
		expect(workbenchProjectFolderAddInput.parse({ path: '   ', projectId: 'project-1' })).toEqual({
			code: 'expected-workbench-project-folder-add-input',
			success: false,
		});
	});

	it('copies stable Project and Workspace target identities at the action boundary', () => {
		expect(workbenchProjectInput.parse({ projectId: 'project-1' })).toEqual({
			data: { projectId: 'project-1' },
			success: true,
		});
		expect(
			workbenchWorkspaceInput.parse({
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({
			data: { projectId: 'project-1', workspaceId: 'workspace-1' },
			success: true,
		});
		expect(
			workbenchWorkspaceCreateInput.parse({
				directoryId: 'directory-1',
				projectId: 'project-1',
			}),
		).toEqual({
			data: { directoryId: 'directory-1', projectId: 'project-1' },
			success: true,
		});
		expect(workbenchWorkspaceInput.parse({ projectId: 'project-1' })).toMatchObject({
			success: false,
		});
		expect(
			workbenchProjectDirectoryInput.parse({
				directoryId: 'directory-1',
				projectId: 'project-1',
			}),
		).toMatchObject({ success: true });
		expect(
			workbenchProjectRenameInput.parse({ name: '  Orchbee ', projectId: 'project-1' }),
		).toEqual({
			data: { name: 'Orchbee', projectId: 'project-1' },
			success: true,
		});
		expect(
			workbenchProjectReorderInput.parse({
				beforeProjectId: 'project-2',
				projectId: 'project-1',
			}),
		).toEqual({
			data: { beforeProjectId: 'project-2', projectId: 'project-1' },
			success: true,
		});
		expect(
			workbenchProjectReorderInput.parse({ beforeProjectId: null, projectId: 'project-1' }),
		).toMatchObject({ success: true });
		expect(
			workbenchWorkspaceReorderInput.parse({
				beforeWorkspaceId: 'workspace-2',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({
			data: {
				beforeWorkspaceId: 'workspace-2',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			},
			success: true,
		});
		expect(
			workbenchWorkspaceReorderInput.parse({
				beforeWorkspaceId: null,
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toMatchObject({ success: true });
		expect(
			workbenchWorkspaceUpdateInput.parse({
				name: ' Feature ',
				primaryDirectoryId: 'directory-1',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({
			data: {
				name: 'Feature',
				primaryDirectoryId: 'directory-1',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			},
			success: true,
		});
	});

	it('parses only supported unified-search filters', () => {
		expect(workbenchSearchOpenInput.parse({ filter: 'all' })).toEqual({
			success: true,
			data: { filter: 'all' },
		});
		expect(workbenchSearchOpenInput.parse({ filter: 'actions' })).toEqual({
			success: true,
			data: { filter: 'actions' },
		});
		expect(workbenchSearchOpenInput.parse({ filter: 'files' })).toEqual({
			success: false,
			code: 'expected-workbench-search-filter',
		});
	});

	it('accepts only supported tab-strip orientations', () => {
		expect(workbenchTabStripOrientationInput.parse({ orientation: 'horizontal' })).toEqual({
			success: true,
			data: { orientation: 'horizontal' },
		});
		expect(workbenchTabStripOrientationInput.parse({ orientation: 'diagonal' })).toEqual({
			success: false,
			code: 'expected-tab-strip-orientation',
		});
	});

	it('defaults terminal creation to the ungrouped trailing position and validates placement', () => {
		expect(workbenchTerminalCreateInput.parse(undefined)).toEqual({
			success: true,
			data: { groupId: null },
		});
		expect(workbenchTerminalCreateInput.parse({ groupId: 'group-1', index: 2 })).toMatchObject({
			success: true,
		});
		expect(
			workbenchTerminalCreateInput.parse({
				directoryId: 'directory-1',
				groupId: null,
			}),
		).toEqual({
			data: { directoryId: 'directory-1', groupId: null },
			success: true,
		});
		expect(workbenchTerminalCreateInput.parse({ directoryId: '', groupId: null })).toMatchObject({
			success: false,
		});
		expect(workbenchTerminalCreateInput.parse({ groupId: null, index: -1 })).toMatchObject({
			success: false,
		});
	});
});
