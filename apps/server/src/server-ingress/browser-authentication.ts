import {
	BrowserSessionIdSchema,
	BrowserSessionSigningKeySchema,
	createBrowserSessionCredential,
	PersonalAccessTokenVerifierSchema,
	readBrowserSessionCredential,
	serializeBrowserSessionCookie,
	verifyBrowserSessionCredential,
	type BrowserSessionSigningKey,
	type PersonalAccessToken,
	type PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';
import { createHash, createHmac, randomBytes, timingSafeEqual } from 'node:crypto';

export interface BrowserAuthenticationConfiguration {
	readonly personalAccessTokenVerifier: PersonalAccessTokenVerifier;
	readonly sessionSigningKey: BrowserSessionSigningKey;
}

export interface BrowserAuthenticationService {
	readonly createSessionCookie: (secure: boolean) => string;
	readonly isAuthenticated: (cookieHeader: string | undefined) => boolean;
	readonly verifyPersonalAccessToken: (personalAccessToken: PersonalAccessToken) => boolean;
}

export function createBrowserAuthenticationService(
	configuration: BrowserAuthenticationConfiguration,
): BrowserAuthenticationService {
	const expectedPersonalAccessTokenVerifier = decodeSecret(
		PersonalAccessTokenVerifierSchema.parse(configuration.personalAccessTokenVerifier),
	);
	const sessionSigningKey = decodeSecret(
		BrowserSessionSigningKeySchema.parse(configuration.sessionSigningKey),
	);
	const signSessionPayload = (payload: string): string =>
		createHmac('sha256', sessionSigningKey).update(payload, 'utf8').digest('base64url');

	return {
		createSessionCookie: (secure) => {
			const sessionId = BrowserSessionIdSchema.parse(randomBytes(32).toString('base64url'));
			const credential = createBrowserSessionCredential(sessionId, signSessionPayload);
			return serializeBrowserSessionCookie(credential, { secure });
		},
		isAuthenticated: (cookieHeader) => {
			const credential = readBrowserSessionCredential(cookieHeader);
			if (credential === null) return false;
			return verifyBrowserSessionCredential(credential, (payload, signature) => {
				const expectedSignature = Buffer.from(signSessionPayload(payload), 'ascii');
				const candidateSignature = Buffer.from(signature, 'ascii');
				return (
					candidateSignature.byteLength === expectedSignature.byteLength &&
					timingSafeEqual(candidateSignature, expectedSignature)
				);
			});
		},
		verifyPersonalAccessToken: (personalAccessToken) => {
			const candidateVerifier = createHash('sha256').update(personalAccessToken, 'utf8').digest();
			return timingSafeEqual(candidateVerifier, expectedPersonalAccessTokenVerifier);
		},
	};
}

function decodeSecret(value: string): Buffer {
	const decoded = Buffer.from(value, 'base64url');
	if (decoded.byteLength !== 32) throw new Error('A canonical 256-bit secret is required.');
	return decoded;
}

export interface FailedAuthenticationRateLimiter {
	readonly clear: (source: string) => void;
	readonly isLimited: (source: string) => boolean;
	readonly recordFailure: (source: string) => void;
}

export function createFailedAuthenticationRateLimiter(options?: {
	readonly clock?: () => number;
	readonly maximumFailures?: number;
	readonly maximumSources?: number;
	readonly windowMilliseconds?: number;
}): FailedAuthenticationRateLimiter {
	const clock = options?.clock ?? Date.now;
	const maximumFailures = options?.maximumFailures ?? 5;
	const maximumSources = options?.maximumSources ?? 1_024;
	const windowMilliseconds = options?.windowMilliseconds ?? 60_000;
	const failures = new Map<string, { readonly count: number; readonly expiresAt: number }>();

	const activeEntry = (
		source: string,
	): { readonly count: number; readonly expiresAt: number } | null => {
		const entry = failures.get(source);
		if (entry === undefined) return null;
		if (entry.expiresAt > clock()) return entry;
		failures.delete(source);
		return null;
	};

	return {
		clear: (source) => failures.delete(source),
		isLimited: (source) => (activeEntry(source)?.count ?? 0) >= maximumFailures,
		recordFailure: (source) => {
			const now = clock();
			const current = activeEntry(source);
			if (current !== null) {
				failures.set(source, { count: current.count + 1, expiresAt: current.expiresAt });
				return;
			}
			while (failures.size >= maximumSources) {
				const oldest = failures.keys().next();
				if (oldest.done) break;
				failures.delete(oldest.value);
			}
			failures.set(source, { count: 1, expiresAt: now + windowMilliseconds });
		},
	};
}

export function isSameOriginRequest(input: {
	readonly host: string | undefined;
	readonly origin: string | undefined;
	readonly protocol: string;
}): boolean {
	if (input.host === undefined || input.origin === undefined) return false;
	try {
		const origin = new URL(input.origin);
		return (
			origin.origin === input.origin &&
			origin.origin === new URL(`${input.protocol}://${input.host}`).origin
		);
	} catch {
		return false;
	}
}
