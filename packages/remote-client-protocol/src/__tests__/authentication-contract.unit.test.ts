import { describe, expect, it } from 'vitest';

import {
	BROWSER_AUTHENTICATION_SESSION_PATH,
	BrowserAuthenticationRequestSchema,
	BrowserSessionSigningKeySchema,
	PersonalAccessTokenSchema,
} from '../authentication-contract';

describe('remote Browser authentication contract', () => {
	it('accepts one canonical Personal Access Token exchange request', () => {
		const token = 'a'.repeat(43);

		expect(BrowserAuthenticationRequestSchema.parse({ personalAccessToken: token })).toEqual({
			personalAccessToken: token,
		});
		expect(BROWSER_AUTHENTICATION_SESSION_PATH).toBe('/api/v1/auth/session');
	});

	it('keeps tokens and session signing keys high-entropy and canonical', () => {
		for (const invalid of ['', 'a'.repeat(42), 'a'.repeat(44), `${'a'.repeat(42)}+`]) {
			expect(PersonalAccessTokenSchema.safeParse(invalid).success).toBe(false);
			expect(BrowserSessionSigningKeySchema.safeParse(invalid).success).toBe(false);
		}
	});
});
