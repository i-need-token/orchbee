import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
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
	workspaceNew: {
		description: 'Create an ungrouped Workspace with the default shell.',
		id: 'workbench.workspaces.new',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.new'],
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
	actionUnavailable: {
		id: 'workbench.actions.states.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.states.unavailable'],
		description: 'Reason shown when a host does not provide an optional Workbench action.',
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
	terminal: {
		id: 'workbench.sidebar.items.terminal',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.terminal'],
		description: 'Label for the standalone terminal entry in the Workbench sidebar.',
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
	panelBroadcastInputStart: {
		id: 'workbench.panels.broadcastInput.actions.start',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.actions.start'],
		description: 'Action that opens live input broadcasting for every displayed Panel terminal.',
	},
	panelBroadcastInputDismiss: {
		id: 'workbench.panels.broadcastInput.actions.dismiss',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.actions.dismiss'],
		description: 'Action that dismisses a stopped Panel broadcast input notice.',
	},
	panelBroadcastInputEnd: {
		id: 'workbench.panels.broadcastInput.actions.end',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.actions.end'],
		description: 'Action that ends live Panel input broadcasting.',
	},
	panelBroadcastInputHide: {
		id: 'workbench.panels.broadcastInput.actions.hide',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.actions.hide'],
		description: 'Action that masks the live Panel broadcast input surface.',
	},
	panelBroadcastInputShow: {
		id: 'workbench.panels.broadcastInput.actions.show',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.actions.show'],
		description: 'Action that reveals the live Panel broadcast input surface.',
	},
	panelBroadcastInputLabel: {
		id: 'workbench.panels.broadcastInput.label',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.label'],
		description: 'Accessible label for the live Panel broadcast input surface.',
	},
	panelBroadcastInputPlaceholder: {
		id: 'workbench.panels.broadcastInput.placeholder',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.placeholder'],
		description: 'Placeholder for the live Panel broadcast input field.',
	},
	panelBroadcastInputTargetCount: {
		id: 'workbench.panels.broadcastInput.targetCount',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.targetCount'],
		description: 'Status describing how many Panel terminals receive broadcast input.',
	},
	panelBroadcastInputTargetInvalidated: {
		id: 'workbench.panels.broadcastInput.failure.targetInvalidated',
		defaultMessage:
			enUSWorkbenchMessages['workbench.panels.broadcastInput.failure.targetInvalidated'],
		description: 'Warning shown when the captured Panel broadcast target set becomes stale.',
	},
	panelBroadcastInputPartialDelivery: {
		id: 'workbench.panels.broadcastInput.failure.partialDelivery',
		defaultMessage:
			enUSWorkbenchMessages['workbench.panels.broadcastInput.failure.partialDelivery'],
		description: 'Warning shown after non-atomic Panel broadcast input reaches only some targets.',
	},
	panelBroadcastInputDeliveryFailed: {
		id: 'workbench.panels.broadcastInput.failure.deliveryFailed',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.broadcastInput.failure.deliveryFailed'],
		description: 'Warning shown when a Panel broadcast input could not reach any target.',
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
		description: 'Action that closes the displayed terminal or its Workbench panel.',
	},
	panelCloseAnyway: {
		id: 'workbench.panels.actions.closeAnyway',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.closeAnyway'],
		description: 'Action that confirms closing running terminals.',
	},
	panelCloseRunningDescription: {
		id: 'workbench.panels.closeRunning.description',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.closeRunning.description'],
		description: 'Warning shown before closing one or more running terminals.',
	},
	panelCloseRunningTitle: {
		id: 'workbench.panels.closeRunning.title',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.closeRunning.title'],
		description: 'Title for the running-terminal close confirmation.',
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
	tabCloseClosed: {
		id: 'workbench.tabs.close.closed',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.closed'],
		description: 'Status notice after one Tab closed.',
	},
	tabCloseClosing: {
		id: 'workbench.tabs.close.closing',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.closing'],
		description: 'Status notice while one Tab closes.',
	},
	tabCloseDescription: {
		id: 'workbench.tabs.close.description',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.close.description'],
		description: 'Confirmation text naming the Tabs that close and how many terminals end.',
	},
	tabCreateNotCreated: {
		id: 'workbench.tabs.create.notCreated',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.create.notCreated'],
		description: 'Status notice when the Workbench authority did not create a Tab.',
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
	tabCloseRunningDescription: {
		id: 'workbench.tabs.closeRunning.description',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.closeRunning.description'],
		description: 'Warning shown before closing tabs that contain running terminals.',
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
	panelMinimize: {
		id: 'workbench.panels.actions.minimize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.minimize'],
		description: 'Action that restores a maximized Workbench panel to the split layout.',
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
	panelNewChildTerminal: {
		id: 'workbench.panels.actions.newChildTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.newChildTerminal'],
		description: 'Action that creates a child terminal in the current Workbench panel.',
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
	projectConnect: {
		id: 'workbench.projects.actions.connect',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.connect'],
		description: 'Action that connects an Environment-backed Project.',
	},
	projectConnectViaSsh: {
		id: 'workbench.projects.actions.connectViaSsh',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.connectViaSsh'],
		description: 'Action that invokes an optional platform SSH Project enrollment capability.',
	},
	projectDisconnect: {
		id: 'workbench.projects.actions.disconnect',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.disconnect'],
		description: 'Action that disconnects an Environment-backed Project.',
	},
	projectRetryConnection: {
		id: 'workbench.projects.actions.retryConnection',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.retryConnection'],
		description: 'Action that retries an Environment-backed Project connection.',
	},
	projectConnectionBlocked: {
		id: 'workbench.projects.connection.blocked',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.blocked'],
		description: 'Status for a Project connection that needs user action.',
	},
	projectConnectionConnected: {
		id: 'workbench.projects.connection.connected',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.connected'],
		description: 'Status for a connected Environment-backed Project.',
	},
	projectConnectionConnecting: {
		id: 'workbench.projects.connection.connecting',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.connecting'],
		description: 'Status for an Environment-backed Project being connected.',
	},
	projectConnectionDisconnected: {
		id: 'workbench.projects.connection.disconnected',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.disconnected'],
		description: 'Status for a disconnected Environment-backed Project.',
	},
	projectConnectionInteractionRequired: {
		id: 'workbench.projects.connection.interactionRequired',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.interactionRequired'],
		description: 'Status for a Project connection waiting for authentication.',
	},
	projectConnectionRetrying: {
		id: 'workbench.projects.connection.retrying',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.retrying'],
		description: 'Status for an Environment-backed Project reconnecting automatically.',
	},
	projectConnectionUnavailable: {
		id: 'workbench.projects.connection.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.connection.unavailable'],
		description: 'Status for an unavailable Environment-backed Project connection.',
	},
	projectAddFolder: {
		id: 'workbench.projects.actions.addFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.addFolder'],
		description: 'Action that adds an existing Folder to a Project.',
	},
	projectCreate: {
		id: 'workbench.projects.actions.create',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.create'],
		description: 'Action that creates a Project from one entered Folder path.',
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
	projectFolderAcquisitionAlreadyAdded: {
		id: 'workbench.projects.folderAcquisition.failure.alreadyAdded',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.alreadyAdded'],
		description: 'Failure shown when a selected Folder already belongs to the target Project.',
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
	projectFolderAcquisitionNotDirectory: {
		id: 'workbench.projects.folderAcquisition.failure.notDirectory',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.notDirectory'],
		description: 'Failure shown when the native selection is not a directory.',
	},
	projectFolderAcquisitionNotFound: {
		id: 'workbench.projects.folderAcquisition.failure.notFound',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.notFound'],
		description: 'Failure shown when the selected Folder disappears before validation.',
	},
	projectFolderAcquisitionPermissionDenied: {
		id: 'workbench.projects.folderAcquisition.failure.permissionDenied',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.permissionDenied'],
		description: 'Failure shown when the selected Folder cannot be read and traversed.',
	},
	projectFolderAcquisitionProjectUnavailable: {
		id: 'workbench.projects.folderAcquisition.failure.projectUnavailable',
		defaultMessage:
			enUSWorkbenchMessages['workbench.projects.folderAcquisition.failure.projectUnavailable'],
		description: 'Failure shown when the target Project disappears during native selection.',
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
	projectRemoveFolder: {
		id: 'workbench.projects.actions.removeFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.removeFolder'],
		description: 'Action that removes a manually added Folder from a Project.',
	},
	projectReorder: {
		id: 'workbench.projects.actions.reorder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.reorder'],
		description: 'Action that reorders a Project in the Workbench sidebar.',
	},
	projectSetPrimaryFolder: {
		id: 'workbench.projects.actions.setPrimaryFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.setPrimaryFolder'],
		description: 'Action that sets the Project primary Folder.',
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
	projectManageDescription: {
		id: 'workbench.projects.manage.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.description'],
		description: 'Accessible description for the Project management interface.',
	},
	projectManageAddLocalFolder: {
		id: 'workbench.projects.manage.addLocalFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.addLocalFolder'],
		description: 'Action that adds a local Folder to an existing Project.',
	},
	projectManageAddSshFolder: {
		id: 'workbench.projects.manage.addSshFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.addSshFolder'],
		description: 'Action that connects over SSH and adds a remote Folder to an existing Project.',
	},
	projectManageFolders: {
		id: 'workbench.projects.manage.folders',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.folders'],
		description: 'Heading for Project folders in the Project management interface.',
	},
	projectManageName: {
		id: 'workbench.projects.manage.name',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.name'],
		description: 'Label for the Project name field.',
	},
	projectManageNamePlaceholder: {
		id: 'workbench.projects.manage.namePlaceholder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.namePlaceholder'],
		description: 'Placeholder for the Project name field.',
	},
	projectManageFolderAlreadyAdded: {
		id: 'workbench.projects.manage.folderAlreadyAdded',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.folderAlreadyAdded'],
		description: 'Explanation shown when a Folder path already belongs to the Project.',
	},
	projectManagePrimaryFolder: {
		id: 'workbench.projects.manage.primaryFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.primaryFolder'],
		description: 'Label identifying a Project primary Folder.',
	},
	projectManageSshFolderBadge: {
		id: 'workbench.projects.manage.sshFolderBadge',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.sshFolderBadge'],
		description: 'Short visible marker for an SSH remote Folder.',
	},
	projectManageSshFolderDescription: {
		id: 'workbench.projects.manage.sshFolderDescription',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.sshFolderDescription'],
		description: 'Accessible description for an SSH remote Folder marker.',
	},
	projectManageTitle: {
		id: 'workbench.projects.manage.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.title'],
		description: 'Title for the Project management interface.',
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
	recentsToggleExpansion: {
		id: 'workbench.recents.actions.toggleExpansion',
		defaultMessage: enUSWorkbenchMessages['workbench.recents.actions.toggleExpansion'],
		description: 'Action that toggles disclosure of the Recents group in the Workbench sidebar.',
	},
	resourceCancel: {
		id: 'workbench.resource.actions.cancel',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.cancel'],
		description: 'Action that cancels a Project or Workspace edit.',
	},
	resourceAdd: {
		id: 'workbench.resource.actions.add',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.add'],
		description: 'Action that adds an entered Folder to a Project.',
	},
	resourceClose: {
		id: 'workbench.resource.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.close'],
		description: 'Accessible label for closing a Project or Workspace dialog.',
	},
	resourceCreate: {
		id: 'workbench.resource.actions.create',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.create'],
		description: 'Action that creates a new Project.',
	},
	resourceSave: {
		id: 'workbench.resource.actions.save',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.save'],
		description: 'Action that saves a Project or Workspace edit.',
	},
	search: {
		id: 'workbench.sidebar.items.search',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.search'],
		description: 'Label for the search entry in the Workbench sidebar.',
	},
	workspaceCopySuffix: {
		id: 'workbench.workspaces.names.copySuffix',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.names.copySuffix'],
		description: 'Suffix appended to the default name of a duplicated Workspace.',
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
		description: 'Announcement when an Agent leaves its terminal before navigation can commit.',
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
	workspacePrimaryName: {
		id: 'workbench.workspaces.names.primary',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.names.primary'],
		description: 'Default name of the Workspace created with a Project.',
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
	workspaceDuplicate: {
		id: 'workbench.workspaces.actions.duplicate',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.duplicate'],
		description: 'Action that duplicates a Workspace without copying running processes.',
	},
	workspaceManage: {
		id: 'workbench.workspaces.actions.manage',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.manage'],
		description: 'Action that opens the Workspace management interface.',
	},
	workspaceManageDescription: {
		id: 'workbench.workspaces.manage.description',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.description'],
		description: 'Accessible description for the Workspace management interface.',
	},
	workspaceManageNameOnlyDescription: {
		id: 'workbench.workspaces.manage.nameOnlyDescription',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.nameOnlyDescription'],
		description: 'Description of the Workspace management interface when only its name can change.',
	},
	workspaceManageName: {
		id: 'workbench.workspaces.manage.name',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.name'],
		description: 'Label for the Workspace name field.',
	},
	workspaceManageNamePlaceholder: {
		id: 'workbench.workspaces.manage.namePlaceholder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.namePlaceholder'],
		description: 'Placeholder for the Workspace name field.',
	},
	workspaceManagePrimaryFolder: {
		id: 'workbench.workspaces.manage.primaryFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.primaryFolder'],
		description: 'Label for the Workspace primary Folder field.',
	},
	workspaceManageTitle: {
		id: 'workbench.workspaces.manage.title',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.title'],
		description: 'Title for the Workspace management interface.',
	},
	workspaceOptions: {
		id: 'workbench.workspaces.actions.options',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.options'],
		description: 'Accessible label for a Workspace options menu.',
	},
	workspaceReorder: {
		id: 'workbench.workspaces.actions.reorder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.reorder'],
		description: 'Action that reorders a Workspace inside its owning Project.',
	},
	searchActionsNeedContext: {
		id: 'workbench.search.actions.needsContext',
		defaultMessage: enUSWorkbenchMessages['workbench.search.actions.needsContext'],
		description:
			'Explanation for a discovered action that needs a target not available in unified search.',
	},
	searchArgumentHorizontalTabs: {
		id: 'workbench.search.arguments.horizontalTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.horizontalTabs'],
		description: 'Argument option that places the Workbench tab strip horizontally.',
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
	searchArgumentVerticalTabs: {
		id: 'workbench.search.arguments.verticalTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.verticalTabs'],
		description: 'Argument option that places the Workbench tab strip vertically.',
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
	tabStripOrientation: {
		id: 'workbench.tabs.actions.setOrientation',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.setOrientation'],
		description: 'Action that sets the Workbench tab-strip orientation.',
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
		description: 'Workspace menu command that creates a Git Worktree with defaults and opens it.',
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
});
