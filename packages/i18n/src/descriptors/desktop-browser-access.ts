import { defineMessages } from 'react-intl';

import { enUSDesktopBrowserAccessMessages } from '../locales';

export const desktopBrowserAccessMessages = defineMessages({
	addresses: message('desktop.browserAccess.addresses'),
	browserOpenFailed: message('desktop.browserAccess.error.browserOpenFailed'),
	clipboardWriteFailed: message('desktop.browserAccess.error.clipboardWriteFailed'),
	close: message('desktop.browserAccess.close'),
	copyLink: message('desktop.browserAccess.actions.copyLink'),
	copied: message('desktop.browserAccess.actions.copied'),
	description: message('desktop.browserAccess.description'),
	desktopQuiescing: message('desktop.browserAccess.error.desktopQuiescing'),
	enableLocalNetwork: message('desktop.browserAccess.actions.enableLocalNetwork'),
	gatewayUnavailable: message('desktop.browserAccess.error.gatewayUnavailable'),
	invalidOrigin: message('desktop.browserAccess.error.invalidOrigin'),
	localNetwork: message('desktop.browserAccess.address.localNetwork'),
	localNetworkStatus: message('desktop.browserAccess.status.localNetwork'),
	networkAddressUnavailable: message('desktop.browserAccess.error.networkAddressUnavailable'),
	openBrowser: message('desktop.browserAccess.actions.openBrowser'),
	openThisComputer: message('desktop.browserAccess.actions.openThisComputer'),
	serverUnavailable: message('desktop.browserAccess.error.serverUnavailable'),
	startProgress: message('desktop.browserAccess.progress.starting'),
	stop: message('desktop.browserAccess.actions.stop'),
	stopProgress: message('desktop.browserAccess.progress.stopping'),
	stoppedDescription: message('desktop.browserAccess.status.stoppedDescription'),
	stoppedStatus: message('desktop.browserAccess.status.stopped'),
	switchLoopback: message('desktop.browserAccess.actions.switchLoopback'),
	thisComputer: message('desktop.browserAccess.address.thisComputer'),
	loopbackStatus: message('desktop.browserAccess.status.loopback'),
	title: message('desktop.browserAccess.title'),
	warningDescription: message('desktop.browserAccess.warning.description'),
	warningTitle: message('desktop.browserAccess.warning.title'),
});

function message(id: keyof typeof enUSDesktopBrowserAccessMessages) {
	return {
		defaultMessage: enUSDesktopBrowserAccessMessages[id],
		id,
	};
}
