import {
	TerminalPresentationFailureSchema,
	TerminalPresentationIdSchema,
} from '@orchbee/workbench/terminal-presentation';
import { TerminalGeometrySchema } from '@orchbee/terminal-protocol';
import { z } from 'zod';

import { ProjectedAttachmentBootstrapSchema } from './projected-terminal-contract';

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
		geometry: TerminalGeometrySchema,
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
	z.strictObject({
		...projectionIdentitySchema,
		type: z.literal('terminal-projection-released'),
	}),
]);

export type TerminalProjectionControlEvent = z.infer<typeof TerminalProjectionControlEventSchema>;
export type TerminalProjectionRequestId = z.infer<typeof TerminalProjectionRequestIdSchema>;
