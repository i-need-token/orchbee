import {
	TerminalPresentationFailureSchema,
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

const operationIdentitySchema = {
	operationId: TerminalPresentationOperationIdSchema,
	presentationId: TerminalPresentationIdSchema,
} as const;

const terminalExitSchema = z
	.strictObject({
		exitCode: z.number().int().min(0).max(0xffff_ffff).nullable(),
		presentationId: TerminalPresentationIdSchema,
		signal: z.string().min(1).max(128).nullable(),
		type: z.literal('terminal-exited'),
	})
	.refine((event) => event.exitCode === null || event.signal === null, {
		message: 'A terminal exit cannot contain both an exit code and a signal',
	});

export const DesktopTerminalPresentationEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...operationIdentitySchema,
		type: z.literal('terminal-provision-completed'),
	}),
	z.strictObject({
		...operationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-provision-failed'),
	}),
	z.strictObject({
		...operationIdentitySchema,
		type: z.literal('terminal-restore-started'),
	}),
	z.strictObject({
		...operationIdentitySchema,
		type: z.literal('terminal-restore-completed'),
	}),
	z.strictObject({
		...operationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-restore-failed'),
	}),
	z.strictObject({
		...operationIdentitySchema,
		type: z.literal('terminal-close-completed'),
	}),
	z.strictObject({
		...operationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-close-failed'),
	}),
	terminalExitSchema,
]) satisfies z.ZodType<TerminalPresentationEvent>;
