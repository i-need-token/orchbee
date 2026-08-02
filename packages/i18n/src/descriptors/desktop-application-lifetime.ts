import { defineMessages } from 'react-intl';

import { enUSDesktopApplicationLifetimeMessages } from '../locales';

export const desktopApplicationLifetimeMessages = defineMessages({
	cancel: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.actions.cancel'],
		id: 'desktop.applicationLifetime.actions.cancel',
	},
	exitDescription: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.exit.description'],
		description: 'Explains how many terminal sessions are running before application exit.',
		id: 'desktop.applicationLifetime.exit.description',
	},
	exitTitle: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.exit.title'],
		id: 'desktop.applicationLifetime.exit.title',
	},
	failedDescription: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.failed.description'],
		id: 'desktop.applicationLifetime.failed.description',
	},
	failedTitle: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.failed.title'],
		id: 'desktop.applicationLifetime.failed.title',
	},
	preserveAndQuit: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.actions.preserveAndQuit'],
		description: 'Default exit action that leaves terminal sessions running.',
		id: 'desktop.applicationLifetime.actions.preserveAndQuit',
	},
	progressDraining: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.progress.draining'],
		id: 'desktop.applicationLifetime.progress.draining',
	},
	progressExiting: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.progress.exiting'],
		id: 'desktop.applicationLifetime.progress.exiting',
	},
	progressFlushing: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.progress.flushing'],
		id: 'desktop.applicationLifetime.progress.flushing',
	},
	progressStopping: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.progress.stopping'],
		id: 'desktop.applicationLifetime.progress.stopping',
	},
	progressTitle: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.progress.title'],
		id: 'desktop.applicationLifetime.progress.title',
	},
	quitAnyway: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.actions.quitAnyway'],
		id: 'desktop.applicationLifetime.actions.quitAnyway',
	},
	retry: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.actions.retry'],
		id: 'desktop.applicationLifetime.actions.retry',
	},
	stopAndQuit: {
		defaultMessage:
			enUSDesktopApplicationLifetimeMessages['desktop.applicationLifetime.actions.stopAndQuit'],
		description: 'Destructive exit action that terminates all owned terminal sessions.',
		id: 'desktop.applicationLifetime.actions.stopAndQuit',
	},
});
