import { defineMessages } from 'react-intl';

import { enUSBrowserAuthenticationMessages } from '../locales';

export const browserAuthenticationMessages = defineMessages({
	description: message('browser.authentication.description'),
	documentTitle: message('browser.authentication.documentTitle'),
	invalidOrigin: message('browser.authentication.error.invalidOrigin'),
	invalidPersonalAccessToken: message('browser.authentication.error.invalidPersonalAccessToken'),
	invalidRequest: message('browser.authentication.error.invalidRequest'),
	personalAccessTokenLabel: message('browser.authentication.personalAccessTokenLabel'),
	personalAccessTokenPlaceholder: message('browser.authentication.personalAccessTokenPlaceholder'),
	rateLimited: message('browser.authentication.error.rateLimited'),
	required: message('browser.authentication.error.required'),
	securityNotice: message('browser.authentication.securityNotice'),
	submit: message('browser.authentication.submit'),
	submitting: message('browser.authentication.submitting'),
	title: message('browser.authentication.title'),
	unavailable: message('browser.authentication.error.unavailable'),
});

function message(id: keyof typeof enUSBrowserAuthenticationMessages) {
	return { defaultMessage: enUSBrowserAuthenticationMessages[id], id };
}
