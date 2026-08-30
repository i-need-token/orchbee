import { defineMessages } from 'react-intl';

import { enUSRemoteDirectoryBrowserMessages } from '../locales';

export const remoteDirectoryBrowserMessages = defineMessages({
	cancel: message('remoteDirectoryBrowser.actions.cancel'),
	editPath: message('remoteDirectoryBrowser.actions.editPath'),
	empty: message('remoteDirectoryBrowser.empty'),
	emptyHidden: message('remoteDirectoryBrowser.empty.hidden'),
	failureBusy: message('remoteDirectoryBrowser.failure.busy'),
	failureInvalidPath: message('remoteDirectoryBrowser.failure.invalidPath'),
	failureNotDirectory: message('remoteDirectoryBrowser.failure.notDirectory'),
	failureNotFound: message('remoteDirectoryBrowser.failure.notFound'),
	failurePermission: message('remoteDirectoryBrowser.failure.permission'),
	failureTimedOut: message('remoteDirectoryBrowser.failure.timedOut'),
	failureUnavailable: message('remoteDirectoryBrowser.failure.unavailable'),
	filterNoMatches: message('remoteDirectoryBrowser.filter.noMatches'),
	filterPartial: message('remoteDirectoryBrowser.filter.partial'),
	go: message('remoteDirectoryBrowser.actions.go'),
	listLabel: message('remoteDirectoryBrowser.listLabel'),
	loading: message('remoteDirectoryBrowser.loading'),
	locationLabel: message('remoteDirectoryBrowser.locationLabel'),
	path: message('remoteDirectoryBrowser.path'),
	showHidden: message('remoteDirectoryBrowser.actions.showHidden'),
	truncated: message('remoteDirectoryBrowser.truncated'),
	useFolder: message('remoteDirectoryBrowser.actions.useFolder'),
});

function message(id: keyof typeof enUSRemoteDirectoryBrowserMessages) {
	return { defaultMessage: enUSRemoteDirectoryBrowserMessages[id], id };
}
