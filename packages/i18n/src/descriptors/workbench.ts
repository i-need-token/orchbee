import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	launchRequestTooLarge: {
		description:
			'A deterministic launch frame overflow; edit arguments or environment before retrying.',
		id: 'workbench.launch.requestTooLarge',
		defaultMessage: enUSWorkbenchMessages['workbench.launch.requestTooLarge'],
	},
	workspaceMoveToProject: {
		description: 'Workspace grouping submenu.',
		id: 'workbench.workspaces.moveToProject',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.moveToProject'],
	},
	workspaceCreateProject: {
		description: 'Atomically create a group and move this Workspace.',
		id: 'workbench.workspaces.createProject',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.createProject'],
	},
	workspaceUngroup: {
		description: 'Keep the Workspace running without a group.',
		id: 'workbench.workspaces.ungroup',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.ungroup'],
	},
	workspaceMenuClose: {
		description: 'Workspace menu row that closes that Workspace; the menu title names it.',
		id: 'workbench.workspaces.menu.close',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.menu.close'],
	},
	workspaceMenuNewProject: {
		description:
			'Move to Project row that asks for a name, creates the Project and moves the Workspace.',
		id: 'workbench.workspaces.menu.newProject',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.menu.newProject'],
	},
	workspaceMenuRename: {
		description: 'Workspace menu row that renames that Workspace; the menu title names it.',
		id: 'workbench.workspaces.menu.rename',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.menu.rename'],
	},
	workspaceRenameTitle: {
		description: 'Title of the dialog that renames a Workspace; it also names its only field.',
		id: 'workbench.workspaces.rename.title',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.rename.title'],
	},
	projectMenuDissolve: {
		description: 'Project menu row that ungroups its Workspaces and removes only the Project.',
		id: 'workbench.projects.menu.dissolve',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.menu.dissolve'],
	},
	projectMenuRename: {
		description: 'Project menu row that renames that Project; the menu title names it.',
		id: 'workbench.projects.menu.rename',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.menu.rename'],
	},
	projectRenameTitle: {
		description: 'Title of the dialog that renames a Project; it also names its only field.',
		id: 'workbench.projects.rename.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.rename.title'],
	},
	nameRequired: {
		description: 'Field error when a Project or Workspace name has no visible character.',
		id: 'workbench.names.required',
		defaultMessage: enUSWorkbenchMessages['workbench.names.required'],
	},
	nameTooLong: {
		description:
			'Field error when a Project, Workspace or Tab group name is longer than the label limit.',
		id: 'workbench.names.tooLong',
		defaultMessage: enUSWorkbenchMessages['workbench.names.tooLong'],
	},
	renameSubmit: {
		description: 'Submit button of the Project, Workspace, Tab and Tab group rename dialogs.',
		id: 'workbench.names.renameSubmit',
		defaultMessage: enUSWorkbenchMessages['workbench.names.renameSubmit'],
	},
	panelRenameTitle: {
		description: 'Title and Action name for renaming the primary Presentation of a Panel.',
		id: 'workbench.panels.rename.title',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.rename.title'],
	},
	panelRenameMenu: {
		description: 'Rename verb in the menu of a Panel.',
		id: 'workbench.panels.rename.menu',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.rename.menu'],
	},
	panelRenameHelp: {
		description: 'Explains that blank clears the primary Presentation name override.',
		id: 'workbench.panels.rename.help',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.rename.help'],
	},
	projectCloseAll: {
		description: 'Explicitly close every Workspace currently in this Project.',
		id: 'workbench.projects.closeAll',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.closeAll'],
	},
	projectCloseAllDescription: {
		description: 'Confirmation for closing all current Project members.',
		id: 'workbench.projects.closeAll.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.closeAll.description'],
	},
	projectCloseAllTitle: {
		description: 'Title of the confirmation before closing the Workspaces of a Project.',
		id: 'workbench.projects.closeAll.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.closeAll.title'],
	},
	projectCloseClosed: {
		description: 'Status notice after every Workspace of a Project closed.',
		id: 'workbench.projects.close.closed',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.close.closed'],
	},
	projectCloseClosing: {
		description: 'Status notice while the Workspaces of a Project close.',
		id: 'workbench.projects.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.close.closing'],
	},
	projectCloseOutcomeUnknown: {
		description:
			'Status notice when closing the Workspaces of a Project may have taken effect, naming the command that reads its status.',
		id: 'workbench.projects.close.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.close.outcomeUnknown'],
	},
	projectCloseRefused: {
		description:
			'Status notice when closing the Workspaces of a Project was not submitted or was refused.',
		id: 'workbench.projects.close.refused',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.close.refused'],
	},
	projectCloseStopped: {
		description: 'Status notice when an accepted close of a Project stopped before finishing.',
		id: 'workbench.projects.close.stopped',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.close.stopped'],
	},
	sidebarNewWorkspace: {
		description: 'Fixed full-row sidebar action that creates an independent Workspace.',
		id: 'workbench.sidebar.items.newWorkspace',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.newWorkspace'],
	},

	projectNewFolderTitle: {
		description: 'Title of the native-parent and single-child-name creation dialog.',
		id: 'workbench.projects.newFolder.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.title'],
	},
	projectNewFolderDescription: {
		description: 'Explains explicit creation under the displayed local parent.',
		id: 'workbench.projects.newFolder.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.description'],
	},
	projectNewFolderParent: {
		description: 'Label for the native-selected parent path.',
		id: 'workbench.projects.newFolder.parent',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.parent'],
	},
	projectNewFolderName: {
		description: 'Label for the single new child folder name.',
		id: 'workbench.projects.newFolder.name',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.name'],
	},
	projectNewFolderNameHint: {
		description: 'Name boundary guidance before explicit filesystem creation.',
		id: 'workbench.projects.newFolder.nameHint',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.nameHint'],
	},
	projectNewFolderCreating: {
		description: 'Pending non-cancellable directory creation status.',
		id: 'workbench.projects.newFolder.creating',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.creating'],
	},
	projectNewFolderOpening: {
		description: 'Pending Project publication and exact Workspace activation status.',
		id: 'workbench.projects.newFolder.opening',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.opening'],
	},
	projectNewFolderRetained: {
		description:
			'Recovery after known directory creation without confirmed Project opening; also used by Main fallback.',
		id: 'workbench.projects.newFolder.retained',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.retained'],
	},
	projectNewFolderUnknown: {
		description:
			'Recovery when directory creation may have committed; never invite repeated creation.',
		id: 'workbench.projects.newFolder.unknown',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.newFolder.unknown'],
	},
	projectFolderAcquisitionAlreadyExists: {
		description: 'No-overwrite conflict; retain the chosen parent for name correction.',
		id: 'workbench.projects.folderAcquisition.failure.alreadyExists',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.alreadyExists'],
	},
	projectFolderAcquisitionInvalidName: {
		description: 'A new child name failed the bounded single-component validation.',
		id: 'workbench.projects.folderAcquisition.failure.invalidName',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.invalidName'],
	},
	projectFolderAcquisitionStaleParent: {
		description: 'The native parent selection no longer authorizes creation.',
		id: 'workbench.projects.folderAcquisition.failure.staleParent',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.staleParent'],
	},
	actionCategoryNavigation: {
		id: 'workbench.actions.categories.navigation',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.navigation'],
		description: 'Action category for navigation commands.',
	},
	actionCategoryPanels: {
		id: 'workbench.actions.categories.panels',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.panels'],
		description: 'Action category for Workbench panel commands.',
	},
	actionCategoryProjects: {
		id: 'workbench.actions.categories.projects',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.projects'],
		description: 'Action category for Project and Workspace commands.',
	},
	actionCategoryTerminal: {
		id: 'workbench.actions.categories.terminal',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.terminal'],
		description: 'Action category for terminal commands.',
	},
	actionCategoryView: {
		id: 'workbench.actions.categories.view',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.view'],
		description: 'Action category for Workbench view commands.',
	},
	focusActiveTerminal: {
		id: 'workbench.focus.actions.activeTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.actions.activeTerminal'],
		description: 'Action that focuses the displayed terminal in the active Workbench Panel.',
	},
	focusNextRegion: {
		id: 'workbench.focus.actions.nextRegion',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.actions.nextRegion'],
		description: 'Action that focuses the next available ordinary Workbench region.',
	},
	focusPreviousRegion: {
		id: 'workbench.focus.actions.previousRegion',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.actions.previousRegion'],
		description: 'Action that focuses the previous available ordinary Workbench region.',
	},
	focusSidebar: {
		id: 'workbench.focus.actions.sidebar',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.actions.sidebar'],
		description: 'Action that focuses the active destination in the visible Workbench sidebar.',
	},
	focusTabStrip: {
		id: 'workbench.focus.actions.tabStrip',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.actions.tabStrip'],
		description: 'Action that focuses the remembered item in the Workbench Tab Strip.',
	},
	hideSidebar: {
		id: 'workbench.sidebar.actions.hide',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.hide'],
		description: 'Action that hides the Workbench sidebar.',
	},
	newTerminal: {
		id: 'workbench.sidebar.items.newTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.newTerminal'],
		description: 'Label for the new-terminal entry in the Workbench sidebar.',
	},
	terminalCloseActive: {
		id: 'workbench.terminal.actions.closeActive',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.closeActive'],
		description: 'Action that closes the active child terminal, panel, or terminal Tab.',
	},
	terminalFontSizeDecrease: {
		id: 'workbench.terminal.actions.fontSize.decrease',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.fontSize.decrease'],
		description: 'Action that decreases the application-wide terminal font size.',
	},
	terminalFontSizeIncrease: {
		id: 'workbench.terminal.actions.fontSize.increase',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.fontSize.increase'],
		description: 'Action that increases the application-wide terminal font size.',
	},
	terminalFontSizeReset: {
		id: 'workbench.terminal.actions.fontSize.reset',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.fontSize.reset'],
		description: 'Action that restores the application-wide terminal font size.',
	},
	panelCancel: {
		id: 'workbench.panels.actions.cancel',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.cancel'],
		description: 'Action that cancels closing a running Workbench terminal.',
	},
	panelActivateChild: {
		id: 'workbench.panels.actions.activateChild',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.activateChild'],
		description: 'Action that activates a child terminal in a Workbench panel.',
	},
	panelChildTerminalsLabel: {
		id: 'workbench.panels.childTerminals.label',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.childTerminals.label'],
		description: 'Accessible label for the child-terminal list within a Workbench panel.',
	},
	panelClose: {
		id: 'workbench.panels.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.close'],
		description: 'Action that closes a Workbench panel and every terminal it holds.',
	},
	panelCloseLastPanel: {
		id: 'workbench.panels.close.lastPanel',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.close.lastPanel'],
		description: 'Close confirmation line saying that closing the last Panel also closes its Tab.',
	},
	panelCloseTab: {
		id: 'workbench.panels.close.tab',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.close.tab'],
		description: 'Close confirmation line naming the Tab that holds the Panel.',
	},
	panelCloseTitle: {
		id: 'workbench.panels.close.title',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.close.title'],
		description: 'Title for the confirmation before closing a Panel.',
	},
	terminalCloseWarning: {
		id: 'workbench.close.warning',
		defaultMessage: enUSWorkbenchMessages['workbench.close.warning'],
		description:
			'What a close does: ends the terminals still running, and discards the output of the ended ones.',
	},
	panelCloseNotApplied: {
		id: 'workbench.panels.close.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.close.notApplied'],
		description: 'Status notice when closing a Panel was not applied.',
	},
	panelsNotApplied: {
		id: 'workbench.panels.update.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.update.notApplied'],
		description:
			'Status notice when the Workbench authority refused creating, moving, or swapping a Panel.',
	},
	panelResizeContended: {
		id: 'workbench.panels.resize.contended',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.contended'],
		description:
			'Status notice when a Panel resize stopped trying because other Clients kept changing the Workbench.',
	},
	panelResizeFailed: {
		id: 'workbench.panels.resize.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.failed'],
		description: 'Status notice when the Workbench authority refused a Panel resize.',
	},
	panelResizeOutcomeUnknown: {
		id: 'workbench.panels.resize.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.outcomeUnknown'],
		description: 'Status notice when a Panel resize may or may not have taken effect.',
	},
	tabCloseAwaitingRemoval: {
		id: 'workbench.tabs.close.awaitingRemoval',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.awaitingRemoval'],
		description: 'Status notice when a closing Tab has ended but has not been removed yet.',
	},
	tabCloseAwaitingRemovalResumable: {
		id: 'workbench.tabs.close.awaitingRemovalResumable',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.awaitingRemovalResumable'],
		description:
			'Status notice when a closing Tab has ended but has not been removed yet, and Resume closing this Tab may continue its close.',
	},
	tabCloseCancelled: {
		id: 'workbench.tabs.close.cancelled',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.cancelled'],
		description:
			'Status notice when an open close confirmation was withdrawn because its target changed.',
	},
	tabCloseCleanupBlocked: {
		id: 'workbench.tabs.close.cleanupBlocked',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.cleanupBlocked'],
		description: 'Status notice when cleanup stopped a Tab from closing.',
	},
	tabCloseCleanupBlockedResumable: {
		id: 'workbench.tabs.close.cleanupBlockedResumable',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.cleanupBlockedResumable'],
		description:
			'Status notice when cleanup stopped a Tab from closing, and Resume closing this Tab may continue its close.',
	},
	tabCloseClosing: {
		id: 'workbench.tabs.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.closing'],
		description:
			'Status notice while the close of one Tab is still running a second after it began.',
	},
	tabCloseDescription: {
		id: 'workbench.tabs.close.description',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.description'],
		description: 'Confirmation text naming the Tabs that close, then what closing them does.',
	},
	tabCreateNotCreated: {
		id: 'workbench.tabs.create.notCreated',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.create.notCreated'],
		description: 'Status notice when the Workbench authority did not create a Tab.',
	},
	tabCreateProfileUnavailable: {
		id: 'workbench.tabs.create.profileUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.create.profileUnavailable'],
		description:
			'Status notice, worded as the terminal interface words it, when no Tab or terminal was created because the chosen launch profile changed, was removed, or cannot be started from this window.',
	},
	tabDuplicationChanged: {
		id: 'workbench.tabs.duplicate.changed',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.duplicate.changed'],
		description:
			'Status notice when duplicating Tabs stopped because the selection or Workspace changed.',
	},
	tabDuplicationDeadline: {
		id: 'workbench.tabs.duplicate.deadline',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.duplicate.deadline'],
		description: 'Status notice when duplicating Tabs stopped at its time limit.',
	},
	tabDuplicationNotCreated: {
		id: 'workbench.tabs.duplicate.notCreated',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.duplicate.notCreated'],
		description: 'Status notice when duplicating Tabs stopped because a copy was not created.',
	},
	tabDuplicationUncertain: {
		id: 'workbench.tabs.duplicate.uncertain',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.duplicate.uncertain'],
		description:
			'Status notice when duplicating Tabs stopped because a copy or its terminal has no definite result.',
	},
	tabsNotApplied: {
		id: 'workbench.tabs.update.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.update.notApplied'],
		description: 'Status notice when the Workbench authority refused a change to Tabs.',
	},
	tabCloseNotApplied: {
		id: 'workbench.tabs.close.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.notApplied'],
		description: 'Status notice when closing a Tab was not applied.',
	},
	tabCloseOutcomeUnknown: {
		id: 'workbench.tabs.close.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.outcomeUnknown'],
		description: 'Status notice when closing a Tab may have taken effect.',
	},
	tabCloseResume: {
		id: 'workbench.tabs.close.resume',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.resume'],
		description:
			'Action that continues a Tab close that stopped where it may continue, shown in the Tab menu and Search.',
	},
	tabCloseResumeDescription: {
		id: 'workbench.tabs.close.resumeDescription',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.resumeDescription'],
		description:
			'Confirmation text before continuing a stopped Tab close: the Tab, where its close stopped, then what closing it does.',
	},
	tabCloseResumeTitle: {
		id: 'workbench.tabs.close.resumeTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.resumeTitle'],
		description: 'Title of the confirmation before continuing a stopped Tab close.',
	},
	tabCloseSetProgress: {
		id: 'workbench.tabs.close.setProgress',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.setProgress'],
		description: 'Status notice counting the Tabs of a close set.',
	},
	tabCloseSetStopped: {
		id: 'workbench.tabs.close.setStopped',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.setStopped'],
		description: 'Status notice when closing a set of Tabs stopped before its end.',
	},
	tabCloseStatusUnavailable: {
		id: 'workbench.tabs.close.statusUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.statusUnavailable'],
		description: 'Status notice when a closing Tab can no longer be observed.',
	},
	tabCloseTitle: {
		id: 'workbench.tabs.close.title',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.title'],
		description: 'Title of the confirmation before closing one or several Tabs.',
	},
	panelMaximize: {
		id: 'workbench.panels.actions.maximize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.maximize'],
		description: 'Action that expands one Workbench panel to the tab content area.',
	},
	panelEqualize: {
		id: 'workbench.panels.actions.equalize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.equalize'],
		description: 'Action that redistributes Workbench panels into equal axis shares.',
	},
	panelFocus: {
		id: 'workbench.panels.actions.focus',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focus'],
		description: 'Action that focuses a Workbench panel.',
	},
	panelFocusDown: {
		id: 'workbench.panels.actions.focusDown',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusDown'],
		description: 'Action that focuses the neighboring Workbench panel below the active panel.',
	},
	panelFocusLeft: {
		id: 'workbench.panels.actions.focusLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusLeft'],
		description: 'Action that focuses the neighboring Workbench panel to the left.',
	},
	panelFocusByPosition: {
		id: 'workbench.panels.actions.focusByPosition',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusByPosition'],
		description: 'Action that focuses one exact Workbench panel position.',
	},
	panelFocusNext: {
		id: 'workbench.panels.actions.focusNext',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusNext'],
		description: 'Action that focuses the next Workbench panel in canonical layout order.',
	},
	panelFocusPrevious: {
		id: 'workbench.panels.actions.focusPrevious',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusPrevious'],
		description: 'Action that focuses the previous Workbench panel in canonical layout order.',
	},
	panelFocusLast: {
		id: 'workbench.panels.actions.focusLast',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusLast'],
		description:
			'Action that returns to the most recently focused other panel, in any tab or workspace.',
	},
	panelFocusRight: {
		id: 'workbench.panels.actions.focusRight',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusRight'],
		description: 'Action that focuses the neighboring Workbench panel to the right.',
	},
	panelFocusUp: {
		id: 'workbench.panels.actions.focusUp',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusUp'],
		description: 'Action that focuses the neighboring Workbench panel above the active panel.',
	},
	panelFocusVisible: {
		id: 'workbench.panels.actions.focusVisible',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focusVisible'],
		description: 'Action that visibly labels and focuses another Workbench panel.',
	},
	panelMenuMaximize: {
		id: 'workbench.panels.menu.maximize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.menu.maximize'],
		description:
			"Panel menu row, the verb alone, that expands the menu's panel to the tab content area.",
	},
	panelMenuRestore: {
		id: 'workbench.panels.menu.restore',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.menu.restore'],
		description:
			"Panel menu row, the verb alone, that returns the menu's maximized panel to the split layout.",
	},
	panelMove: {
		id: 'workbench.panels.actions.move',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.move'],
		description: 'Action that moves a Workbench panel relative to another panel.',
	},
	panelMoveDown: {
		id: 'workbench.panels.actions.moveDown',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.moveDown'],
		description: 'Action that moves the active Workbench panel toward the panel below it.',
	},
	panelMoveLeft: {
		id: 'workbench.panels.actions.moveLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.moveLeft'],
		description: 'Action that moves the active Workbench panel toward the panel to its left.',
	},
	panelMoveRight: {
		id: 'workbench.panels.actions.moveRight',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.moveRight'],
		description: 'Action that moves the active Workbench panel toward the panel to its right.',
	},
	panelMoveUp: {
		id: 'workbench.panels.actions.moveUp',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.moveUp'],
		description: 'Action that moves the active Workbench panel toward the panel above it.',
	},
	panelResize: {
		id: 'workbench.panels.actions.resize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.resize'],
		description: 'Action that opens keyboard resizing for the active Workbench panel.',
	},
	panelResizeDescription: {
		id: 'workbench.panels.resize.description',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.description'],
		description: 'Keyboard instructions shown while resizing a Workbench panel.',
	},
	panelResizeDone: {
		id: 'workbench.panels.resize.done',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.done'],
		description: 'Action that leaves Workbench panel keyboard resize mode.',
	},
	panelResizeNarrower: {
		id: 'workbench.panels.resize.narrower',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.narrower'],
		description: 'Action that makes the active Workbench panel narrower.',
	},
	panelResizeReset: {
		id: 'workbench.panels.resize.reset',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.reset'],
		description: 'Action that restores equal Workbench panel split weights.',
	},
	panelResizeShorter: {
		id: 'workbench.panels.resize.shorter',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.shorter'],
		description: 'Action that makes the active Workbench panel shorter.',
	},
	panelResizeTaller: {
		id: 'workbench.panels.resize.taller',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.taller'],
		description: 'Action that makes the active Workbench panel taller.',
	},
	panelResizeTitle: {
		id: 'workbench.panels.resize.title',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.title'],
		description: 'Title for Workbench panel keyboard resize mode.',
	},
	panelResizeUnavailable: {
		id: 'workbench.panels.resize.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.unavailable'],
		description: 'Status shown when a Workbench panel cannot be resized in one direction.',
	},
	panelResizeWider: {
		id: 'workbench.panels.resize.wider',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.resize.wider'],
		description: 'Action that makes the active Workbench panel wider.',
	},
	panelUnavailableFocusedPanelRequired: {
		id: 'workbench.panels.unavailable.focusedPanelRequired',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.focusedPanelRequired'],
		description:
			'Reason a Workbench Panel action is disabled: no Panel has focus in the active Tab.',
	},
	panelUnavailableLastPanelMissing: {
		id: 'workbench.panels.unavailable.lastPanelMissing',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.lastPanelMissing'],
		description:
			'Reason Back to Last Focused Panel is disabled: no other Panel this window focused still exists.',
	},
	panelUnavailableMaximized: {
		id: 'workbench.panels.unavailable.maximized',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.maximized'],
		description:
			'Reason a Workbench Panel action is disabled: a Panel of the active Tab is maximized.',
	},
	panelUnavailableMultipleRequired: {
		id: 'workbench.panels.unavailable.multipleRequired',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.multipleRequired'],
		description: 'Reason a Workbench Panel action is disabled: the active Tab has only one Panel.',
	},
	panelUnavailableNoneAbove: {
		id: 'workbench.panels.unavailable.noneAbove',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.noneAbove'],
		description:
			'Reason a directional Workbench Panel action is disabled: no Panel lies above the focused one.',
	},
	panelUnavailableNoneBelow: {
		id: 'workbench.panels.unavailable.noneBelow',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.noneBelow'],
		description:
			'Reason a directional Workbench Panel action is disabled: no Panel lies below the focused one.',
	},
	panelUnavailableNoneLeft: {
		id: 'workbench.panels.unavailable.noneLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.noneLeft'],
		description:
			'Reason a directional Workbench Panel action is disabled: no Panel lies left of the focused one.',
	},
	panelUnavailableNoneRight: {
		id: 'workbench.panels.unavailable.noneRight',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.noneRight'],
		description:
			'Reason a directional Workbench Panel action is disabled: no Panel lies right of the focused one.',
	},
	panelUnavailablePositionEmpty: {
		id: 'workbench.panels.unavailable.positionEmpty',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.positionEmpty'],
		description:
			'Reason focusing a Workbench Panel by position is disabled: the active Tab has fewer Panels.',
	},
	panelUnavailableMissing: {
		id: 'workbench.panels.unavailable.missing',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.missing'],
		description: 'Reason a Workbench Panel action is disabled: the Panel it names is gone.',
	},
	panelUnavailableThreeRequired: {
		id: 'workbench.panels.unavailable.threeRequired',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.threeRequired'],
		description:
			'Reason visual Panel selection is disabled: the active Tab has fewer than three Panels.',
	},
	panelUnavailableSameTarget: {
		id: 'workbench.panels.unavailable.sameTarget',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.sameTarget'],
		description:
			'Reason moving or swapping a Workbench Panel is disabled: the target is the Panel itself.',
	},
	panelUnavailablePrimaryShown: {
		id: 'workbench.panels.unavailable.primaryShown',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.primaryShown'],
		description:
			'Reason Return to primary terminal is disabled: the Panel already shows its primary terminal.',
	},
	panelUnavailableDirectoryUnreported: {
		id: 'workbench.panels.unavailable.directoryUnreported',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.unavailable.directoryUnreported'],
		description:
			'Reason New Tab in the current directory is disabled: the focused terminal has reported no directory.',
	},
	projectUnavailableMissing: {
		id: 'workbench.projects.unavailable.missing',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.unavailable.missing'],
		description: 'Reason a Project action is disabled: the Project it names is gone.',
	},
	projectUnavailableOnlyOne: {
		id: 'workbench.projects.unavailable.onlyOne',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.unavailable.onlyOne'],
		description: 'Reason Reorder Project is disabled: there is a single Project.',
	},
	projectUnavailableCreationPending: {
		id: 'workbench.projects.unavailable.creationPending',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.unavailable.creationPending'],
		description:
			'Reason New Project is disabled: a folder choice, new folder, or path form for one is open.',
	},
	workspaceUnavailableMissing: {
		id: 'workbench.workspaces.unavailable.missing',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.unavailable.missing'],
		description: 'Reason a Workspace action is disabled: the Workspace it names is gone.',
	},
	workspaceUnavailableRequired: {
		id: 'workbench.workspaces.unavailable.required',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.unavailable.required'],
		description:
			'Reason an action is disabled: no Workspace is open. Matches the TUI Workspace-required reason.',
	},
	workspaceUnavailableDestinationMissing: {
		id: 'workbench.workspaces.unavailable.destinationMissing',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.unavailable.destinationMissing'],
		description:
			'Reason reordering a Workspace or Project is disabled: the destination Project or neighbor is gone.',
	},
	workspaceUnavailableFolderUnused: {
		id: 'workbench.workspaces.unavailable.folderUnused',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.unavailable.folderUnused'],
		description:
			'Reason creating a Workspace in a recorded folder is disabled: no Workspace records that folder any more.',
	},
	workspaceUnavailableCreationChanged: {
		id: 'workbench.workspaces.unavailable.creationChanged',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.unavailable.creationChanged'],
		description:
			'Reason opening a just-created Workspace is disabled: its folder, context, or first terminal no longer match the creation.',
	},
	agentUnavailableMissing: {
		id: 'workbench.agents.unavailable.missing',
		defaultMessage: enUSWorkbenchMessages['workbench.agents.unavailable.missing'],
		description: 'Reason navigating to an Agent is disabled: the Agent or its terminal is gone.',
	},
	focusUnavailableSurfaceOpen: {
		id: 'workbench.focus.unavailable.surfaceOpen',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.unavailable.surfaceOpen'],
		description:
			'Reason a Workbench focus action is disabled: a dialog or menu holds the keyboard.',
	},
	focusUnavailableNoRegion: {
		id: 'workbench.focus.unavailable.noRegion',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.unavailable.noRegion'],
		description:
			'Reason a Workbench focus action is disabled: no terminal, Tab strip, or Sidebar is shown.',
	},
	focusUnavailableNoOtherRegion: {
		id: 'workbench.focus.unavailable.noOtherRegion',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.unavailable.noOtherRegion'],
		description:
			'Reason Focus Next or Previous Region is disabled: the focused region is the only one shown.',
	},
	focusUnavailableNoTerminal: {
		id: 'workbench.focus.unavailable.noTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.unavailable.noTerminal'],
		description: 'Reason Focus Active Terminal is disabled: no terminal is displayed.',
	},
	focusUnavailableTabStripHidden: {
		id: 'workbench.focus.unavailable.tabStripHidden',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.unavailable.tabStripHidden'],
		description: 'Reason Focus Tab Strip is disabled: the Tab strip is not shown.',
	},
	focusUnavailableSidebarHidden: {
		id: 'workbench.focus.unavailable.sidebarHidden',
		defaultMessage: enUSWorkbenchMessages['workbench.focus.unavailable.sidebarHidden'],
		description: 'Reason Focus Sidebar is disabled: the Sidebar is hidden.',
	},
	panelSwapVisible: {
		id: 'workbench.panels.actions.swapVisible',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.swapVisible'],
		description: 'Action that visibly labels and swaps the active Workbench panel.',
	},
	panelVisualSelectionFocusLabel: {
		id: 'workbench.panels.visualSelection.focusLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.visualSelection.focusLabel'],
		description: 'Accessible instructions while choosing a Workbench panel to focus.',
	},
	panelVisualSelectionFocusTarget: {
		id: 'workbench.panels.visualSelection.focusTarget',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.visualSelection.focusTarget'],
		description: 'Accessible label for one visibly labeled Workbench panel focus target.',
	},
	panelVisualSelectionStart: {
		id: 'workbench.panels.visualSelection.start',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.visualSelection.start'],
		description: 'Internal action that starts visible Workbench panel selection.',
	},
	panelVisualSelectionSwapLabel: {
		id: 'workbench.panels.visualSelection.swapLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.visualSelection.swapLabel'],
		description: 'Accessible instructions while choosing a Workbench panel to swap.',
	},
	panelVisualSelectionSwapTarget: {
		id: 'workbench.panels.visualSelection.swapTarget',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.visualSelection.swapTarget'],
		description: 'Accessible label for one visibly labeled Workbench panel swap target.',
	},
	panelMore: {
		id: 'workbench.panels.actions.more',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.more'],
		description: 'Accessible label for the Workbench panel options menu.',
	},
	panelReturnToPrimary: {
		id: 'workbench.panels.actions.returnToPrimary',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.returnToPrimary'],
		description: 'Action that returns from a child terminal to the primary terminal.',
	},
	panelSplitAbove: {
		id: 'workbench.panels.actions.splitAbove',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitAbove'],
		description: 'Action that creates a Workbench panel above the current panel.',
	},
	panelSplit: {
		id: 'workbench.panels.actions.split',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.split'],
		description: 'Action that splits a Workbench panel in a requested direction.',
	},
	panelSplitBelow: {
		id: 'workbench.panels.actions.splitBelow',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitBelow'],
		description: 'Action that creates a Workbench panel below the current panel.',
	},
	panelSplitLeft: {
		id: 'workbench.panels.actions.splitLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitLeft'],
		description: 'Action that creates a Workbench panel to the left of the current panel.',
	},
	panelSplitRight: {
		id: 'workbench.panels.actions.splitRight',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitRight'],
		description: 'Action that creates a Workbench panel to the right of the current panel.',
	},
	panelToggleMaximization: {
		id: 'workbench.panels.actions.toggleMaximization',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.toggleMaximization'],
		description: 'Action that toggles maximization for the active Workbench panel.',
	},
	project: {
		id: 'workbench.sidebar.sections.project',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.sections.project'],
		description:
			'Accessible name of the Workspace list in the Workbench sidebar, which shows no heading of its own.',
	},
	projectCollapse: {
		id: 'workbench.projects.actions.collapse',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.collapse'],
		description: 'Action that collapses a Project in the Workbench sidebar.',
	},
	projectCreate: {
		id: 'workbench.projects.actions.create',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.create'],
		description: 'Action that creates a Project from one entered Folder path.',
	},
	projectNew: {
		id: 'workbench.projects.actions.new',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.new'],
		description:
			'Search Action that starts a new Project from a Folder that is selected or created for it.',
	},
	projectCreateDescription: {
		id: 'workbench.projects.create.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.description'],
		description: 'Explanation of how the selected Folder is used when creating a Project.',
	},
	projectCreatePath: {
		id: 'workbench.projects.create.path',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.path'],
		description: 'Label for the Folder path used to create a Project.',
	},
	projectCreatePathDescription: {
		id: 'workbench.projects.create.pathDescription',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.pathDescription'],
		description: 'Browser validation limitation shown below the Project Folder path field.',
	},
	projectCreatePathPlaceholder: {
		id: 'workbench.projects.create.pathPlaceholder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.pathPlaceholder'],
		description: 'Example absolute path shown in the Project Folder path field.',
	},
	projectCreateTitle: {
		id: 'workbench.projects.create.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.title'],
		description: 'Title for selecting the first Folder used to create a Project.',
	},
	projectFolderAcquisitionBusy: {
		id: 'workbench.projects.folderAcquisition.failure.busy',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.busy'],
		description: 'Failure shown when a second native Folder selection overlaps the first.',
	},
	projectFolderAcquisitionControlUnavailable: {
		id: 'workbench.projects.folderAcquisition.failure.controlUnavailable',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.controlUnavailable'],
		description: 'Failure shown when the Desktop Folder-selection control has retired.',
	},
	projectFolderAcquisitionDialogFailed: {
		id: 'workbench.projects.folderAcquisition.failure.dialogFailed',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.dialogFailed'],
		description: 'Failure shown when the operating-system Folder picker cannot be opened.',
	},
	projectFolderAcquisitionDismiss: {
		id: 'workbench.projects.folderAcquisition.failure.dismiss',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.dismiss'],
		description: 'Action that dismisses a native Folder-selection failure.',
	},
	projectFolderAcquisitionNotFound: {
		id: 'workbench.projects.folderAcquisition.failure.notFound',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.notFound'],
		description:
			'Failure shown when New Folder finds that the selected parent Folder no longer exists.',
	},
	projectFolderAcquisitionPermissionDenied: {
		id: 'workbench.projects.folderAcquisition.failure.permissionDenied',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.permissionDenied'],
		description:
			'Failure shown when New Folder may not create a Folder inside the selected parent Folder.',
	},
	projectFolderAcquisitionTitle: {
		id: 'workbench.projects.folderAcquisition.failure.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.title'],
		description: 'Title for a native Project Folder-selection failure.',
	},
	projectFolderAcquisitionValidationFailed: {
		id: 'workbench.projects.folderAcquisition.failure.validationFailed',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.validationFailed'],
		description: 'Failure shown when a selected Folder cannot be validated safely.',
	},
	projectCreateWorkspaceAtDirectory: {
		id: 'workbench.projects.actions.createWorkspaceAtDirectory',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.createWorkspaceAtDirectory'],
		description: 'Creates a Workspace in a selected directory within the addressed Project.',
	},
	projectCreateWorkspace: {
		id: 'workbench.projects.actions.createWorkspace',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.createWorkspace'],
		description: 'Action that creates a Workspace from one Project folder.',
	},
	projectExpand: {
		id: 'workbench.projects.actions.expand',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.expand'],
		description: 'Action that expands a Project in the Workbench sidebar.',
	},
	projectManage: {
		id: 'workbench.projects.actions.manage',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.manage'],
		description: 'Action that opens the Project management interface.',
	},
	projectNewFolder: {
		id: 'workbench.projects.actions.newFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.newFolder'],
		description: 'Action that creates a new Folder for a Project.',
	},
	projectOptions: {
		id: 'workbench.projects.actions.options',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.options'],
		description: 'Accessible label for a Project options menu.',
	},
	projectRemove: {
		id: 'workbench.projects.actions.remove',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.remove'],
		description: 'Action that removes a Project from Orchbee.',
	},
	projectReorder: {
		id: 'workbench.projects.actions.reorder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.reorder'],
		description: 'Action that reorders a Project in the Workbench sidebar.',
	},
	projectSelectFolder: {
		id: 'workbench.projects.actions.selectFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.selectFolder'],
		description: 'Action that selects one Folder for a new Project.',
	},
	projectToggleExpansion: {
		id: 'workbench.projects.actions.toggleExpansion',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.toggleExpansion'],
		description: 'Action that toggles Project disclosure in the Workbench sidebar.',
	},
	projectFolders: {
		id: 'workbench.projects.groups.folders',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.groups.folders'],
		description: 'Menu group for regular Project folders.',
	},
	projectWorktrees: {
		id: 'workbench.projects.groups.worktrees',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.groups.worktrees'],
		description: 'Menu group for Git worktree folders.',
	},
	recents: {
		id: 'workbench.sidebar.sections.recents',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.sections.recents'],
		description:
			'Heading over the Workspaces outside any Project, always the last group in the Workbench sidebar.',
	},
	recentsCollapse: {
		id: 'workbench.recents.actions.collapse',
		defaultMessage: enUSWorkbenchMessages['workbench.recents.actions.collapse'],
		description: 'Action that collapses the Recents group in the Workbench sidebar.',
	},
	recentsExpand: {
		id: 'workbench.recents.actions.expand',
		defaultMessage: enUSWorkbenchMessages['workbench.recents.actions.expand'],
		description: 'Action that expands the Recents group in the Workbench sidebar.',
	},
	sidebarDropEndOfProjects: {
		id: 'workbench.sidebar.drop.endOfProjects',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.drop.endOfProjects'],
		description:
			'Drop area shown after the sidebar list while a Project is dragged; dropping there makes it the last Project.',
	},
	recentsToggleExpansion: {
		id: 'workbench.recents.actions.toggleExpansion',
		defaultMessage: enUSWorkbenchMessages['workbench.recents.actions.toggleExpansion'],
		description: 'Action that toggles disclosure of the Recents group in the Workbench sidebar.',
	},
	resourceCancel: {
		id: 'workbench.resource.actions.cancel',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.cancel'],
		description: 'Action that cancels a Project or Workspace edit, or the New Worktree form.',
	},
	resourceClose: {
		id: 'workbench.resource.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.close'],
		description: 'Accessible label for closing a Project, Workspace or New Worktree dialog.',
	},
	resourceCreate: {
		id: 'workbench.resource.actions.create',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.create'],
		description: 'Action that creates a new Project, Tab group or Worktree.',
	},
	search: {
		id: 'workbench.sidebar.items.search',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.search'],
		description: 'Label for the search entry in the Workbench sidebar.',
	},
	workspaceActivate: {
		id: 'workbench.workspaces.actions.activate',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.activate'],
		description: 'Action that activates a Workspace terminal context.',
	},
	workspaceAgentCollapse: {
		id: 'workbench.workspaces.agents.actions.collapse',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.actions.collapse'],
		description: 'Accessible action that collapses the live Agent list for one Workspace.',
	},
	workspaceAgentCount: {
		id: 'workbench.workspaces.agents.count',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.count'],
		description: 'Number of terminals holding an Agent in one Workspace summary.',
	},
	workspaceAgentExpand: {
		id: 'workbench.workspaces.agents.actions.expand',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.actions.expand'],
		description: 'Accessible action that expands the live Agent list for one Workspace.',
	},
	workspaceAgentLocation: {
		id: 'workbench.workspaces.agents.location',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.location'],
		description: 'Compact Tab, Panel, and terminal location for a Workspace Agent row.',
	},
	workspaceAgentLocationChildTerminal: {
		id: 'workbench.workspaces.agents.location.childTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.location.childTerminal'],
		description: 'Position label for a child terminal that holds an Agent.',
	},
	workspaceAgentLocationPrimaryTerminal: {
		id: 'workbench.workspaces.agents.location.primaryTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.location.primaryTerminal'],
		description: 'Position label for a primary terminal that holds an Agent.',
	},
	workspaceAgentNavigate: {
		id: 'workbench.workspaces.agents.actions.navigate',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.actions.navigate'],
		description: 'Semantic action that opens the exact terminal holding an Agent.',
	},
	workspaceAgentNavigationLabel: {
		id: 'workbench.workspaces.agents.navigationLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.navigationLabel'],
		description: 'Accessible label for a Workspace Agent row that opens its terminal.',
	},
	workspaceAgentNavigationUnavailable: {
		id: 'workbench.workspaces.agents.navigationUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.agents.navigationUnavailable'],
		description:
			'Status notice when an Agent destination, such as a notification target, is gone, rebound to another terminal, or unavailable, so navigation moved nowhere.',
	},
	workspaceDefaultName: {
		id: 'workbench.workspaces.names.default',
		description: 'Default name for an independently created Workspace.',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.names.default'],
	},
	workspaceEmptyDirectory: {
		id: 'workbench.workspaces.empty.directory',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.empty.directory'],
		description: 'Where a new Tab of a Workspace without Tabs starts, shown in place of terminals.',
	},
	workspaceEmptyNoDirectory: {
		id: 'workbench.workspaces.empty.noDirectory',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.empty.noDirectory'],
		description: 'Shown in place of terminals when a Workspace without Tabs has no directory.',
	},
	workspaceEmptyStartAgent: {
		id: 'workbench.workspaces.empty.startAgent',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.empty.startAgent'],
		description:
			'Hint beside its shortcut, in place of terminals when a Workspace has no Tabs, that opens Start Agent.',
	},
	workspaceGitAhead: {
		id: 'workbench.workspaces.git.ahead',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.git.ahead'],
		description:
			'Accessible words for the up-arrow count on a sidebar Workspace row: local commits its Git branch has not pushed to its upstream.',
	},
	workspaceGitBehind: {
		id: 'workbench.workspaces.git.behind',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.git.behind'],
		description:
			'Accessible words for the down-arrow count on a sidebar Workspace row: upstream commits its Git branch has not pulled.',
	},
	workspaceSetupDirectoryRefusedInaccessible: {
		id: 'workbench.workspaces.setup.directoryRefused.inaccessible',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefused.inaccessible'],
		description:
			'Reason under the folder field of a new Workspace or Project when the Server could not read the folder, as when access is denied; the TUI shows the same words.',
	},
	workspaceSetupDirectoryRefusedMissing: {
		id: 'workbench.workspaces.setup.directoryRefused.missing',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefused.missing'],
		description:
			'Reason under the folder field of a new Workspace or Project when nothing exists at the path; the TUI shows the same words.',
	},
	workspaceSetupDirectoryRefusedNotADirectory: {
		id: 'workbench.workspaces.setup.directoryRefused.notADirectory',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefused.notADirectory'],
		description:
			'Reason under the folder field of a new Workspace or Project when the path names something other than a folder; the TUI shows the same words.',
	},
	workspaceSetupDirectoryRefusedUnavailable: {
		id: 'workbench.workspaces.setup.directoryRefused.unavailable',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefused.unavailable'],
		description:
			'Reason under the folder field of a new Workspace or Project when the Server refused the folder without naming why; the TUI shows the same words.',
	},
	workspaceSetupDirectoryRefusedNoticeInaccessible: {
		id: 'workbench.workspaces.setup.directoryRefusedNotice.inaccessible',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefusedNotice.inaccessible'],
		description:
			'Status notice when the Server could not read a folder picked for a new Workspace or Project. {path} is the folder.',
	},
	workspaceSetupDirectoryRefusedNoticeMissing: {
		id: 'workbench.workspaces.setup.directoryRefusedNotice.missing',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefusedNotice.missing'],
		description:
			'Status notice when nothing exists at a folder picked for a new Workspace or Project. {path} is the folder.',
	},
	workspaceSetupDirectoryRefusedNoticeNotADirectory: {
		id: 'workbench.workspaces.setup.directoryRefusedNotice.notADirectory',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefusedNotice.notADirectory'],
		description:
			'Status notice when a path picked for a new Workspace or Project is not a folder. {path} is the path.',
	},
	workspaceSetupDirectoryRefusedNoticeUnavailable: {
		id: 'workbench.workspaces.setup.directoryRefusedNotice.unavailable',
		defaultMessage:
			enUSWorkbenchMessages['workbench.workspaces.setup.directoryRefusedNotice.unavailable'],
		description:
			'Status notice when the Server refused a folder picked for a new Workspace or Project without naming why. {path} is the folder.',
	},
	workspaceSetupNotApplied: {
		id: 'workbench.workspaces.setup.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.setup.notApplied'],
		description: 'Status notice when the Workbench authority definitely refused a change.',
	},
	workspaceSetupOutcomeUnknown: {
		id: 'workbench.workspaces.setup.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.setup.outcomeUnknown'],
		description: 'Status notice when a change may or may not have taken effect.',
	},
	workspaceSetupProjectDissolved: {
		id: 'workbench.workspaces.setup.projectDissolved',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.setup.projectDissolved'],
		description: 'Status notice after dissolving a Project that held Workspaces.',
	},
	workspaceSetupProjectDissolvedEmpty: {
		id: 'workbench.workspaces.setup.projectDissolvedEmpty',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.setup.projectDissolvedEmpty'],
		description: 'Status notice after dissolving a Project that held no Workspace.',
	},
	workspaceSetupStale: {
		id: 'workbench.workspaces.setup.stale',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.setup.stale'],
		description: 'Status notice when a change was refused because the Workbench changed elsewhere.',
	},
	workspaceCloseChanged: {
		id: 'workbench.workspaces.close.changed',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.changed'],
		description: 'Status notice when a Workspace close stopped because its target changed.',
	},
	workspaceCloseClosed: {
		id: 'workbench.workspaces.close.closed',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.closed'],
		description: 'Status notice after a Workspace closed.',
	},
	workspaceCloseClosing: {
		id: 'workbench.workspaces.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.closing'],
		description: 'Status notice while a Workspace closes.',
	},
	workspaceCloseOutcomeUnknown: {
		id: 'workbench.workspaces.close.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.outcomeUnknown'],
		description:
			'Status notice when a Workspace close may have taken effect, naming the command that reads its status.',
	},
	workspaceCloseRefused: {
		id: 'workbench.workspaces.close.refused',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.refused'],
		description: 'Status notice when a Workspace close was not submitted or was refused.',
	},
	workspaceCloseStopped: {
		id: 'workbench.workspaces.close.stopped',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.stopped'],
		description: 'Status notice when an accepted Workspace close stopped before finishing.',
	},
	workspaceDelete: {
		id: 'workbench.workspaces.actions.delete',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.delete'],
		description: 'Action that deletes a Workspace and its terminal context.',
	},
	workspaceDeleteDescription: {
		id: 'workbench.workspaces.confirmDelete.description',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.confirmDelete.description'],
		description: 'Explanation shown before deleting a Workspace.',
	},
	workspaceDeleteTitle: {
		id: 'workbench.workspaces.confirmDelete.title',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.confirmDelete.title'],
		description: 'Title shown before deleting a Workspace.',
	},
	workspaceManage: {
		id: 'workbench.workspaces.actions.manage',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.manage'],
		description: 'Action that opens the Workspace management interface.',
	},
	workspaceOptions: {
		id: 'workbench.workspaces.actions.options',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.options'],
		description: 'Accessible label for a Workspace options menu.',
	},
	workspaceReorder: {
		id: 'workbench.workspaces.actions.reorder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.reorder'],
		description:
			'Action that moves a Workspace to a place in the sidebar: inside a Project, or among the Workspaces outside any Project.',
	},
	searchActionsNeedContext: {
		id: 'workbench.search.actions.needsContext',
		defaultMessage: enUSWorkbenchMessages['workbench.search.actions.needsContext'],
		description:
			'Explanation for a discovered action that needs a target not available in unified search.',
	},
	searchArgumentOptions: {
		id: 'workbench.search.arguments.options',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.options'],
		description: 'Group heading for choices in a nested unified-search argument picker.',
	},
	searchArgumentPlaceholder: {
		id: 'workbench.search.arguments.placeholder',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.placeholder'],
		description: 'Placeholder for a nested unified-search argument picker.',
	},
	searchArgumentPrompt: {
		id: 'workbench.search.arguments.prompt',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.prompt'],
		description: 'Prompt for a nested unified-search argument picker.',
	},
	searchDialogDescription: {
		id: 'workbench.search.dialog.description',
		defaultMessage: enUSWorkbenchMessages['workbench.search.dialog.description'],
		description: 'Accessible description for the unified search dialog.',
	},
	searchDialogTitle: {
		id: 'workbench.search.dialog.title',
		defaultMessage: enUSWorkbenchMessages['workbench.search.dialog.title'],
		description: 'Accessible title for the unified search dialog.',
	},
	searchEmptyActions: {
		id: 'workbench.search.empty.actions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.actions'],
		description: 'Empty state for the Actions unified-search filter.',
	},
	searchEmptyAgents: {
		id: 'workbench.search.empty.agents',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.agents'],
		description: 'Empty state for the Agents unified-search filter.',
	},
	searchEmptyAll: {
		id: 'workbench.search.empty.all',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.all'],
		description: 'Empty state for the All unified-search filter.',
	},
	searchEmptySettings: {
		id: 'workbench.search.empty.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.settings'],
		description: 'Empty state for the Settings unified-search filter.',
	},
	searchFilterActions: {
		id: 'workbench.search.filters.actions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.actions'],
		description: 'Actions filter in unified search.',
	},
	searchFilterAgents: {
		id: 'workbench.search.filters.agents',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.agents'],
		description: 'Agents filter in unified search.',
	},
	searchFilterAll: {
		id: 'workbench.search.filters.all',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.all'],
		description: 'All-results filter in unified search.',
	},
	searchFiltersLabel: {
		id: 'workbench.search.filters.label',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.label'],
		description: 'Accessible name for the unified-search filter tablist.',
	},
	searchFilterSettings: {
		id: 'workbench.search.filters.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.settings'],
		description: 'Settings filter in unified search.',
	},
	searchFooterBack: {
		id: 'workbench.search.footer.back',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.back'],
		description: 'Footer hint for returning from a nested unified-search picker.',
	},
	searchFooterClose: {
		id: 'workbench.search.footer.close',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.close'],
		description: 'Footer hint for closing unified search.',
	},
	searchFooterRun: {
		id: 'workbench.search.footer.run',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.run'],
		description: 'Footer hint for invoking the highlighted unified-search result.',
	},
	searchFooterSelect: {
		id: 'workbench.search.footer.select',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.select'],
		description: 'Footer hint for moving the unified-search highlight.',
	},
	searchGroupRecommendedActions: {
		id: 'workbench.search.groups.recommendedActions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.groups.recommendedActions'],
		description: 'Unified-search group containing recommended product actions.',
	},
	searchGroupSettings: {
		id: 'workbench.search.groups.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.groups.settings'],
		description: 'Unified-search group containing settings destinations.',
	},
	searchInputActions: {
		id: 'workbench.search.input.actions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.actions'],
		description: 'Input placeholder while the Actions unified-search filter is selected.',
	},
	searchInputAgents: {
		id: 'workbench.search.input.agents',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.agents'],
		description: 'Input placeholder while the Agents unified-search filter is selected.',
	},
	searchInputAll: {
		id: 'workbench.search.input.all',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.all'],
		description: 'Input placeholder while the All unified-search filter is selected.',
	},
	searchInputSettings: {
		id: 'workbench.search.input.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.settings'],
		description: 'Input placeholder while the Settings unified-search filter is selected.',
	},
	searchProviderFailed: {
		id: 'workbench.search.providers.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.search.providers.failed'],
		description: 'Status shown when one unified-search provider fails.',
	},
	searchProviderLoading: {
		id: 'workbench.search.providers.loading',
		defaultMessage: enUSWorkbenchMessages['workbench.search.providers.loading'],
		description: 'Status shown while one unified-search provider is loading.',
	},
	searchProviderRetry: {
		id: 'workbench.search.providers.retry',
		defaultMessage: enUSWorkbenchMessages['workbench.search.providers.retry'],
		description: 'Button label for retrying one failed unified-search provider.',
	},
	searchResultsCount: {
		id: 'workbench.search.results.count',
		defaultMessage: enUSWorkbenchMessages['workbench.search.results.count'],
		description: 'Live announcement for the current unified-search result count.',
	},
	searchResultsLabel: {
		id: 'workbench.search.results.label',
		defaultMessage: enUSWorkbenchMessages['workbench.search.results.label'],
		description: 'Accessible name for the unified-search results list.',
	},
	settings: {
		id: 'workbench.sidebar.items.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.settings'],
		description: 'Label for the settings entry in the Workbench sidebar footer.',
	},
	shortcuts: {
		id: 'workbench.sidebar.items.shortcuts',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.shortcuts'],
		description:
			'Command and sidebar footer button that opens the keyboard shortcut reference in Settings.',
	},
	environmentDevelopment: {
		id: 'workbench.sidebar.environment.development',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.environment.development'],
		description:
			'Sidebar footer marker, a word or two, shown only in a development build of Orchbee.',
	},
	environmentDevelopmentCheckout: {
		id: 'workbench.sidebar.environment.developmentCheckout',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.environment.developmentCheckout'],
		description:
			'Tooltip of the development marker; checkout is the folder name of the source checkout the build came from.',
	},
	environmentTest: {
		id: 'workbench.sidebar.environment.test',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.environment.test'],
		description: 'Sidebar footer marker, a word or two, shown only in a test build of Orchbee.',
	},
	environmentTestCheckout: {
		id: 'workbench.sidebar.environment.testCheckout',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.environment.testCheckout'],
		description:
			'Tooltip of the test marker; checkout is the folder name of the source checkout the build came from.',
	},
	showSidebar: {
		id: 'workbench.sidebar.actions.show',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.show'],
		description: 'Action that shows the Workbench sidebar.',
	},
	toggleSidebar: {
		id: 'workbench.sidebar.actions.toggle',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.toggle'],
		description: 'Action that toggles Workbench sidebar visibility.',
	},
	mobileTabs: {
		id: 'workbench.tabs.mobile.all',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.mobile.all'],
		description:
			'Mobile layout bar button and Tab list title naming all Tabs of the Workspace; the count follows it.',
	},
	mobileTabsLabel: {
		id: 'workbench.tabs.mobile.allLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.mobile.allLabel'],
		description: 'Accessible name of the mobile layout button that shows or hides the Tab list.',
	},
	mobileCurrentTab: {
		id: 'workbench.tabs.mobile.current',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.mobile.current'],
		description:
			'Accessible name of the current Tab centered on the mobile layout bar; choosing it shows the Tab list.',
	},
	sidebarLabel: {
		id: 'workbench.sidebar.label',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.label'],
		description: 'Accessible label for the Workbench navigation sidebar.',
	},
	worktreeDefaultLabel: {
		id: 'workbench.worktrees.defaultLabel',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.defaultLabel'],
		description: 'Label of the Workspace a new Worktree opens in; the first unused ordinal.',
	},
	worktreeNew: {
		id: 'workbench.worktrees.new',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.new'],
		description:
			'Menu and Search command that opens the New Worktree form; the Worktree is created only when the form is submitted.',
	},
	worktreeFormTitle: {
		id: 'workbench.worktrees.form.title',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.title'],
		description:
			'Title of the form that asks for a new Worktree’s branch and base before creating it.',
	},
	worktreeFormBranch: {
		id: 'workbench.worktrees.form.branch',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.branch'],
		description: 'Label of the New Worktree form field naming the Git branch to create.',
	},
	worktreeFormBranchHelp: {
		id: 'workbench.worktrees.form.branchHelp',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.branchHelp'],
		description:
			'Help under the Branch field: a blank branch is generated and the Workspace gets a default name.',
	},
	worktreeFormBranchInvalid: {
		id: 'workbench.worktrees.form.branchInvalid',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.branchInvalid'],
		description: 'Error under the Branch field when its text cannot name a Git branch.',
	},
	worktreeFormBase: {
		id: 'workbench.worktrees.form.base',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.base'],
		description:
			'Label of the New Worktree form field naming the branch, tag or commit the Worktree starts from; blank means HEAD.',
	},
	worktreeFormBaseInvalid: {
		id: 'workbench.worktrees.form.baseInvalid',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.baseInvalid'],
		description: 'Error under the Base field when its text cannot name a branch, tag or commit.',
	},
	worktreeFormTooLong: {
		id: 'workbench.worktrees.form.tooLong',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.form.tooLong'],
		description:
			'Error under a New Worktree form field whose text is longer than 1024 UTF-8 bytes.',
	},
	worktreeOpen: {
		id: 'workbench.worktrees.open',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.open'],
		description: "Workspace submenu listing the repository's other Git checkouts to open.",
	},
	worktreeRemove: {
		id: 'workbench.worktrees.remove',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.remove'],
		description: "Destructive Workspace menu command that removes the Workspace's own checkout.",
	},
	worktreeReading: {
		id: 'workbench.worktrees.reading',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.reading'],
		description: 'Disabled menu row while Git lists the checkouts of a Workspace repository.',
	},
	worktreeNoOtherCheckouts: {
		id: 'workbench.worktrees.noOtherCheckouts',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.noOtherCheckouts'],
		description: 'Disabled menu row when the repository has no other checkout to open.',
	},
	worktreeNotRepository: {
		id: 'workbench.worktrees.notRepository',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.notRepository'],
		description: 'Disabled menu row when the Workspace is not in a Git repository Git trusts.',
	},
	worktreeListingUnavailable: {
		id: 'workbench.worktrees.listingUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.listingUnavailable'],
		description: 'Disabled menu row when the Git checkouts could not be listed.',
	},
	worktreeRemoveTitle: {
		id: 'workbench.worktrees.removeTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.removeTitle'],
		description: 'Title of the confirmation before a Worktree is removed.',
	},
	worktreeRemoveDescription: {
		id: 'workbench.worktrees.removeDescription',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.removeDescription'],
		description:
			'What removing a Worktree closes and deletes: the Workspace label, its terminal count, the checkout path and the branch Git keeps.',
	},
	worktreeRemoveConfirm: {
		id: 'workbench.worktrees.removeConfirm',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.removeConfirm'],
		description: 'Destructive confirmation button that removes the Worktree.',
	},
	worktreeStoppedTitle: {
		id: 'workbench.worktrees.stoppedTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.stoppedTitle'],
		description: 'Title of the dialog offering to resume a Worktree removal that stopped.',
	},
	worktreeResume: {
		id: 'workbench.worktrees.resume',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.resume'],
		description: 'Button that resumes a stopped Worktree removal.',
	},
	worktreeResumeRemoval: {
		id: 'workbench.worktrees.resumeRemoval',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.resumeRemoval'],
		description:
			'Workspace menu command, in place of Remove Worktree, that asks to resume a removal of this Workspace that stopped.',
	},
	worktreeCreating: {
		id: 'workbench.worktrees.creating',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.creating'],
		description: 'Status line while a Worktree is created.',
	},
	worktreeOpening: {
		id: 'workbench.worktrees.opening',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.opening'],
		description: 'Status line while a Worktree is opened.',
	},
	worktreeRemoving: {
		id: 'workbench.worktrees.removing',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.removing'],
		description: 'Status line while a Worktree is removed.',
	},
	worktreeRemoved: {
		id: 'workbench.worktrees.removed',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.removed'],
		description: 'Status line after a Worktree was removed and its Workspace closed.',
	},
	worktreeOutcomeUnknown: {
		id: 'workbench.worktrees.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.outcomeUnknown'],
		description:
			'Status line when a Worktree operation may have taken effect; names its operation ID.',
	},
	worktreeCreateFailed: {
		id: 'workbench.worktrees.createFailed',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.createFailed'],
		description: 'Status line when Git or the Worker stopped a Worktree creation.',
	},
	worktreeOpenFailed: {
		id: 'workbench.worktrees.openFailed',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.openFailed'],
		description: 'Status line when Git or the Worker stopped opening a Worktree.',
	},
	worktreeFailureCheckoutInUse: {
		id: 'workbench.worktrees.failure.checkoutInUse',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.checkoutInUse'],
		description: 'Worktree removal stopped because another Workspace uses the checkout.',
	},
	worktreeFailureCheckoutNotFound: {
		id: 'workbench.worktrees.failure.checkoutNotFound',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.checkoutNotFound'],
		description: 'Worktree operation stopped because Git no longer lists the checkout.',
	},
	worktreeFailureCloseIncomplete: {
		id: 'workbench.worktrees.failure.closeIncomplete',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.closeIncomplete'],
		description: 'Worktree removal stopped because not every terminal closed.',
	},
	worktreeFailureConfigurationUnavailable: {
		id: 'workbench.worktrees.failure.configurationUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.configurationUnavailable'],
		description:
			'Worktree creation stopped because the configured Worktrees directory was unreadable.',
	},
	worktreeFailureDenied: {
		id: 'workbench.worktrees.failure.denied',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.denied'],
		description: 'Worktree operation refused for this Client.',
	},
	worktreeFailureDirectoryInUse: {
		id: 'workbench.worktrees.failure.directoryInUse',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.directoryInUse'],
		description: 'Worktree removal stopped because the checkout directory could not be deleted.',
	},
	worktreeFailureDirty: {
		id: 'workbench.worktrees.failure.dirty',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.dirty'],
		description: 'Worktree removal stopped because the checkout has changes.',
	},
	worktreeFailureGit: {
		id: 'workbench.worktrees.failure.git',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.git'],
		description: 'Worktree removal stopped by another Git failure.',
	},
	worktreeFailureGitFailed: {
		id: 'workbench.worktrees.failure.gitFailed',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.gitFailed'],
		description: 'Worktree removal stopped because the Git command failed.',
	},
	worktreeFailureInternal: {
		id: 'workbench.worktrees.failure.internal',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.internal'],
		description: 'Worktree operation stopped for a reason the person cannot act on directly.',
	},
	worktreeFailureMainCheckout: {
		id: 'workbench.worktrees.failure.mainCheckout',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.mainCheckout'],
		description: "Worktree removal refused for the repository's main checkout.",
	},
	worktreeFailureNotSubmitted: {
		id: 'workbench.worktrees.failure.notSubmitted',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.notSubmitted'],
		description: 'Worktree operation that never reached the Server.',
	},
	worktreeFailureSessionStopping: {
		id: 'workbench.worktrees.failure.sessionStopping',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.sessionStopping'],
		description: 'Worktree operation stopped because the Session is stopping.',
	},
	worktreeFailureSourceChanged: {
		id: 'workbench.worktrees.failure.sourceChanged',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.sourceChanged'],
		description: 'Worktree operation stopped because the checkout or Workspace changed.',
	},
	worktreeFailureUnsupported: {
		id: 'workbench.worktrees.failure.unsupported',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.unsupported'],
		description: 'Worktree operation the Worker does not support.',
	},
	worktreeFailureWorkerUnavailable: {
		id: 'workbench.worktrees.failure.workerUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.failure.workerUnavailable'],
		description: 'Worktree operation stopped because the Worker is unavailable or busy.',
	},
	configuredCommandsCategory: {
		id: 'workbench.configuredCommands.category',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.category'],
		description: 'Action category for the commands the configuration installs.',
	},
	configuredCommandTitle: {
		id: 'workbench.configuredCommands.title',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.title'],
		description: 'A configured command, named by its description or the command text it runs.',
	},
	configuredCommandUnnamed: {
		id: 'workbench.configuredCommands.unnamed',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.unnamed'],
		description: 'A configured command without a description, named by its place in the list.',
	},
	configuredCommandKeyTook: {
		id: 'workbench.configuredCommands.key.took',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.key.took'],
		description:
			'Search row note: this configured command runs on the chord of a default window shortcut, named by its action.',
	},
	configuredCommandKeyYielded: {
		id: 'workbench.configuredCommands.key.yielded',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.key.yielded'],
		description:
			"Search row note: the configured command's chord stays with a shortcut the person set for the named action.",
	},
	configuredCommandKeyTuiOnly: {
		id: 'workbench.configuredCommands.key.tuiOnly',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.key.tuiOnly'],
		description:
			'Search row note: the configured key, as the TUI labels it, such as a prefix key, cannot be bound in the window.',
	},
	configuredCommandAccepted: {
		id: 'workbench.configuredCommands.accepted',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.accepted'],
		description: 'Status notice when a configured command started.',
	},
	configuredCommandFailed: {
		id: 'workbench.configuredCommands.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.failed'],
		description: 'Status notice when a configured command, or the focus on its new Panel, failed.',
	},
	configuredCommandUnknown: {
		id: 'workbench.configuredCommands.unknown',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.unknown'],
		description: 'Status notice when a configured command may or may not have run.',
	},
	configuredCommandBusy: {
		id: 'workbench.configuredCommands.refused.busy',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.busy'],
		description: 'Status notice when a configured command was refused while another runs.',
	},
	configuredCommandConflict: {
		id: 'workbench.configuredCommands.refused.conflict',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.conflict'],
		description: 'Status notice when the configuration changed while a command was starting.',
	},
	configuredCommandDenied: {
		id: 'workbench.configuredCommands.refused.denied',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.denied'],
		description: 'Status notice when the Session does not permit a configured command.',
	},
	configuredCommandInvalidIntent: {
		id: 'workbench.configuredCommands.refused.invalidIntent',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.invalidIntent'],
		description: 'Status notice when the Server refused a configured command as invalid.',
	},
	configuredCommandNotFound: {
		id: 'workbench.configuredCommands.refused.notFound',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.notFound'],
		description: 'Status notice when the configuration no longer holds the chosen command.',
	},
	configuredCommandSessionMismatch: {
		id: 'workbench.configuredCommands.refused.sessionMismatch',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.sessionMismatch'],
		description: 'Status notice when the Session changed under a configured command.',
	},
	configuredCommandStaleSource: {
		id: 'workbench.configuredCommands.refused.staleSource',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.staleSource'],
		description: 'Status notice when the Panel a command starts from changed before it ran.',
	},
	configuredCommandUnavailable: {
		id: 'workbench.configuredCommands.refused.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.unavailable'],
		description: 'Status notice when the Server could not run a configured command.',
	},
	agentBoardTitle: {
		id: 'workbench.agentBoard.title',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.title'],
		description:
			'Title of the board that lists the Server’s Agent sessions, and the Action that opens it.',
	},
	agentBoardList: {
		id: 'workbench.agentBoard.list',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.list'],
		description: 'Accessible name of the Agent board’s list of sessions.',
	},
	agentBoardOpen: {
		id: 'workbench.agentBoard.open',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.open'],
		description: 'Agent board button that shows the selected session’s terminal.',
	},
	agentBoardRefresh: {
		id: 'workbench.agentBoard.refresh',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.refresh'],
		description: 'Agent board button that reads the sessions again.',
	},
	agentBoardSelect: {
		id: 'workbench.agentBoard.select',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.select'],
		description: 'Agent board key hint for moving the selection with the arrow keys.',
	},
	agentBoardJumping: {
		id: 'workbench.agentBoard.jumping',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jumping'],
		description: 'Agent board status while it checks a session before showing its terminal.',
	},
	agentBoardReadUnread: {
		id: 'workbench.agentBoard.read.unread',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.read.unread'],
		description: 'Agent board status before any read finished; it never claims there are none.',
	},
	agentBoardReadReading: {
		id: 'workbench.agentBoard.read.reading',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.read.reading'],
		description: 'Agent board status while it reads the sessions.',
	},
	agentBoardReadIncomplete: {
		id: 'workbench.agentBoard.read.incomplete',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.read.incomplete'],
		description: 'Agent board status when the last read stopped before the end of the list.',
	},
	agentBoardReadDenied: {
		id: 'workbench.agentBoard.read.denied',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.read.denied'],
		description: 'Agent board status when the Server refused to list the sessions.',
	},
	agentBoardReadUnavailable: {
		id: 'workbench.agentBoard.read.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.read.unavailable'],
		description: 'Agent board status when the Server’s Agent runtime could not answer.',
	},
	agentBoardReadTransport: {
		id: 'workbench.agentBoard.read.transport',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.read.transport'],
		description: 'Agent board status when no usable answer to a read came back.',
	},
	agentBoardEmpty: {
		id: 'workbench.agentBoard.empty',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.empty'],
		description: 'Agent board body when a whole read found no sessions.',
	},
	agentBoardEmptyPartial: {
		id: 'workbench.agentBoard.emptyPartial',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.emptyPartial'],
		description: 'Agent board body when a cut-short read found no sessions; there may be some.',
	},
	agentBoardStale: {
		id: 'workbench.agentBoard.freshness.stale',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.freshness.stale'],
		description: 'Agent board row mark: the session’s last observation is old.',
	},
	agentBoardDisconnected: {
		id: 'workbench.agentBoard.freshness.disconnected',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.freshness.disconnected'],
		description: 'Agent board row mark: the session’s observer lost its connection.',
	},
	agentBoardNoPanel: {
		id: 'workbench.agentBoard.noPanel',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.noPanel'],
		description: 'Agent board row location when no terminal of this window holds the session.',
	},
	agentBoardJumpGone: {
		id: 'workbench.agentBoard.jump.gone',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jump.gone'],
		description: 'Agent board notice when the session or every terminal it was in has gone.',
	},
	agentBoardJumpUnavailable: {
		id: 'workbench.agentBoard.jump.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jump.unavailable'],
		description: 'Agent board notice when the session could not be read again before moving.',
	},
	agentBoardJumpDenied: {
		id: 'workbench.agentBoard.jump.denied',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jump.denied'],
		description: 'Agent board notice when the Server refused to read the session again.',
	},
	agentBoardJumpEnded: {
		id: 'workbench.agentBoard.jump.ended',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jump.ended'],
		description: 'Agent board notice when the session ended before its terminal could be shown.',
	},
	agentBoardJumpNoPlacement: {
		id: 'workbench.agentBoard.jump.noPlacement',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jump.noPlacement'],
		description: 'Agent board notice when the session is in no terminal that can be shown.',
	},
	agentBoardJumpExpired: {
		id: 'workbench.agentBoard.jump.expired',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.jump.expired'],
		description: 'Agent board notice when showing the session took longer than allowed.',
	},
	agentBoardStartRow: {
		id: 'workbench.agentBoard.startRow',
		defaultMessage: enUSWorkbenchMessages['workbench.agentBoard.startRow'],
		description: 'The Agent board’s first row, above its sessions, which opens Start Agent.',
	},
	heldRestoreDirectoryUnavailable: {
		id: 'workbench.heldRestore.directoryUnavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.heldRestore.directoryUnavailable'],
		description:
			'Status notice when Retry on a Panel a restore held found its saved directory still unavailable.',
	},
	heldRestoreAgentNotResumed: {
		id: 'workbench.heldRestore.agentNotResumed',
		defaultMessage: enUSWorkbenchMessages['workbench.heldRestore.agentNotResumed'],
		description:
			'Status notice when a held Panel started in the home folder, so its Agent conversation was not resumed.',
	},
	agentResumePromptUnconfirmed: {
		id: 'workbench.agentResume.promptUnconfirmed',
		defaultMessage: enUSWorkbenchMessages['workbench.agentResume.promptUnconfirmed'],
		description:
			'Status notice when the Server gave up resuming an Agent conversation after a restart because the Host never confirmed the shell at its prompt. {agent} is the Agent product name.',
	},
	agentResumeTerminalTooOld: {
		id: 'workbench.agentResume.terminalTooOld',
		defaultMessage: enUSWorkbenchMessages['workbench.agentResume.terminalTooOld'],
		description:
			'Status notice when the Server gave up resuming an Agent conversation after a restart because the terminal was started by an Orchbee that cannot confirm a prompt. {agent} is the Agent product name.',
	},
	agentStartNotFound: {
		id: 'workbench.agentStart.notFound',
		defaultMessage: enUSWorkbenchMessages['workbench.agentStart.notFound'],
		description:
			'Status notice when the shell of a new terminal did not find the Agent it was to start. {product} is the Agent product name; {key} is the shortcut that opens Start Agent.',
	},
	agentStartNotFoundUnbound: {
		id: 'workbench.agentStart.notFoundUnbound',
		defaultMessage: enUSWorkbenchMessages['workbench.agentStart.notFoundUnbound'],
		description:
			'Status notice when the shell of a new terminal did not find the Agent it was to start, while Start Agent has no shortcut. {product} is the Agent product name.',
	},
	agentStartExited: {
		id: 'workbench.agentStart.exited',
		defaultMessage: enUSWorkbenchMessages['workbench.agentStart.exited'],
		description:
			'Status notice when an Agent started in a new terminal quit soon after it started. {product} is the Agent product name.',
	},
	agentStartUnsupportedShell: {
		id: 'workbench.agentStart.unsupportedShell',
		defaultMessage: enUSWorkbenchMessages['workbench.agentStart.unsupportedShell'],
		description:
			'Status notice when the shell of a new terminal takes no start command, so the Agent was not started and nothing was typed. {product} is the Agent product name.',
	},
	agentStartNotAtPrompt: {
		id: 'workbench.agentStart.notAtPrompt',
		defaultMessage: enUSWorkbenchMessages['workbench.agentStart.notAtPrompt'],
		description:
			'Status notice when an Agent start typed nothing because the shell of the new terminal never confirmed an empty prompt.',
	},
	agentStartTerminalTooOld: {
		id: 'workbench.agentStart.terminalTooOld',
		defaultMessage: enUSWorkbenchMessages['workbench.agentStart.terminalTooOld'],
		description:
			'Status notice when an Agent start typed nothing because the new terminal was started by an Orchbee that cannot confirm a prompt.',
	},
	heldRestoreNotStarted: {
		id: 'workbench.heldRestore.notStarted',
		defaultMessage: enUSWorkbenchMessages['workbench.heldRestore.notStarted'],
		description:
			'Status notice when starting a held Panel did not start its terminal, for example because the Panel changed.',
	},
	earlierLayouts: {
		id: 'workbench.earlierLayouts.action',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.action'],
		description: 'Action that lists the earlier layouts the Server kept and reopens one.',
	},
	earlierLayoutsReading: {
		id: 'workbench.earlierLayouts.reading',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.reading'],
		description: 'Disabled picker row while the earlier layouts are being read.',
	},
	earlierLayoutsEmpty: {
		id: 'workbench.earlierLayouts.empty',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.empty'],
		description: 'Disabled picker row when the Server has kept no earlier layout yet.',
	},
	earlierLayoutJustNow: {
		id: 'workbench.earlierLayouts.when.justNow',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.when.justNow'],
		description: 'When an earlier layout was kept, less than a minute ago.',
	},
	earlierLayoutMinutesAgo: {
		id: 'workbench.earlierLayouts.when.minutes',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.when.minutes'],
		description: 'When an earlier layout was kept, in minutes.',
	},
	earlierLayoutHoursAgo: {
		id: 'workbench.earlierLayouts.when.hours',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.when.hours'],
		description: 'When an earlier layout was kept, in hours.',
	},
	earlierLayoutDaysAgo: {
		id: 'workbench.earlierLayouts.when.days',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.when.days'],
		description: 'When an earlier layout was kept, in days (two or more).',
	},
	earlierLayoutWhenClock: {
		id: 'workbench.earlierLayouts.when.clock',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.when.clock'],
		description:
			'Relative time an earlier layout was kept, then its local clock time or date and time.',
	},
	earlierLayoutAllOpen: {
		id: 'workbench.earlierLayouts.summary.allOpen',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.summary.allOpen'],
		description: 'Earlier layout summary when everything it holds is open now.',
	},
	earlierLayoutTabsNotOpen: {
		id: 'workbench.earlierLayouts.summary.tabs',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.summary.tabs'],
		description: 'Earlier layout summary: how many of its Tabs are not open now.',
	},
	earlierLayoutWorkspacesTabsNotOpen: {
		id: 'workbench.earlierLayouts.summary.workspacesTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.summary.workspacesTabs'],
		description: 'Earlier layout summary: how many of its Workspaces and Tabs are not open now.',
	},
	earlierLayoutReopenedTabs: {
		id: 'workbench.earlierLayouts.reopened.tabs',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.reopened.tabs'],
		description: 'Status notice after reopening Tabs from an earlier layout.',
	},
	earlierLayoutReopenedWorkspaces: {
		id: 'workbench.earlierLayouts.reopened.workspaces',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.reopened.workspaces'],
		description: 'Status notice after reopening only empty Workspaces from an earlier layout.',
	},
	earlierLayoutReopenedWorkspacesTabs: {
		id: 'workbench.earlierLayouts.reopened.workspacesTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.reopened.workspacesTabs'],
		description: 'Status notice after reopening Workspaces and Tabs from an earlier layout.',
	},
	earlierLayoutNothingToReopen: {
		id: 'workbench.earlierLayouts.nothingToReopen',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.nothingToReopen'],
		description: 'Status notice when everything in the chosen earlier layout is already open.',
	},
	earlierLayoutNotFound: {
		id: 'workbench.earlierLayouts.notFound',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.notFound'],
		description: 'Status notice when the chosen earlier layout was removed meanwhile.',
	},
	earlierLayoutResourceLimit: {
		id: 'workbench.earlierLayouts.resourceLimit',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.resourceLimit'],
		description: 'Status notice when reopening would exceed the Workspace or Tab limit.',
	},
	earlierLayoutBusy: {
		id: 'workbench.earlierLayouts.busy',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.busy'],
		description: 'Status notice when a Workspace close in progress blocks the reopen.',
	},
	earlierLayoutDenied: {
		id: 'workbench.earlierLayouts.denied',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.denied'],
		description: 'Status notice when this Client may not open Tabs.',
	},
	earlierLayoutUnknown: {
		id: 'workbench.earlierLayouts.unknown',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.unknown'],
		description: 'Status notice when the reopen outcome never arrived; it is not repeated.',
	},
	earlierLayoutsUnavailable: {
		id: 'workbench.earlierLayouts.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.earlierLayouts.unavailable'],
		description: 'Status notice or picker row when the earlier layouts cannot be read or reopened.',
	},
});
