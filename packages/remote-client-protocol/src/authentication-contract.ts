import { z } from 'zod';

const secretTextSchema = z
	.string()
	.length(43)
	.regex(/^[A-Za-z0-9_-]+$/u);

export const PersonalAccessTokenSchema = secretTextSchema.brand<'PersonalAccessToken'>();
export const PersonalAccessTokenVerifierSchema =
	secretTextSchema.brand<'PersonalAccessTokenVerifier'>();
export const BrowserSessionIdSchema = secretTextSchema.brand<'BrowserSessionId'>();
export const BrowserSessionSigningKeySchema = secretTextSchema.brand<'BrowserSessionSigningKey'>();

export const BrowserAuthenticationRequestSchema = z.strictObject({
	personalAccessToken: PersonalAccessTokenSchema,
});

export const BrowserAuthenticationErrorCodeSchema = z.enum([
	'invalid-request',
	'invalid-origin',
	'invalid-personal-access-token',
	'rate-limited',
]);

export const BrowserAuthenticationErrorResponseSchema = z.strictObject({
	error: z.strictObject({
		code: BrowserAuthenticationErrorCodeSchema,
	}),
});

export const BROWSER_AUTHENTICATION_LOGIN_PATH = '/login';
export const BROWSER_AUTHENTICATION_SESSION_PATH = '/api/v1/auth/session';

export type BrowserAuthenticationErrorCode = z.infer<typeof BrowserAuthenticationErrorCodeSchema>;
export type BrowserAuthenticationErrorResponse = z.infer<
	typeof BrowserAuthenticationErrorResponseSchema
>;
export type BrowserAuthenticationRequest = z.infer<typeof BrowserAuthenticationRequestSchema>;
export type BrowserSessionId = z.infer<typeof BrowserSessionIdSchema>;
export type BrowserSessionSigningKey = z.infer<typeof BrowserSessionSigningKeySchema>;
export type PersonalAccessToken = z.infer<typeof PersonalAccessTokenSchema>;
export type PersonalAccessTokenVerifier = z.infer<typeof PersonalAccessTokenVerifierSchema>;
