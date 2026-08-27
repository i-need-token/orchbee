import type { DesktopBrowserAccessFailureCode } from '@orchbee/desktop-client-protocol';
import { desktopBrowserAccessMessages, type MessageDescriptor } from '@orchbee/i18n';

export const DESKTOP_BROWSER_ACCESS_ERROR_MESSAGES = {
	'browser-open-failed': desktopBrowserAccessMessages.browserOpenFailed,
	'clipboard-write-failed': desktopBrowserAccessMessages.clipboardWriteFailed,
	'desktop-quiescing': desktopBrowserAccessMessages.desktopQuiescing,
	'gateway-unavailable': desktopBrowserAccessMessages.gatewayUnavailable,
	'invalid-origin': desktopBrowserAccessMessages.invalidOrigin,
	'network-address-unavailable': desktopBrowserAccessMessages.networkAddressUnavailable,
	'personal-access-token-required': desktopBrowserAccessMessages.personalAccessTokenRequired,
	'personal-access-token-store-unavailable':
		desktopBrowserAccessMessages.personalAccessTokenStoreUnavailable,
	'server-unavailable': desktopBrowserAccessMessages.serverUnavailable,
} as const satisfies Readonly<Record<DesktopBrowserAccessFailureCode, MessageDescriptor>>;
