export const enUSBrowserAuthenticationMessages = {
	'browser.authentication.description':
		'Enter the Personal Access Token created in Orchbee Desktop.',
	'browser.authentication.documentTitle': 'Sign in — Orchbee',
	'browser.authentication.error.invalidOrigin':
		'This sign-in request did not come from the current Orchbee address. Reload the page and try again.',
	'browser.authentication.error.invalidPersonalAccessToken':
		'That Personal Access Token is not valid.',
	'browser.authentication.error.invalidRequest': 'Enter a valid Personal Access Token.',
	'browser.authentication.error.rateLimited':
		'Too many unsuccessful attempts. Wait a minute, then try again.',
	'browser.authentication.error.required': 'Enter your Personal Access Token.',
	'browser.authentication.error.unavailable':
		'Orchbee could not complete sign-in. Check the connection and try again.',
	'browser.authentication.personalAccessTokenLabel': 'Personal Access Token',
	'browser.authentication.personalAccessTokenPlaceholder': 'Paste your Personal Access Token',
	'browser.authentication.securityNotice':
		'Your token is sent only to this Orchbee address for sign-in and is not stored in Browser storage. Local network Browser access uses HTTP, so credentials and terminal traffic are not encrypted; use it only on a trusted network.',
	'browser.authentication.submit': 'Sign in',
	'browser.authentication.submitting': 'Signing in…',
	'browser.authentication.title': 'Sign in to Orchbee',
} as const;

export type BrowserAuthenticationMessageId = keyof typeof enUSBrowserAuthenticationMessages;
