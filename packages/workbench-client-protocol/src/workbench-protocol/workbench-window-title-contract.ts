import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
// Rust trim uses Unicode White_Space, unlike JS trim (which also strips BOM).
const WindowTitleSchema = z
	.string()
	.min(1)
	.max(400)
	.refine(
		(value) =>
			!/[\uD800-\uDFFF]/u.test(value) &&
			[...value].length <= 200 &&
			!/\p{Cc}/u.test(value) &&
			!/^\p{White_Space}|\p{White_Space}$/u.test(value),
	);
const contextText = z
	.string()
	.max(400)
	.refine(
		(value) =>
			!/[\uD800-\uDFFF]/u.test(value) && [...value].length <= 200 && !/\p{Cc}/u.test(value),
	);
const context = z.strictObject({
	workspace: contextText,
	tab: contextText,
	panel: contextText,
	terminalTitle: contextText,
});
const snapshot = z.strictObject({
	revision: WorkbenchRevisionSchema.refine((value) => value !== '0'),
	title: WindowTitleSchema.nullable(),
});
const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};
export const WindowTitleRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.window-title'),
		...correlation,
		action: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('set'), title: WindowTitleSchema }),
			z.strictObject({ kind: z.literal('clear') }),
			z.strictObject({ kind: z.literal('listen'), focused: z.boolean(), context }),
		]),
	})
	.readonly();
export const WindowTitleResultSchema = z
	.strictObject({
		kind: z.literal('workbench.window-title.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z
				.strictObject({
					kind: z.literal('published'),
					snapshot,
					changed: z.boolean(),
					reason: z.enum(['set', 'cleared', 'no-foreground-client']),
				})
				.refine((value) => {
					if (!value.changed) return value.reason === 'no-foreground-client';
					const expected = value.snapshot.title === null ? 'cleared' : 'set';
					return value.reason === expected;
				}),
			z.strictObject({ kind: z.literal('listening'), snapshot: snapshot.nullable() }),
			z.strictObject({
				kind: z.literal('failed'),
				reason: z.enum(['denied', 'unavailable', 'session-mismatch', 'conflict', 'busy']),
			}),
		]),
	})
	.readonly();
