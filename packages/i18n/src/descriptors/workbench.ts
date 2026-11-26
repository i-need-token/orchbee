import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	launchRequestTooLarge: {
		description:
			'A deterministic launch frame overflow; edit arguments or environment before retrying.',
		id: 'workbench.launch.requestTooLarge',
		defaultMessage: enUSWorkbenchMessages['workbench.launch.requestTooLarge'],
	},
	terminalLimitReached: {
		description:
			'The Session refused a new terminal because it already holds its bound of terminals; closing some frees room.',
		id: 'workbench.launch.terminalLimitReached',
		defaultMessage: enUSWorkbenchMessages['workbench.launch.terminalLimitReached'],
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
	projectCloseClosing: {
		description: 'Status notice while the Workspaces of a Project close.',
		id: 'workbench.projects.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.close.closing'],
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
	panelsNotApplied: {
		id: 'workbench.panels.update.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.update.notApplied'],
		description:
			'Status notice when the Workbench authority refused creating, moving, or swapping a Panel.',
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
	tabCloseCancelled: {
		id: 'workbench.tabs.close.cancelled',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.cancelled'],
		description:
			'Status notice when an open close confirmation was withdrawn because its target changed.',
	},
	tabCloseClosing: {
		id: 'workbench.tabs.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.closing'],
		description:
			'Status notice while the close of one Tab is still running a second after it began.',
	},
	tabCloseFailed: {
		id: 'workbench.tabs.close.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.failed'],
		description:
			'Status notice when a close of a Tab, Panel, Workspace or Project ended without removing it; it names what was closing and gives no reason.',
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
	tabDuplicationNotCreated: {
		id: 'workbench.tabs.duplicate.notCreated',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.duplicate.notCreated'],
		description:
			'Status notice, worded as the terminal interface words it, when duplicating Tabs was refused, failed, or stopped before every copy was created.',
	},
	tabsNotApplied: {
		id: 'workbench.tabs.update.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.update.notApplied'],
		description: 'Status notice when the Workbench authority refused a change to Tabs.',
	},
	tabCloseNotApplied: {
		id: 'workbench.tabs.close.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.notApplied'],
		description: 'Status notice when closing a set of Tabs was not applied.',
	},
	tabCloseSetProgress: {
		id: 'workbench.tabs.close.setProgress',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.setProgress'],
		description: 'Status notice while a set of Tabs closes.',
	},
	tabCloseSetStopped: {
		id: 'workbench.tabs.close.setStopped',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.setStopped'],
		description: 'Status notice when closing a set of Tabs stopped before its end.',
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
		description:
			'Lower Action that grows or shrinks one Workbench panel by one step; the divider Actions run it.',
	},
	panelResizeDown: {
		id: 'workbench.panels.actions.resizeDown',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.resizeDown'],
		description:
			'Action that moves the divider beside the active Workbench panel down by one 5% step, as the TUI does.',
	},
	panelResizeLeft: {
		id: 'workbench.panels.actions.resizeLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.resizeLeft'],
		description:
			'Action that moves the divider beside the active Workbench panel left by one 5% step, as the TUI does.',
	},
	panelResizeRight: {
		id: 'workbench.panels.actions.resizeRight',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.resizeRight'],
		description:
			'Action that moves the divider beside the active Workbench panel right by one 5% step, as the TUI does.',
	},
	panelResizeUp: {
		id: 'workbench.panels.actions.resizeUp',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.resizeUp'],
		description:
			'Action that moves the divider beside the active Workbench panel up by one 5% step, as the TUI does.',
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
	workspaceCloseClosing: {
		id: 'workbench.workspaces.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.close.closing'],
		description: 'Status notice while a Workspace closes.',
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
	update: {
		id: 'workbench.sidebar.items.update',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.update'],
		description:
			'Sidebar footer button and Action, shown only while a newer Orchbee version is installed, that reopens the window on that version.',
	},
	updateTooltip: {
		id: 'workbench.sidebar.items.updateTooltip',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.updateTooltip'],
		description:
			'Tooltip of the Update button; version is the installed Orchbee version the window switches to. Terminals keep running through the switch.',
	},
	updateUnavailable: {
		id: 'workbench.update.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.update.unavailable'],
		description:
			'Notice after Update when switching could not start or was refused; advises quitting and reopening Orchbee or running orchbee update.',
	},
	actionUnavailable: {
		id: 'workbench.actions.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.unavailable'],
		description:
			'Notice after an Action the person invoked from a control, menu, or shortcut could not run because it is not available right now, such as a disabled or unregistered Action.',
	},
	tabsRenameNotApplied: {
		id: 'workbench.tabs.rename.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.rename.notApplied'],
		description: 'Notice after renaming a Tab was refused or failed, worded as the TUI names it.',
	},
	tabsMoveNotApplied: {
		id: 'workbench.tabs.move.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.move.notApplied'],
		description: 'Notice after moving a Tab was refused or failed, worded as the TUI names it.',
	},
	tabsSwitchNotApplied: {
		id: 'workbench.tabs.switch.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.switch.notApplied'],
		description: 'Notice after switching Tabs was refused or failed, worded as the TUI names it.',
	},
	panelsMoveNotApplied: {
		id: 'workbench.panels.move.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.move.notApplied'],
		description: 'Notice after moving a Panel was refused or failed, worded as the TUI names it.',
	},
	panelsSplitNotApplied: {
		id: 'workbench.panels.split.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.split.notApplied'],
		description:
			'Notice after splitting a Panel was refused or failed, worded as the TUI names it.',
	},
	projectRenameNotApplied: {
		id: 'workbench.projects.rename.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.rename.notApplied'],
		description:
			'Notice after renaming a Project was refused or failed, worded as the TUI names it.',
	},
	workspaceRenameNotApplied: {
		id: 'workbench.workspaces.rename.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.rename.notApplied'],
		description:
			'Notice after renaming a Workspace was refused or failed, worded as the TUI names it.',
	},
	workspaceCreateNotApplied: {
		id: 'workbench.workspaces.create.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.create.notApplied'],
		description:
			'Notice after creating a Workspace was refused or failed, worded as the TUI names it.',
	},
	tabCloseSingleNotApplied: {
		id: 'workbench.tab.close.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.tab.close.notApplied'],
		description: 'Notice after closing a Tab was refused or failed, worded as the TUI names it.',
	},
	panelCloseNotApplied: {
		id: 'workbench.panel.close.notApplied',
		defaultMessage: enUSWorkbenchMessages['workbench.panel.close.notApplied'],
		description: 'Notice after closing a Panel was refused or failed, worded as the TUI names it.',
	},
	updateWaiting: {
		id: 'workbench.update.waiting',
		defaultMessage: enUSWorkbenchMessages['workbench.update.waiting'],
		description:
			'Status notice after Update when the new version cannot join the running terminals yet; it takes effect once no terminal runs.',
	},
	updateHostReplacementTitle: {
		id: 'workbench.update.hostReplacement.title',
		defaultMessage: enUSWorkbenchMessages['workbench.update.hostReplacement.title'],
		description:
			'Title of the confirmation after Update when the new version cannot keep the terminals running and must restart them; worded as the TUI asks it.',
	},
	updateHostReplacementDescription: {
		id: 'workbench.update.hostReplacement.description',
		defaultMessage: enUSWorkbenchMessages['workbench.update.hostReplacement.description'],
		description:
			'Body of that confirmation: every terminal restarts in its folder and the programs in them end.',
	},
	updateHostReplacementStaying: {
		id: 'workbench.update.hostReplacement.staying',
		defaultMessage: enUSWorkbenchMessages['workbench.update.hostReplacement.staying'],
		description:
			'Body of that confirmation once the Session runs the new version and {count} terminals could not move to it: restarting the Session restarts every terminal, and Cancel keeps those terminals on the previous version.',
	},
	updateHostReplacementConfirm: {
		id: 'workbench.update.hostReplacement.confirm',
		defaultMessage: enUSWorkbenchMessages['workbench.update.hostReplacement.confirm'],
		description: 'Destructive button of that confirmation, which restarts every terminal.',
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
	worktreeOutcomeUnknown: {
		id: 'workbench.worktrees.outcomeUnknown',
		defaultMessage: enUSWorkbenchMessages['workbench.worktrees.outcomeUnknown'],
		description: 'Status line when a Worktree operation may have taken effect.',
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
		description: 'Worktree operation refused for this window.',
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
	configuredCommandSourceExited: {
		id: 'workbench.configuredCommands.refused.sourceExited',
		defaultMessage: enUSWorkbenchMessages['workbench.configuredCommands.refused.sourceExited'],
		description:
			'Status notice when the Panel a configured command starts from has a terminal that already exited.',
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
	agentConfirmationOffer: {
		id: 'workbench.agentConfirmation.offer',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.offer'],
		description:
			"Status notice offering to answer an Agent's routine confirmations for the person. {agent} is the Agent product name. Three answers follow it.",
	},
	agentConfirmationRun: {
		id: 'workbench.agentConfirmation.run',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.run'],
		description:
			'Answer to the offer and single-choice menu item: Orchbee answers routine confirmations for this one running Agent until it exits.',
	},
	agentConfirmationAgentType: {
		id: 'workbench.agentConfirmation.agentType',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.agentType'],
		description:
			'Answer to the offer and single-choice menu item: Orchbee answers routine confirmations for every Agent of this product, saved as a setting. {agent} is the Agent product name.',
	},
	agentConfirmationNo: {
		id: 'workbench.agentConfirmation.no',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.no'],
		description: 'Answer to the offer: Orchbee never offers again.',
	},
	agentConfirmationMenu: {
		id: 'workbench.agentConfirmation.menu',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.menu'],
		description:
			'Agent row menu entry that opens the three single-choice items for answering routine confirmations.',
	},
	agentConfirmationOff: {
		id: 'workbench.agentConfirmation.off',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.off'],
		description: 'Single-choice menu item: Orchbee answers no confirmation for this Agent.',
	},
	agentConfirmationAllowedCommand: {
		id: 'workbench.agentConfirmation.allowedCommand',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.allowedCommand'],
		description:
			"Timed status notice after Orchbee answered an Agent's confirmation of a command. {command} is the command text.",
	},
	agentConfirmationAllowedCreate: {
		id: 'workbench.agentConfirmation.allowedCreate',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.allowedCreate'],
		description:
			"Timed status notice after Orchbee answered an Agent's confirmation to create a file. {path} is the file path.",
	},
	agentConfirmationAllowedEdit: {
		id: 'workbench.agentConfirmation.allowedEdit',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.allowedEdit'],
		description:
			"Timed status notice after Orchbee answered an Agent's confirmation to edit a file. {path} is the file path.",
	},
	agentConfirmationStopped: {
		id: 'workbench.agentConfirmation.stopped',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.stopped'],
		description:
			"Status notice when Orchbee stopped answering an Agent's confirmations because one confirmation came back after every answer.",
	},
	agentConfirmationNotChanged: {
		id: 'workbench.agentConfirmation.notChanged',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.notChanged'],
		description:
			'Status notice when a choice about answering Agent confirmations did not reach the Server or was refused, so nothing changed.',
	},
	agentConfirmationWaitStatus: {
		id: 'workbench.agentConfirmation.waitStatus',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.waitStatus'],
		description:
			'Agent state tooltip while a confirmation waits for the person: the state name, then why Orchbee did not answer it.',
	},
	agentConfirmationWaitLeavesWorkspace: {
		id: 'workbench.agentConfirmation.wait.leavesWorkspace',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.leavesWorkspace'],
		description:
			'Why a confirmation waits: the file the Agent would create or edit is outside the Workspace folder.',
	},
	agentConfirmationWaitAnswered: {
		id: 'workbench.agentConfirmation.wait.answered',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.answered'],
		description:
			'Why a confirmation waits: Orchbee pressed its accept key, but the Agent still shows the same confirmation, so Orchbee does not press again and the person answers it.',
	},
	agentConfirmationWaitHomeWorkspace: {
		id: 'workbench.agentConfirmation.wait.homeWorkspace',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.homeWorkspace'],
		description:
			"Why a confirmation waits: the Workspace folder is the person's home folder or contains it, so Orchbee changes no file there unattended.",
	},
	agentConfirmationWaitProtectedFile: {
		id: 'workbench.agentConfirmation.wait.protectedFile',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.protectedFile'],
		description:
			'Why a confirmation waits: the file is one Orchbee never changes unattended, such as Git data, .env, Agent configuration or shell start files.',
	},
	agentConfirmationWaitNotUnderstood: {
		id: 'workbench.agentConfirmation.wait.notUnderstood',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.notUnderstood'],
		description:
			"Why a confirmation waits: the file's path has a shape Orchbee does not read, such as a network path.",
	},
	agentConfirmationWaitNotVisible: {
		id: 'workbench.agentConfirmation.wait.notVisible',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.notVisible'],
		description: "Why a confirmation waits: the file's path is not completely on screen.",
	},
	agentConfirmationWaitMarkedDangerous: {
		id: 'workbench.agentConfirmation.wait.markedDangerous',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.markedDangerous'],
		description: 'Why a confirmation waits: the Agent itself marks the request as dangerous.',
	},
	agentConfirmationWaitNotAnswered: {
		id: 'workbench.agentConfirmation.wait.notAnswered',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.notAnswered'],
		description:
			'Why an Agent waits: it shows something that is not a confirmation Orchbee answers.',
	},
	agentConfirmationWaitPerson: {
		id: 'workbench.agentConfirmation.wait.person',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.person'],
		description:
			'Why a confirmation waits: the person typed or clicked in the terminal since the confirmation appeared.',
	},
	agentConfirmationWaitChanging: {
		id: 'workbench.agentConfirmation.wait.changing',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.changing'],
		description:
			'Why a confirmation waits: the terminal screen changed every time Orchbee was about to answer.',
	},
	agentConfirmationWaitRepeating: {
		id: 'workbench.agentConfirmation.wait.repeating',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.repeating'],
		description:
			'Why a confirmation waits: the same confirmation came back after every answer, so Orchbee stopped.',
	},
	agentConfirmationWaitHostTooOld: {
		id: 'workbench.agentConfirmation.wait.hostTooOld',
		defaultMessage: enUSWorkbenchMessages['workbench.agentConfirmation.wait.hostTooOld'],
		description:
			'Why a confirmation waits: the terminal was started by an Orchbee that cannot check the screen before typing.',
	},
	toggleRightBar: {
		id: 'workbench.rightBar.actions.toggle',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.actions.toggle'],
		description:
			"Action that shows or hides the right bar with the Workspace's files and Git changes.",
	},
	rightBarLabel: {
		id: 'workbench.rightBar.label',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.label'],
		description: 'Accessible name of the right bar.',
	},
	rightBarFiles: {
		id: 'workbench.rightBar.files',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.files'],
		description: "Right bar tab that lists the Workspace's files.",
	},
	rightBarChanges: {
		id: 'workbench.rightBar.changes',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changes'],
		description: "Right bar tab that lists the Workspace's Git changes.",
	},
	rightBarRefresh: {
		id: 'workbench.rightBar.refresh',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.refresh'],
		description: 'Right bar button that reads the files or changes again.',
	},
	rightBarClose: {
		id: 'workbench.rightBar.close',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.close'],
		description: 'Right bar button that hides it.',
	},
	rightBarNoWorkspace: {
		id: 'workbench.rightBar.noWorkspace',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.noWorkspace'],
		description: 'Right bar text when no Workspace is shown.',
	},
	rightBarLoading: {
		id: 'workbench.rightBar.loading',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.loading'],
		description: 'Right bar text while a listing, change list or file is read.',
	},
	rightBarFailed: {
		id: 'workbench.rightBar.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.failed'],
		description: 'Right bar text when a read failed.',
	},
	rightBarEmpty: {
		id: 'workbench.rightBar.empty',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.empty'],
		description: 'Right bar text for an empty folder.',
	},
	rightBarTruncated: {
		id: 'workbench.rightBar.truncated',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.truncated'],
		description: 'Right bar note when a listing was cut short. count is the number shown.',
	},
	rightBarNoChanges: {
		id: 'workbench.rightBar.changes.none',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changes.none'],
		description: 'Right bar Changes text when Git reports nothing changed.',
	},
	rightBarNotRepository: {
		id: 'workbench.rightBar.changes.notRepository',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changes.notRepository'],
		description: 'Right bar Changes text outside a Git repository.',
	},
	rightBarConflicts: {
		id: 'workbench.rightBar.changes.conflicts',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changes.conflicts'],
		description: 'Right bar Changes group of conflicted files.',
	},
	rightBarStaged: {
		id: 'workbench.rightBar.changes.staged',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changes.staged'],
		description: 'Right bar Changes group of staged files.',
	},
	rightBarUnstaged: {
		id: 'workbench.rightBar.changes.unstaged',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changes.unstaged'],
		description: 'Right bar Changes group of unstaged and untracked files.',
	},
	rightBarPreviewBinary: {
		id: 'workbench.rightBar.preview.binary',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.preview.binary'],
		description:
			'File preview text for a binary file. {type} is the file extension in capitals, or none; {size} is the formatted file size.',
	},
	rightBarPreviewTooLarge: {
		id: 'workbench.rightBar.preview.tooLarge',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.preview.tooLarge'],
		description:
			'File preview text for a file over the size limit. {size} is the file size and {limit} the limit, both formatted.',
	},
	rightBarImageSize: {
		id: 'workbench.rightBar.imageSize',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.imageSize'],
		description: 'An image preview’s width and height in pixels.',
	},
	rightBarPreviewAbsent: {
		id: 'workbench.rightBar.preview.absent',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.preview.absent'],
		description: 'File preview text for a file that was deleted.',
	},
	rightBarSideHead: {
		id: 'workbench.rightBar.side.head',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.side.head'],
		description: 'One side of a comparison: the file as of the last commit.',
	},
	rightBarSideIndex: {
		id: 'workbench.rightBar.side.index',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.side.index'],
		description: 'One side of a comparison: the file as staged.',
	},
	rightBarSideNone: {
		id: 'workbench.rightBar.side.none',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.side.none'],
		description: 'One side of a comparison when the file did not exist on that side.',
	},
	rightBarSideWorktree: {
		id: 'workbench.rightBar.side.worktree',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.side.worktree'],
		description: 'One side of a comparison: the file as it is on disk.',
	},
	rightBarCompare: {
		id: 'workbench.rightBar.compare',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.compare'],
		description:
			'Which two versions a diff compares. before and after are side names such as "last commit" and "working copy".',
	},
	rightBarLineCount: {
		id: 'workbench.rightBar.lineCount',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.lineCount'],
		description: 'How many lines an open file has.',
	},
	rightBarChangeCount: {
		id: 'workbench.rightBar.changeCount',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.changeCount'],
		description: 'How many separate changed regions a diff has.',
	},
	rightBarFloatLabel: {
		id: 'workbench.rightBar.float.label',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.label'],
		description:
			'Accessible name of the floating window over the terminals. name is the file shown.',
	},
	rightBarFloatTabs: {
		id: 'workbench.rightBar.float.tabs',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.tabs'],
		description: "Accessible name of the floating window's tab strip.",
	},
	rightBarFloatPill: {
		id: 'workbench.rightBar.float.pill',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.pill'],
		description:
			'Button that brings back a collapsed floating window. count is how many files it holds.',
	},
	rightBarFloatReopen: {
		id: 'workbench.rightBar.float.reopen',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.reopen'],
		description: 'Tooltip of the button that brings back a collapsed floating window.',
	},
	rightBarFloatClose: {
		id: 'workbench.rightBar.float.close',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.close'],
		description: 'Button that closes the floating window and every file in it.',
	},
	rightBarFloatCloseTab: {
		id: 'workbench.rightBar.float.closeTab',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.closeTab'],
		description: 'Button that closes one file in the floating window. name is the file name.',
	},
	rightBarFloatPreviewTitle: {
		id: 'workbench.rightBar.float.tabTitle',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.float.tabTitle'],
		description:
			"Tooltip of a preview tab, which the next opened file replaces. path is the file's path in the Workspace.",
	},
	rightBarDiffLayout: {
		id: 'workbench.rightBar.diff.layout',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.layout'],
		description: 'Accessible name of the choice between a side-by-side and an inline diff.',
	},
	rightBarDiffSideBySide: {
		id: 'workbench.rightBar.diff.sideBySide',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.sideBySide'],
		description: 'Shows a diff with the two versions next to each other.',
	},
	rightBarDiffInline: {
		id: 'workbench.rightBar.diff.inline',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.inline'],
		description: 'Shows a diff in one column with removed and added lines interleaved.',
	},
	rightBarDiffIgnoreWhitespace: {
		id: 'workbench.rightBar.diff.ignoreWhitespace',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.ignoreWhitespace'],
		description: 'Toggle that hides changes made only of leading or trailing whitespace.',
	},
	rightBarDiffHideUnchanged: {
		id: 'workbench.rightBar.diff.hideUnchanged',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.hideUnchanged'],
		description: 'Toggle that folds the lines a diff did not change.',
	},
	rightBarDiffPrevious: {
		id: 'workbench.rightBar.diff.previous',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.previous'],
		description: 'Moves to the previous change in a diff.',
	},
	rightBarDiffNext: {
		id: 'workbench.rightBar.diff.next',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.next'],
		description: 'Moves to the next change in a diff.',
	},
	rightBarDiffSuffixStaged: {
		id: 'workbench.rightBar.diff.suffixStaged',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.suffixStaged'],
		description:
			'Shown after a file name on a tab that compares the staged file with the last commit.',
	},
	rightBarDiffSuffixWorktree: {
		id: 'workbench.rightBar.diff.suffixWorktree',
		defaultMessage: enUSWorkbenchMessages['workbench.rightBar.diff.suffixWorktree'],
		description:
			'Shown after a file name on a tab that compares the file on disk with the staged or committed one.',
	},
});
