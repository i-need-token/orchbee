import { z } from 'zod';
import {
	WorkbenchSharedDocumentRevisionSchema,
	TerminalPresentationIdSchema,
} from '@orchbee/workbench-client-protocol';

export const DesktopApplicationExitRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopApplicationExitRequestId'>();

const exitReasonSchema = z.enum(['development-host-restart', 'update-restart', 'user-quit']);
const exitModeSchema = z.enum(['development-host-restart', 'preserve', 'stop']);
const exitChoiceSchema = z.enum(['cancel', 'development-host-restart', 'preserve', 'stop']);

const DesktopApplicationExitFailureSchema = z
	.strictObject({
		code: z.enum([
			'development-restart-plan-unavailable',
			'host-inventory-unavailable',
			'host-orphan-detected',
			'shared-document-control-unavailable',
			'workbench-document-write-unavailable',
			'terminal-restart-staging-failed',
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

const exitChoiceCommandSchema = z.union([
	z.strictObject({
		choice: z.literal('development-host-restart'),
		restartPresentationIds: z.array(TerminalPresentationIdSchema).readonly(),
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-choice'),
	}),
	z.strictObject({
		choice: z.enum(['cancel', 'preserve', 'stop']),
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-choice'),
	}),
]);

export const DesktopApplicationLifetimeRendererCommandSchema = z.union([
	exitChoiceCommandSchema,
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

const exitRequestedEventSchema = z.union([
	z.strictObject({
		reason: z.literal('development-host-restart'),
		restartPresentationIds: z.array(TerminalPresentationIdSchema).readonly().optional(),
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-requested'),
	}),
	z.strictObject({
		reason: z.enum(['update-restart', 'user-quit']),
		requestId: DesktopApplicationExitRequestIdSchema,
		type: z.literal('desktop-application-exit-requested'),
	}),
]);

export const DesktopApplicationLifetimeMainEventSchema = z.union([
	exitRequestedEventSchema,
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

export type DesktopApplicationExitChoice = z.infer<typeof exitChoiceSchema>;
export type DesktopApplicationExitFailure = z.infer<typeof DesktopApplicationExitFailureSchema>;
export type DesktopApplicationExitMode = z.infer<typeof exitModeSchema>;
export type DesktopApplicationExitProgress = z.infer<typeof exitProgressSchema>;
export type DesktopApplicationExitReason = z.infer<typeof exitReasonSchema>;
export type DesktopApplicationExitRequestId = z.infer<typeof DesktopApplicationExitRequestIdSchema>;
export type DesktopApplicationLifetimeMainEvent = z.infer<
	typeof DesktopApplicationLifetimeMainEventSchema
>;
export type DesktopApplicationLifetimeRendererCommand = z.infer<
	typeof DesktopApplicationLifetimeRendererCommandSchema
>;
