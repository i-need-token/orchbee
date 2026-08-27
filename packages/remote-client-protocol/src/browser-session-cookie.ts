import { z } from 'zod';

import { BrowserSessionIdSchema, type BrowserSessionId } from './authentication-contract';

export const BROWSER_SESSION_COOKIE_NAME = 'orchbee-browser-session';
const BROWSER_SESSION_VERSION = 'v1';
const browserSessionSignatureSchema = z
	.string()
	.length(43)
	.regex(/^[A-Za-z0-9_-]+$/u);

export type BrowserSessionSigner = (payload: string) => string;
export type BrowserSessionSignatureVerifier = (payload: string, signature: string) => boolean;

export function createBrowserSessionCredential(
	sessionId: BrowserSessionId,
	sign: BrowserSessionSigner,
): string {
	const canonicalSessionId = BrowserSessionIdSchema.parse(sessionId);
	const payload = `${BROWSER_SESSION_VERSION}.${canonicalSessionId}`;
	const signature = browserSessionSignatureSchema.parse(sign(payload));
	return `${payload}.${signature}`;
}

export function verifyBrowserSessionCredential(
	credential: string,
	verify: BrowserSessionSignatureVerifier,
): boolean {
	const parts = credential.split('.');
	if (parts.length !== 3 || parts[0] !== BROWSER_SESSION_VERSION) return false;
	const sessionId = BrowserSessionIdSchema.safeParse(parts[1]);
	const signature = browserSessionSignatureSchema.safeParse(parts[2]);
	if (!sessionId.success || !signature.success) return false;
	return verify(`${BROWSER_SESSION_VERSION}.${sessionId.data}`, signature.data);
}

export function readBrowserSessionCredential(cookieHeader: string | undefined): string | null {
	if (cookieHeader === undefined) return null;
	let credential: string | null = null;
	for (const segment of cookieHeader.split(';')) {
		const separator = segment.indexOf('=');
		if (separator === -1) continue;
		const name = segment.slice(0, separator).trim();
		if (name !== BROWSER_SESSION_COOKIE_NAME) continue;
		if (credential !== null) return null;
		credential = segment.slice(separator + 1).trim();
	}
	return credential === '' ? null : credential;
}

export function serializeBrowserSessionCookie(
	credential: string,
	options: { readonly secure: boolean },
): string {
	if (!verifyBrowserSessionCredential(credential, () => true)) {
		throw new Error('A canonical Browser session credential is required.');
	}
	return [
		`${BROWSER_SESSION_COOKIE_NAME}=${credential}`,
		'HttpOnly',
		'Path=/',
		'SameSite=Strict',
		...(options.secure ? ['Secure'] : []),
	].join('; ');
}
