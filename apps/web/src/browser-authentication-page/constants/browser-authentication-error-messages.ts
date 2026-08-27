import { browserAuthenticationMessages, type MessageDescriptor } from '@orchbee/i18n';

import type { BrowserAuthenticationViewError } from '../model/browser-authentication-controller';

export const BROWSER_AUTHENTICATION_ERROR_MESSAGES = {
	'invalid-origin': browserAuthenticationMessages.invalidOrigin,
	'invalid-personal-access-token': browserAuthenticationMessages.invalidPersonalAccessToken,
	'invalid-request': browserAuthenticationMessages.invalidRequest,
	'rate-limited': browserAuthenticationMessages.rateLimited,
	required: browserAuthenticationMessages.required,
	unavailable: browserAuthenticationMessages.unavailable,
} as const satisfies Readonly<Record<BrowserAuthenticationViewError, MessageDescriptor>>;
