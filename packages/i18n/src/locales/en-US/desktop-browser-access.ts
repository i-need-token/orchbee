export const enUSDesktopBrowserAccessMessages = {
	'desktop.browserAccess.actions.copyLink': 'Copy link',
	'desktop.browserAccess.actions.copyPersonalAccessToken': 'Copy token',
	'desktop.browserAccess.actions.copied': 'Copied',
	'desktop.browserAccess.actions.createPersonalAccessToken': 'Create Personal Access Token',
	'desktop.browserAccess.actions.enableLocalNetwork': 'Enable local network preview',
	'desktop.browserAccess.actions.openBrowser': 'Open browser',
	'desktop.browserAccess.actions.openThisComputer': 'Open on this computer',
	'desktop.browserAccess.actions.personalAccessTokenCopied': 'Token copied',
	'desktop.browserAccess.actions.resetPersonalAccessToken': 'Reset Personal Access Token',
	'desktop.browserAccess.actions.resetPersonalAccessTokenCancel': 'Cancel',
	'desktop.browserAccess.actions.resetPersonalAccessTokenConfirm': 'Reset token',
	'desktop.browserAccess.actions.stop': 'Stop browser access',
	'desktop.browserAccess.actions.switchLoopback': 'Limit to this computer',
	'desktop.browserAccess.address.localNetwork': 'Local network',
	'desktop.browserAccess.address.thisComputer': 'This computer',
	'desktop.browserAccess.addresses': 'Access addresses',
	'desktop.browserAccess.close': 'Close Browser access',
	'desktop.browserAccess.description':
		'Use the current Desktop terminal from a signed-in browser while this Orchbee process is running.',
	'desktop.browserAccess.error.browserOpenFailed': 'The system browser could not be opened.',
	'desktop.browserAccess.error.clipboardWriteFailed':
		'The value could not be copied to the clipboard.',
	'desktop.browserAccess.error.desktopQuiescing': 'Orchbee is shutting down.',
	'desktop.browserAccess.error.gatewayUnavailable': 'The Browser gateway is unavailable.',
	'desktop.browserAccess.error.invalidOrigin': 'That access address is no longer active.',
	'desktop.browserAccess.error.networkAddressUnavailable':
		'No supported private IPv4 address is available.',
	'desktop.browserAccess.error.personalAccessTokenRequired':
		'Create a Personal Access Token before starting Browser access.',
	'desktop.browserAccess.error.personalAccessTokenStoreUnavailable':
		'The Personal Access Token could not be loaded or saved.',
	'desktop.browserAccess.error.serverUnavailable': 'The Web server is unavailable.',
	'desktop.browserAccess.progress.starting': 'Starting Browser access…',
	'desktop.browserAccess.progress.stopping': 'Stopping Browser access…',
	'desktop.browserAccess.progress.updatingPersonalAccessToken': 'Updating Personal Access Token…',
	'desktop.browserAccess.personalAccessToken.configured': 'Configured',
	'desktop.browserAccess.personalAccessToken.description':
		'Browsers use this installation-scoped token to sign in. Orchbee stores only a verifier and shows the token once.',
	'desktop.browserAccess.personalAccessToken.missing': 'Not created',
	'desktop.browserAccess.personalAccessToken.oneTimeDescription':
		'Copy this token now. For your security, it will not be shown again.',
	'desktop.browserAccess.personalAccessToken.oneTimeTitle': 'Your new Personal Access Token',
	'desktop.browserAccess.personalAccessToken.resetDescription':
		'Resetting stops Browser access, signs out every Browser session, and permanently invalidates the current token.',
	'desktop.browserAccess.personalAccessToken.resetTitle': 'Reset Personal Access Token?',
	'desktop.browserAccess.personalAccessToken.title': 'Personal Access Token',
	'desktop.browserAccess.personalAccessToken.unavailable': 'Storage unavailable',
	'desktop.browserAccess.status.localNetwork': 'Available on your trusted local network',
	'desktop.browserAccess.status.loopback': 'Available only on this computer',
	'desktop.browserAccess.status.stopped': 'Browser access is off',
	'desktop.browserAccess.status.stoppedDescription':
		'Nothing is listening until you choose an access mode.',
	'desktop.browserAccess.title': 'Browser access',
	'desktop.browserAccess.warning.description':
		'Local network preview uses HTTP, so credentials and terminal traffic are not encrypted. Anyone with your Personal Access Token can fully control your terminals. Use it only on a trusted network and never share the token.',
	'desktop.browserAccess.warning.title': 'Unencrypted terminal access on trusted networks only',
} as const;

export type DesktopBrowserAccessMessageId = keyof typeof enUSDesktopBrowserAccessMessages;
