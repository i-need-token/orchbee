import { WorkbenchSharedDocumentRevisionSchema } from '@orchbee/workbench/document';
import { z } from 'zod';

export const DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL = 'orchbee.desktop.application-lifetime-port';
export const DESKTOP_APPLICATION_LIFETIME_WINDOW_MESSAGE =
	'orchbee.desktop.application-lifetime-ready';

export const DesktopApplicationExitRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopApplicationExitRequestId'>();

const exitReasonSchema = z.enum(['update-restart', 'user-quit']);
const exitModeSchema = z.enum(['preserve', 'stop']);
const exitChoiceSchema = z.enum(['cancel', 'preserve', 'stop']);

const DesktopApplicationExitFailureSchema = z
	.strictObject({
		code: z.enum([
			'host-inventory-unavailable',
			'host-orphan-detected',
			'shared-document-control-unavailable',
			'workbench-document-write-unavailable',
			'terminal-stop-failed',
			'unreferenced-binding-close-failed',
		]),
		count: z.number().int().positive(),
		retryable: z.boolean(),
	})
	.readonly();

const exitProgressSchema = z
	.strictObject({
		completedCount: z.number().int().nonnegative(),
		failedCount: z.number().int().nonnegative(),
		phase: z.enum([
			'awaiting-choice',
			'draining-unreferenced',
			'failed',
			'flushing-workbench-document',
			'stopping-terminals',
			'verifying-host',
		]),
		totalCount: z.number().int().nonnegative(),
	})
	.readonly();

export const DesktopApplicationLifetimeRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		choice: exitChoiceSchema,
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-choice'),
	}),
	z.strictObject({
		failures: z.array(DesktopApplicationExitFailureSchema),
		progress: exitProgressSchema,
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-progress'),
	}),
	z.strictObject({
		mode: exitModeSchema,
		presentationCount: z.number().int().nonnegative(),
		requestId: DesktopApplicationExitRequestIdSchema,
		documentRevision: WorkbenchSharedDocumentRevisionSchema,
		type: z.literal('desktop-application-exit-renderer-prepared'),
	}),
	z.strictObject({
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-cancelled'),
	}),
	z.strictObject({
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-main-retry-requested'),
	}),
	z.strictObject({
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-quit-anyway-requested'),
	}),
]);

const mainExitResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		closedUnreferencedCount: z.number().int().nonnegative(),
		kind: z.literal('completed'),
	}),
	z.strictObject({
		failures: z.array(DesktopApplicationExitFailureSchema).min(1),
		kind: z.literal('failed'),
	}),
]);

export const DesktopApplicationLifetimeMainEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		reason: exitReasonSchema,
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-requested'),
	}),
	z.strictObject({
		mode: exitModeSchema,
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-mode-accepted'),
	}),
	z.strictObject({
		requestId: DesktopApplicationExitRequestIdSchema,
		result: mainExitResultSchema,
		type: z.literal('desktop-application-exit-main-completed'),
	}),
]);

export const DesktopApplicationLifetimeBootstrapSchema = z.strictObject({
	type: z.literal(DESKTOP_APPLICATION_LIFETIME_IPC_CHANNEL),
});

export const DesktopApplicationLifetimeWindowMessageSchema = z.strictObject({
	security: z.strictObject({ contextIsolated: z.boolean(), sandboxed: z.boolean() }),
	type: z.literal(DESKTOP_APPLICATION_LIFETIME_WINDOW_MESSAGE),
});

export type DesktopApplicationExitChoice = z.infer<typeof exitChoiceSchema>;
export type DesktopApplicationExitFailure = z.infer<typeof DesktopApplicationExitFailureSchema>;
export type DesktopApplicationExitProgress = z.infer<typeof exitProgressSchema>;
export type DesktopApplicationExitReason = z.infer<typeof exitReasonSchema>;
export type DesktopApplicationExitRequestId = z.infer<typeof DesktopApplicationExitRequestIdSchema>;
export type DesktopApplicationLifetimeMainEvent = z.infer<
	typeof DesktopApplicationLifetimeMainEventSchema
>;
export type DesktopApplicationLifetimeRendererCommand = z.infer<
	typeof DesktopApplicationLifetimeRendererCommandSchema
>;
