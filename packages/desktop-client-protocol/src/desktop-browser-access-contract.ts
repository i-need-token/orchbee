import { z } from 'zod';

import { PersonalAccessTokenSchema } from '@orchbee/remote-client-protocol';

export const DesktopBrowserAccessRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopBrowserAccessRequestId'>();

export const DesktopBrowserAccessModeSchema = z.enum(['loopback', 'local-network']);

const desktopBrowserAccessOriginSchema = z
	.string()
	.min(1)
	.max(2048)
	.refine((value) => {
		try {
			const url = new URL(value);
			return (
				url.origin === value &&
				url.protocol === 'http:' &&
				url.username === '' &&
				url.password === ''
			);
		} catch {
			return false;
		}
	}, 'Browser access origins must be exact HTTP origins.');

export const DesktopBrowserAccessStateSchema = z.discriminatedUnion('phase', [
	z.strictObject({ phase: z.literal('stopped') }),
	z.strictObject({
		mode: DesktopBrowserAccessModeSchema,
		phase: z.literal('starting'),
	}),
	z.strictObject({
		localOrigin: desktopBrowserAccessOriginSchema,
		mode: DesktopBrowserAccessModeSchema,
		networkOrigins: z.array(desktopBrowserAccessOriginSchema).max(32),
		phase: z.literal('ready'),
	}),
	z.strictObject({
		mode: DesktopBrowserAccessModeSchema,
		phase: z.literal('stopping'),
	}),
]);

export const DesktopPersonalAccessTokenStatusSchema = z.enum([
	'configured',
	'missing',
	'unavailable',
]);

export const DesktopBrowserAccessSnapshotSchema = z.strictObject({
	access: DesktopBrowserAccessStateSchema,
	personalAccessToken: DesktopPersonalAccessTokenStatusSchema,
});

export const DesktopBrowserAccessRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopBrowserAccessRequestIdSchema,
		type: z.literal('desktop-browser-access-state-requested'),
	}),
	z.strictObject({
		mode: DesktopBrowserAccessModeSchema,
		requestId: DesktopBrowserAccessRequestIdSchema,
		type: z.literal('desktop-browser-access-start-requested'),
	}),
	z.strictObject({
		requestId: DesktopBrowserAccessRequestIdSchema,
		type: z.literal('desktop-browser-access-open-requested'),
	}),
	z.strictObject({
		origin: desktopBrowserAccessOriginSchema,
		requestId: DesktopBrowserAccessRequestIdSchema,
		type: z.literal('desktop-browser-access-copy-link-requested'),
	}),
	z.strictObject({
		requestId: DesktopBrowserAccessRequestIdSchema,
		type: z.literal('desktop-browser-access-stop-requested'),
	}),
	z.strictObject({
		requestId: DesktopBrowserAccessRequestIdSchema,
		type: z.literal('desktop-browser-access-personal-access-token-issue-requested'),
	}),
]);

export const DesktopBrowserAccessFailureCodeSchema = z.enum([
	'browser-open-failed',
	'clipboard-write-failed',
	'desktop-quiescing',
	'gateway-unavailable',
	'invalid-origin',
	'network-address-unavailable',
	'personal-access-token-required',
	'personal-access-token-store-unavailable',
	'server-unavailable',
]);

export const DesktopBrowserAccessMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopBrowserAccessRequestIdSchema,
		snapshot: DesktopBrowserAccessSnapshotSchema,
		type: z.literal('desktop-browser-access-completed'),
	}),
	z.strictObject({
		personalAccessToken: PersonalAccessTokenSchema,
		requestId: DesktopBrowserAccessRequestIdSchema,
		snapshot: DesktopBrowserAccessSnapshotSchema,
		type: z.literal('desktop-browser-access-personal-access-token-issued'),
	}),
	z.strictObject({
		code: DesktopBrowserAccessFailureCodeSchema,
		requestId: DesktopBrowserAccessRequestIdSchema,
		snapshot: DesktopBrowserAccessSnapshotSchema,
		type: z.literal('desktop-browser-access-failed'),
	}),
]);

export type DesktopBrowserAccessSnapshot = z.infer<typeof DesktopBrowserAccessSnapshotSchema>;
export type DesktopBrowserAccessFailureCode = z.infer<typeof DesktopBrowserAccessFailureCodeSchema>;
export type DesktopBrowserAccessMainEvent = z.infer<typeof DesktopBrowserAccessMainEventSchema>;
export type DesktopBrowserAccessMode = z.infer<typeof DesktopBrowserAccessModeSchema>;
export type DesktopBrowserAccessRendererCommand = z.infer<
	typeof DesktopBrowserAccessRendererCommandSchema
>;
export type DesktopBrowserAccessRequestId = z.infer<typeof DesktopBrowserAccessRequestIdSchema>;
export type DesktopBrowserAccessState = z.infer<typeof DesktopBrowserAccessStateSchema>;
export type DesktopPersonalAccessTokenStatus = z.infer<
	typeof DesktopPersonalAccessTokenStatusSchema
>;
