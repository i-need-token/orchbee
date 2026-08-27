import { describe, expect, it } from 'vitest';

import { BrowserSessionIdSchema } from '../authentication-contract';
import {
	BROWSER_SESSION_COOKIE_NAME,
	createBrowserSessionCredential,
	readBrowserSessionCredential,
	serializeBrowserSessionCookie,
	verifyBrowserSessionCredential,
} from '../browser-session-cookie';

const sessionId = BrowserSessionIdSchema.parse('s'.repeat(43));
const signature = 'h'.repeat(43);

describe('Browser session Cookie contract', () => {
	it('round-trips a signed opaque session without exposing a Personal Access Token', () => {
		const credential = createBrowserSessionCredential(sessionId, (payload) => {
			expect(payload).toBe(`v1.${sessionId}`);
			return signature;
		});

		expect(
			verifyBrowserSessionCredential(
				credential,
				(payload, candidate) => payload === `v1.${sessionId}` && candidate === signature,
			),
		).toBe(true);
		expect(credential).toBe(`v1.${sessionId}.${signature}`);
	});

	it('rejects malformed, duplicated, and tampered credentials', () => {
		const credential = createBrowserSessionCredential(sessionId, () => signature);

		expect(verifyBrowserSessionCredential(`${credential}x`, () => true)).toBe(false);
		expect(verifyBrowserSessionCredential(credential, () => false)).toBe(false);
		expect(
			readBrowserSessionCredential(
				`${BROWSER_SESSION_COOKIE_NAME}=${credential}; ${BROWSER_SESSION_COOKIE_NAME}=${credential}`,
			),
		).toBeNull();
		expect(readBrowserSessionCredential('theme=dark')).toBeNull();
	});

	it('serializes an HTTP preview session with strict script and cross-site boundaries', () => {
		const credential = createBrowserSessionCredential(sessionId, () => signature);
		const preview = serializeBrowserSessionCookie(credential, { secure: false });
		const tls = serializeBrowserSessionCookie(credential, { secure: true });

		expect(preview).toBe(
			`${BROWSER_SESSION_COOKIE_NAME}=${credential}; HttpOnly; Path=/; SameSite=Strict`,
		);
		expect(tls).toBe(`${preview}; Secure`);
		expect(
			readBrowserSessionCredential(`theme=dark; ${BROWSER_SESSION_COOKIE_NAME}=${credential}`),
		).toBe(credential);
	});
});
