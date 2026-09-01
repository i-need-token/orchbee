import { defineMessages } from 'react-intl';

import { enUSDesktopBrowserAccessMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopBrowserAccessMessages);

export const desktopBrowserAccessMessages = defineMessages({
	addresses: message('desktop.browserAccess.addresses'),
	browserOpenFailed: message('desktop.browserAccess.error.browserOpenFailed'),
	clipboardWriteFailed: message('desktop.browserAccess.error.clipboardWriteFailed'),
	close: message('desktop.browserAccess.close'),
	copyLink: message('desktop.browserAccess.actions.copyLink'),
	copyPersonalAccessToken: message('desktop.browserAccess.actions.copyPersonalAccessToken'),
	copied: message('desktop.browserAccess.actions.copied'),
	createPersonalAccessToken: message('desktop.browserAccess.actions.createPersonalAccessToken'),
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
	personalAccessTokenConfigured: message('desktop.browserAccess.personalAccessToken.configured'),
	personalAccessTokenCopied: message('desktop.browserAccess.actions.personalAccessTokenCopied'),
	personalAccessTokenDescription: message('desktop.browserAccess.personalAccessToken.description'),
	personalAccessTokenMissing: message('desktop.browserAccess.personalAccessToken.missing'),
	personalAccessTokenOneTimeDescription: message(
		'desktop.browserAccess.personalAccessToken.oneTimeDescription',
	),
	personalAccessTokenOneTimeTitle: message(
		'desktop.browserAccess.personalAccessToken.oneTimeTitle',
	),
	personalAccessTokenRequired: message('desktop.browserAccess.error.personalAccessTokenRequired'),
	personalAccessTokenResetCancel: message(
		'desktop.browserAccess.actions.resetPersonalAccessTokenCancel',
	),
	personalAccessTokenResetConfirm: message(
		'desktop.browserAccess.actions.resetPersonalAccessTokenConfirm',
	),
	personalAccessTokenResetDescription: message(
		'desktop.browserAccess.personalAccessToken.resetDescription',
	),
	personalAccessTokenResetTitle: message('desktop.browserAccess.personalAccessToken.resetTitle'),
	personalAccessTokenStoreUnavailable: message(
		'desktop.browserAccess.error.personalAccessTokenStoreUnavailable',
	),
	personalAccessTokenTitle: message('desktop.browserAccess.personalAccessToken.title'),
	personalAccessTokenUnavailable: message('desktop.browserAccess.personalAccessToken.unavailable'),
	resetPersonalAccessToken: message('desktop.browserAccess.actions.resetPersonalAccessToken'),
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
	updatePersonalAccessTokenProgress: message(
		'desktop.browserAccess.progress.updatingPersonalAccessToken',
	),
	warningDescription: message('desktop.browserAccess.warning.description'),
	warningTitle: message('desktop.browserAccess.warning.title'),
});
