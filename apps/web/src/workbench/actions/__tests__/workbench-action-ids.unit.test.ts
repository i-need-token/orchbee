import { describe, expect, it } from 'vitest';
import registry from '../../../../../../schemas/workbench/v1/workbench-action-identities-v1.json';

import {
	WORKBENCH_ACTION_IDS,
	workbenchAgentNavigateInput,
	workbenchProjectCreateInput,
	workbenchProjectInput,
	workbenchProjectRenameInput,
	workbenchProjectReorderInput,
	workbenchPanelPositionInput,
	workbenchSearchOpenInput,
	workbenchTabPositionInput,
	workbenchTabTitleInput,
	workbenchTerminalCreateInput,
	workbenchWorkspaceCreateInput,
	workbenchWorkspaceInput,
	workbenchWorkspaceReorderInput,
	workbenchWorkspaceUpdateInput,
} from '../workbench-action-ids';

describe('workbench action contracts', () => {
	it('owns stable semantic action ids', () => {
		expect(WORKBENCH_ACTION_IDS).toMatchObject({
			navigateAgent: 'workbench.agent.navigate',
			activateWorkspace: 'workbench.workspace.activate',
			createProject: 'workbench.project.create',
			newProject: 'workbench.project.new',
			createTerminal: 'workbench.terminal.create',
			createTerminalInCurrentDirectory: 'workbench.terminal.create-current-directory',
			createWorkspace: 'workbench.workspace.create',
			closeActiveTerminal: 'workbench.terminal.active.close',
			closeTabsAfterActive: 'workbench.tab.active.close-after',
			closeWorkspaceConfirmed: 'workbench.workspace.close-confirmed',
			closeTabConfirmed: 'workbench.tab.close-confirmed',
			createTabAfterActive: 'workbench.tab.active.create-after',
			createTabInRecentGroup: 'workbench.tab.recent-group.create',
			decreaseTerminalFontSize: 'workbench.terminal.font-size.decrease',
			deleteWorkspace: 'workbench.workspace.delete',
			duplicateActiveTabs: 'workbench.tab.active.duplicate',
			equalizeActivePanels: 'workbench.panel.active.equalize',
			focusActiveTerminal: 'workbench.focus.active-terminal',
			focusActivePanelByPosition: 'workbench.panel.active.focus-by-position',
			focusActivePanelDown: 'workbench.panel.active.focus-down',
			focusActivePanelLeft: 'workbench.panel.active.focus-left',
			focusActivePanelNext: 'workbench.panel.active.focus-next',
			focusActivePanelPrevious: 'workbench.panel.active.focus-previous',
			focusActivePanelRight: 'workbench.panel.active.focus-right',
			focusActivePanelUp: 'workbench.panel.active.focus-up',
			focusActivePanelVisible: 'workbench.panel.active.focus-visible',
			focusNextRegion: 'workbench.focus.next-region',
			focusPreviousRegion: 'workbench.focus.previous-region',
			focusSidebar: 'workbench.focus.sidebar',
			focusTabStrip: 'workbench.focus.tab-strip',
			increaseTerminalFontSize: 'workbench.terminal.font-size.increase',
			openAgentBoard: 'workbench.agent-board.open',
			openSearch: 'workbench.search.open',
			openSettings: 'workbench.settings.open',
			openShortcuts: 'workbench.shortcuts.open',
			removeProject: 'workbench.project.dissolve',
			setWorkspaceProject: 'workbench.workspace.set-project',
			createWorkspaceProject: 'workbench.workspace.create-project',
			closeProjectWorkspaces: 'workbench.project.close-all',
			renameProject: 'workbench.project.rename',
			reorderProject: 'workbench.project.reorder',
			reorderWorkspace: 'workbench.workspace.reorder',
			moveActiveTabBackward: 'workbench.tab.active.move-backward',
			moveActiveTabForward: 'workbench.tab.active.move-forward',
			moveActivePanelDown: 'workbench.panel.active.move-down',
			moveActivePanelLeft: 'workbench.panel.active.move-left',
			moveActivePanelRight: 'workbench.panel.active.move-right',
			moveActivePanelUp: 'workbench.panel.active.move-up',
			resumeTabClose: 'workbench.tab.close.resume',
			setTabTitle: 'workbench.tab.title.set',
			renamePanel: 'workbench.panel.rename',
			resetTerminalFontSize: 'workbench.terminal.font-size.reset',
			resizeActivePanel: 'workbench.panel.active.resize',
			selectTabByPosition: 'workbench.tab.select-by-position',
			selectNextTab: 'workbench.tab.select-next',
			selectPreviousTab: 'workbench.tab.select-previous',
			splitActivePanelAbove: 'workbench.panel.active.split-above',
			splitActivePanelBelow: 'workbench.panel.active.split-below',
			splitActivePanelLeft: 'workbench.panel.active.split-left',
			splitActivePanelRight: 'workbench.panel.active.split-right',
			swapActivePanelVisible: 'workbench.panel.active.swap-visible',
			toggleActivePanelMaximization: 'workbench.panel.active.maximization.toggle',
			toggleProjectExpansion: 'workbench.project.expansion.toggle',
			toggleRecentsExpansion: 'workbench.recents.expansion.toggle',
			toggleSidebar: 'workbench.sidebar.toggle',
			updateWorkspace: 'workbench.workspace.update',
			createWorktree: 'workbench.worktree.create',
			createFocusedPanelWorktree: 'workbench.worktree.active.create',
			openWorktree: 'workbench.worktree.open',
			openFocusedPanelWorktree: 'workbench.worktree.active.open',
			removeWorktree: 'workbench.worktree.remove',
			resumeWorktreeRemoval: 'workbench.worktree.resume-removal',
		});
	});

	it('owns every Workbench identity in the shared cross-Client registry', () => {
		expect(registry.contractVersion).toBe(1);
		const owned = new Set<string>(Object.values(WORKBENCH_ACTION_IDS));
		expect(registry.catalogs.workbench.filter((id) => !owned.has(id))).toEqual([]);
	});

	it('validates exact Agent navigation identity without accepting projected topology', () => {
		expect(
			workbenchAgentNavigateInput.parse({
				presentationId: 'terminal-a',
				workspaceId: 'workspace-a',
			}),
		).toEqual({
			data: {
				presentationId: 'terminal-a',
				workspaceId: 'workspace-a',
			},
			success: true,
		});
		for (const stale of [{ panelId: 'stale-panel' }, { sessionId: 'session-a' }]) {
			expect(workbenchAgentNavigateInput.parse({ ...stale, workspaceId: 'workspace-a' })).toEqual({
				code: 'expected-workbench-agent-navigate-input',
				success: false,
			});
		}
	});

	it('accepts only the first ten one-based Panel positions', () => {
		for (let position = 1; position <= 10; position += 1) {
			expect(workbenchPanelPositionInput.parse({ position })).toEqual({
				data: { position },
				success: true,
			});
		}
		for (const position of [0, 11, 1.5, '2']) {
			expect(workbenchPanelPositionInput.parse({ position })).toEqual({
				code: 'expected-workbench-panel-position-input',
				success: false,
			});
		}
	});

	it('accepts only the first nine one-based Tab positions', () => {
		for (let position = 1; position <= 9; position += 1) {
			expect(workbenchTabPositionInput.parse({ position })).toEqual({
				data: { position },
				success: true,
			});
		}
		for (const position of [0, 10, 1.5, '2']) {
			expect(workbenchTabPositionInput.parse({ position })).toEqual({
				code: 'expected-workbench-tab-position-input',
				success: false,
			});
		}
	});

	it('accepts discovery, explicit open, and nullable commit Tab-title inputs', () => {
		expect(workbenchTabTitleInput.parse(undefined)).toEqual({ data: undefined, success: true });
		expect(
			workbenchTabTitleInput.parse({
				kind: 'open',
				tabId: 'tab-2',
				terminalContextId: 'terminal-context-1',
			}),
		).toMatchObject({ success: true });
		expect(
			workbenchTabTitleInput.parse({
				kind: 'commit',
				tabId: 'tab-2',
				terminalContextId: 'terminal-context-1',
				title: null,
			}),
		).toMatchObject({ success: true });
		for (const input of [
			{ kind: 'open', tabId: '', terminalContextId: 'terminal-context-1' },
			{ kind: 'commit', tabId: 'tab-2', terminalContextId: '', title: 'Title' },
			{ kind: 'commit', tabId: 'tab-2', terminalContextId: 'terminal-context-1' },
		]) {
			expect(workbenchTabTitleInput.parse(input)).toEqual({
				code: 'expected-workbench-tab-title-input',
				success: false,
			});
		}
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
			data: { workspaceId: 'workspace-1' },
			success: true,
		});
		expect(
			workbenchWorkspaceCreateInput.parse({
				directoryId: 'directory-1',
				projectId: 'project-1',
			}),
		).toEqual({
			data: {
				directoryId: 'directory-1',
				projectId: 'project-1',
			},
			success: true,
		});
		expect(workbenchWorkspaceInput.parse({ projectId: 'project-1' })).toMatchObject({
			success: false,
		});
		// Names keep their exact text, as the protocol and the TUI keep them; only blank ones fail.
		expect(
			workbenchProjectRenameInput.parse({ name: '  Orchbee ', projectId: 'project-1' }),
		).toEqual({
			data: { name: '  Orchbee ', projectId: 'project-1' },
			success: true,
		});
		expect(
			workbenchProjectRenameInput.parse({ name: '  　', projectId: 'project-1' }),
		).toMatchObject({ success: false });
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
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({
			data: {
				name: ' Feature ',
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
		const profile = {
			command: { arguments: ['-l'], executable: '/bin/zsh', kind: 'custom-executable' },
			id: 'profile-1',
			name: 'Zsh',
		};
		expect(workbenchTerminalCreateInput.parse({ groupId: null, profile })).toEqual({
			data: { groupId: null, profile },
			success: true,
		});
		expect(
			workbenchTerminalCreateInput.parse({ groupId: null, profile: { ...profile, name: null } }),
		).toMatchObject({ success: false });
	});
});
