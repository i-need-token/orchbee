import {
	TerminalPresentationFailureSchema,
	TerminalPresentationIdSchema,
} from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

import { ProjectedAttachmentBootstrapSchema } from './projected-attachment-contract';

export const TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL =
	'orchbee.desktop.terminal-projection-control-port';
export const TERMINAL_PROJECTION_CONTROL_WINDOW_MESSAGE =
	'orchbee.desktop.terminal-projection-control-ready';

export const TerminalProjectionRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u)
	.brand<'TerminalProjectionRequestId'>();

const projectionIdentitySchema = {
	presentationId: TerminalPresentationIdSchema,
	requestId: TerminalProjectionRequestIdSchema,
} as const;

export const TerminalProjectionControlCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...projectionIdentitySchema,
		type: z.literal('terminal-projection-open-requested'),
	}),
	z.strictObject({
		...projectionIdentitySchema,
		type: z.literal('terminal-projection-release-requested'),
	}),
]);

export const TerminalProjectionControlEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...projectionIdentitySchema,
		bootstrap: ProjectedAttachmentBootstrapSchema,
		type: z.literal('terminal-projection-opened'),
	}),
	z.strictObject({
		...projectionIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-projection-failed'),
	}),
]);

export const TerminalProjectionControlBootstrapSchema = z.strictObject({
	type: z.literal(TERMINAL_PROJECTION_CONTROL_IPC_CHANNEL),
});

export const TerminalProjectionControlWindowMessageSchema = z.strictObject({
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(TERMINAL_PROJECTION_CONTROL_WINDOW_MESSAGE),
});

export type TerminalProjectionControlEvent = z.infer<typeof TerminalProjectionControlEventSchema>;
export type TerminalProjectionRequestId = z.infer<typeof TerminalProjectionRequestIdSchema>;
