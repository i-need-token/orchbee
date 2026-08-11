export const enUSDesktopBrowserAccessMessages = {
	'desktop.browserAccess.actions.copyLink': 'Copy link',
	'desktop.browserAccess.actions.copied': 'Copied',
	'desktop.browserAccess.actions.enableLocalNetwork': 'Enable local network preview',
	'desktop.browserAccess.actions.openBrowser': 'Open browser',
	'desktop.browserAccess.actions.openThisComputer': 'Open on this computer',
	'desktop.browserAccess.actions.stop': 'Stop browser access',
	'desktop.browserAccess.actions.switchLoopback': 'Limit to this computer',
	'desktop.browserAccess.address.localNetwork': 'Local network',
	'desktop.browserAccess.address.thisComputer': 'This computer',
	'desktop.browserAccess.addresses': 'Access addresses',
	'desktop.browserAccess.close': 'Close Browser access',
	'desktop.browserAccess.description':
		'Use the current Desktop terminal from a browser while this Orchbee process is running.',
	'desktop.browserAccess.error.browserOpenFailed': 'The system browser could not be opened.',
	'desktop.browserAccess.error.clipboardWriteFailed': 'The access link could not be copied.',
	'desktop.browserAccess.error.desktopQuiescing': 'Orchbee is shutting down.',
	'desktop.browserAccess.error.gatewayUnavailable': 'The Browser gateway is unavailable.',
	'desktop.browserAccess.error.invalidOrigin': 'That access address is no longer active.',
	'desktop.browserAccess.error.networkAddressUnavailable':
		'No supported private IPv4 address is available.',
	'desktop.browserAccess.error.serverUnavailable': 'The Web server is unavailable.',
	'desktop.browserAccess.progress.starting': 'Starting Browser access…',
	'desktop.browserAccess.progress.stopping': 'Stopping Browser access…',
	'desktop.browserAccess.status.localNetwork': 'Available on your trusted local network',
	'desktop.browserAccess.status.loopback': 'Available only on this computer',
	'desktop.browserAccess.status.stopped': 'Browser access is off',
	'desktop.browserAccess.status.stoppedDescription':
		'Nothing is listening until you choose an access mode.',
	'desktop.browserAccess.title': 'Browser access',
	'desktop.browserAccess.warning.description':
		'Local network preview uses HTTP. Anyone who obtains an access link can fully control your terminals until Browser access is stopped or Orchbee quits. Use it only on a trusted network and do not share the link.',
	'desktop.browserAccess.warning.title': 'Full terminal control on trusted networks only',
} as const;

export type DesktopBrowserAccessMessageId = keyof typeof enUSDesktopBrowserAccessMessages;
