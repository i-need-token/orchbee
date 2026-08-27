import { createHash } from 'node:crypto';

import {
	BROWSER_SESSION_COOKIE_NAME,
	BrowserSessionSigningKeySchema,
	PersonalAccessTokenSchema,
	PersonalAccessTokenVerifierSchema,
} from '@orchbee/remote-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	createBrowserAuthenticationService,
	createFailedAuthenticationRateLimiter,
	isSameOriginRequest,
} from '../browser-authentication';

describe('browser authentication service', () => {
	it('verifies a Personal Access Token and signs a fresh session Cookie', () => {
		const personalAccessToken = PersonalAccessTokenSchema.parse('p'.repeat(43));
		const personalAccessTokenVerifier = PersonalAccessTokenVerifierSchema.parse(
			createHash('sha256').update(personalAccessToken, 'utf8').digest('base64url'),
		);
		const service = createBrowserAuthenticationService({
			personalAccessTokenVerifier,
			sessionSigningKey: BrowserSessionSigningKeySchema.parse('s'.repeat(43)),
		});

		expect(service.verifyPersonalAccessToken(personalAccessToken)).toBe(true);
		expect(service.verifyPersonalAccessToken(PersonalAccessTokenSchema.parse('x'.repeat(43)))).toBe(
			false,
		);
		const cookie = service.createSessionCookie(false);
		expect(cookie).toContain(`${BROWSER_SESSION_COOKIE_NAME}=v1.`);
		expect(cookie).toContain('HttpOnly; Path=/; SameSite=Strict');
		expect(service.isAuthenticated(cookie)).toBe(true);
		const cookieHeader = cookie.slice(0, cookie.indexOf(';'));
		expect(service.isAuthenticated(`${cookieHeader}x`)).toBe(false);
	});
});

describe('failed authentication rate limiter', () => {
	it('limits one source within the bounded window and clears successful sources', () => {
		let now = 1_000;
		const limiter = createFailedAuthenticationRateLimiter({
			clock: () => now,
			maximumFailures: 2,
			maximumSources: 1,
			windowMilliseconds: 100,
		});

		limiter.recordFailure('source-a');
		expect(limiter.isLimited('source-a')).toBe(false);
		limiter.recordFailure('source-a');
		expect(limiter.isLimited('source-a')).toBe(true);
		limiter.clear('source-a');
		expect(limiter.isLimited('source-a')).toBe(false);
		limiter.recordFailure('source-b');
		now += 101;
		expect(limiter.isLimited('source-b')).toBe(false);
	});
});

describe('same-origin request admission', () => {
	it('accepts only one exact Origin matching the request protocol and Host', () => {
		expect(
			isSameOriginRequest({
				host: '127.0.0.1:43110',
				origin: 'http://127.0.0.1:43110',
				protocol: 'http',
			}),
		).toBe(true);
		expect(
			isSameOriginRequest({
				host: '127.0.0.1:43110',
				origin: 'http://127.0.0.1:43110/',
				protocol: 'http',
			}),
		).toBe(false);
		expect(
			isSameOriginRequest({
				host: '127.0.0.1:43110',
				origin: 'http://127.0.0.1:43111',
				protocol: 'http',
			}),
		).toBe(false);
	});
});
