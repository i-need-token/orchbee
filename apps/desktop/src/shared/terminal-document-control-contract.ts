import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import {
	TerminalPresentationFailureSchema,
	TerminalPresentationIdSchema,
	TerminalPresentationOperationIdSchema,
} from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

export const TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL =
	'orchbee.desktop.terminal-document-control-port';
export const TERMINAL_DOCUMENT_CONTROL_WINDOW_MESSAGE =
	'orchbee.desktop.terminal-document-control-ready';

const boundedLaunchCwdSchema = z
	.string()
	.min(1)
	.max(PROTOCOL_LIMITS.maxStringBytes)
	.refine(
		(value) => new TextEncoder().encode(value).byteLength <= PROTOCOL_LIMITS.maxStringBytes,
		`Launch CWD exceeds ${PROTOCOL_LIMITS.maxStringBytes} UTF-8 bytes`,
	);

export const TerminalDocumentControlCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		launchCwd: boundedLaunchCwdSchema.nullable(),
		operationId: TerminalPresentationOperationIdSchema,
		presentationId: TerminalPresentationIdSchema,
		type: z.literal('terminal-provision-requested'),
	}),
	z.strictObject({
		operationId: TerminalPresentationOperationIdSchema,
		presentationId: TerminalPresentationIdSchema,
		type: z.literal('terminal-close-requested'),
	}),
]);

export const TerminalDocumentControlBootstrapSchema = z.strictObject({
	defaultCwd: boundedLaunchCwdSchema,
	initialPresentationId: TerminalPresentationIdSchema,
	presentations: z.array(
		z.strictObject({
			launchCwd: boundedLaunchCwdSchema.nullable(),
			presentationId: TerminalPresentationIdSchema,
		}),
	),
	type: z.literal(TERMINAL_DOCUMENT_CONTROL_IPC_CHANNEL),
});

const terminalDocumentOperationIdentitySchema = {
	operationId: TerminalPresentationOperationIdSchema,
	presentationId: TerminalPresentationIdSchema,
} as const;

const TerminalDocumentExitSchema = z
	.strictObject({
		exitCode: z.number().int().min(0).max(0xffff_ffff).nullable(),
		presentationId: TerminalPresentationIdSchema,
		signal: z.string().min(1).max(128).nullable(),
		type: z.literal('terminal-exited'),
	})
	.refine((event) => event.exitCode === null || event.signal === null, {
		message: 'A terminal exit cannot contain both an exit code and a signal',
	});

export const TerminalDocumentControlEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...terminalDocumentOperationIdentitySchema,
		type: z.literal('terminal-provision-completed'),
	}),
	z.strictObject({
		...terminalDocumentOperationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-provision-failed'),
	}),
	z.strictObject({
		...terminalDocumentOperationIdentitySchema,
		type: z.literal('terminal-close-completed'),
	}),
	z.strictObject({
		...terminalDocumentOperationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-close-failed'),
	}),
	TerminalDocumentExitSchema,
]);

export const TerminalDocumentControlWindowMessageSchema = z.strictObject({
	bootstrap: TerminalDocumentControlBootstrapSchema,
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(TERMINAL_DOCUMENT_CONTROL_WINDOW_MESSAGE),
});

export type TerminalDocumentControlCommand = z.infer<typeof TerminalDocumentControlCommandSchema>;
export type TerminalDocumentControlBootstrap = z.infer<
	typeof TerminalDocumentControlBootstrapSchema
>;
export type TerminalDocumentControlEvent = z.infer<typeof TerminalDocumentControlEventSchema>;
export type TerminalDocumentControlOperationResult = Exclude<
	TerminalDocumentControlEvent,
	{ readonly type: 'terminal-exited' }
>;
