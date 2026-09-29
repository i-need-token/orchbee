import { z } from 'zod';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
const text = (maximum: number) =>
	z
		.string()
		.min(1)
		.max(maximum * 2)
		.refine((value) => {
			if (/[\uD800-\uDFFF]/u.test(value) || [...value].length > maximum || /\p{Cc}/u.test(value))
				return false;
			return (
				!/^ | $| {2}/u.test(value) &&
				![...value].some((ch) => ch !== ' ' && /\p{White_Space}/u.test(ch))
			);
		});
export const NotificationSchema = z.strictObject({
	title: text(80),
	body: text(240).nullable(),
	position: z.enum(['top-left', 'top-right', 'bottom-left', 'bottom-right']).nullable(),
	sound: z.enum(['none', 'done', 'request']),
});
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
};
export const NotificationRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.notification'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('show'), notification: NotificationSchema }),
			z.strictObject({ kind: z.literal('listen'), accepting: z.boolean() }),
		]),
	})
	.readonly();
export const NotificationResultSchema = z
	.strictObject({
		kind: z.literal('workbench.notification.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z
				.strictObject({
					kind: z.literal('show'),
					shown: z.boolean(),
					reason: z.enum(['shown', 'busy', 'rate-limited', 'no-foreground-client', 'disabled']),
				})
				.refine((value) => value.shown === (value.reason === 'shown')),
			z.strictObject({ kind: z.literal('listening'), notification: NotificationSchema.nullable() }),
			z.strictObject({
				kind: z.literal('failed'),
				reason: z.enum(['denied', 'unavailable', 'site-mismatch', 'conflict']),
			}),
		]),
	})
	.readonly();
export type NotificationRequest = z.infer<typeof NotificationRequestSchema>;
export type NotificationResult = z.infer<typeof NotificationResultSchema>;
