import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	closeSplit: {
		id: 'workbench.terminal.actions.closeSplit',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.closeSplit'],
		description: 'Menu action that closes the secondary terminal pane.',
	},
	closeTerminal: {
		id: 'workbench.terminal.actions.closeTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.closeTerminal'],
		description: 'Destructive menu action that closes the active terminal session.',
	},
	emptySessionsDescription: {
		id: 'workbench.emptySessions.description',
		defaultMessage: enUSWorkbenchMessages['workbench.emptySessions.description'],
		description: 'Supporting copy shown when no terminal sessions are open.',
	},
	emptySessionsTitle: {
		id: 'workbench.emptySessions.title',
		defaultMessage: enUSWorkbenchMessages['workbench.emptySessions.title'],
		description: 'Empty-state heading shown when no terminal sessions are open.',
	},
	localRuntime: {
		id: 'workbench.runtime.local.label',
		defaultMessage: enUSWorkbenchMessages['workbench.runtime.local.label'],
		description: 'Label for the local runtime shown in the Workbench status area.',
	},
	newTerminal: {
		id: 'workbench.terminal.actions.newTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.newTerminal'],
		description: 'Action that creates and opens a terminal session.',
	},
	sessionCount: {
		id: 'workbench.runtime.sessionCount',
		defaultMessage: enUSWorkbenchMessages['workbench.runtime.sessionCount'],
		description: 'Count of terminal sessions connected to the local runtime.',
	},
	sessionsHeading: {
		id: 'workbench.sessions.heading',
		defaultMessage: enUSWorkbenchMessages['workbench.sessions.heading'],
		description: 'Heading for the Workbench session navigation list.',
	},
	sessionStatusDisconnected: {
		id: 'workbench.session.status.disconnected',
		defaultMessage: enUSWorkbenchMessages['workbench.session.status.disconnected'],
		description: 'Visible status for a terminal session that lost its connection.',
	},
	sessionStatusExited: {
		id: 'workbench.session.status.exited',
		defaultMessage: enUSWorkbenchMessages['workbench.session.status.exited'],
		description: 'Visible status for a terminal session whose process has exited.',
	},
	sessionStatusRunning: {
		id: 'workbench.session.status.running',
		defaultMessage: enUSWorkbenchMessages['workbench.session.status.running'],
		description: 'Visible status for a terminal session whose process is running.',
	},
	sessionStatusStarting: {
		id: 'workbench.session.status.starting',
		defaultMessage: enUSWorkbenchMessages['workbench.session.status.starting'],
		description: 'Visible status for a terminal session whose process is starting.',
	},
	splitDown: {
		id: 'workbench.terminal.actions.splitDown',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.splitDown'],
		description: 'Menu action that opens another terminal pane below the active pane.',
	},
	splitRight: {
		id: 'workbench.terminal.actions.splitRight',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.splitRight'],
		description: 'Menu action that opens another terminal pane to the right of the active pane.',
	},
	terminalActionsLabel: {
		id: 'workbench.terminal.actions.label',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.actions.label'],
		description: 'Accessible name for the menu containing terminal actions.',
	},
	terminalPaneLabel: {
		id: 'workbench.terminal.pane.label',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.pane.label'],
		description: 'Accessible name for a terminal pane. sessionTitle is the session name.',
	},
	terminalStartFailed: {
		id: 'workbench.terminal.start.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.start.failed'],
		description: 'Generic error shown when a terminal pane fails to initialize.',
	},
	terminalStarting: {
		id: 'workbench.terminal.start.pending',
		defaultMessage: enUSWorkbenchMessages['workbench.terminal.start.pending'],
		description: 'Progress message shown while a terminal pane initializes.',
	},
	workbenchNavigationLabel: {
		id: 'workbench.navigation.label',
		defaultMessage: enUSWorkbenchMessages['workbench.navigation.label'],
		description: 'Accessible name for the primary Workbench navigation region.',
	},
});
