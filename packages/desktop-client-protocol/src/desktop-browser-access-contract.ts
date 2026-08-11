import { z } from 'zod';

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
]);

export const DesktopBrowserAccessFailureCodeSchema = z.enum([
	'browser-open-failed',
	'clipboard-write-failed',
	'desktop-quiescing',
	'gateway-unavailable',
	'invalid-origin',
	'network-address-unavailable',
	'server-unavailable',
]);

export const DesktopBrowserAccessMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopBrowserAccessRequestIdSchema,
		state: DesktopBrowserAccessStateSchema,
		type: z.literal('desktop-browser-access-completed'),
	}),
	z.strictObject({
		code: DesktopBrowserAccessFailureCodeSchema,
		requestId: DesktopBrowserAccessRequestIdSchema,
		state: DesktopBrowserAccessStateSchema,
		type: z.literal('desktop-browser-access-failed'),
	}),
]);

export type DesktopBrowserAccessFailureCode = z.infer<typeof DesktopBrowserAccessFailureCodeSchema>;
export type DesktopBrowserAccessMainEvent = z.infer<typeof DesktopBrowserAccessMainEventSchema>;
export type DesktopBrowserAccessMode = z.infer<typeof DesktopBrowserAccessModeSchema>;
export type DesktopBrowserAccessRendererCommand = z.infer<
	typeof DesktopBrowserAccessRendererCommandSchema
>;
export type DesktopBrowserAccessRequestId = z.infer<typeof DesktopBrowserAccessRequestIdSchema>;
export type DesktopBrowserAccessState = z.infer<typeof DesktopBrowserAccessStateSchema>;
