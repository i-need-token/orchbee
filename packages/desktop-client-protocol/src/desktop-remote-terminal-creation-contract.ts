import { WorkbenchSharedDocumentRevisionSchema } from '@orchbee/workbench/document';
import { z } from 'zod';

export const DesktopRemoteTerminalCreationRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopRemoteTerminalCreationRequestId'>();

const workbenchIdentifierSchema = z.string().min(1).max(32_768);

export const DesktopRemoteTerminalCreationRendererCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		projectId: workbenchIdentifierSchema,
		requestId: DesktopRemoteTerminalCreationRequestIdSchema,
		type: z.literal('desktop-remote-terminal-creation-requested'),
		workspaceId: workbenchIdentifierSchema,
	}),
	z.strictObject({
		requestId: DesktopRemoteTerminalCreationRequestIdSchema,
		type: z.literal('desktop-remote-terminal-creation-cancel-requested'),
	}),
]);

export const DesktopRemoteTerminalCreationFailureReasonSchema = z.enum([
	'busy',
	'control-unavailable',
	'environment-not-connected',
	'environment-not-found',
	'project-not-found',
	'registry-unavailable',
	'remote-context-invalid',
	'terminal-host-busy',
	'terminal-host-compatibility-unsupported',
	'terminal-host-unavailable',
	'workbench-missing',
	'workbench-unavailable',
	'workspace-environment-mismatch',
	'workspace-not-found',
]);

const settledTerminalSchema = z.strictObject({
	kind: z.enum(['created', 'existing']),
	panelId: workbenchIdentifierSchema,
	presentationId: workbenchIdentifierSchema,
	projectId: workbenchIdentifierSchema,
	revision: WorkbenchSharedDocumentRevisionSchema,
	tabId: workbenchIdentifierSchema,
	terminalContextId: workbenchIdentifierSchema,
	workspaceId: workbenchIdentifierSchema,
});

export const DesktopRemoteTerminalCreationResultSchema = z.discriminatedUnion('kind', [
	settledTerminalSchema,
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('failed'),
		reason: DesktopRemoteTerminalCreationFailureReasonSchema,
	}),
	z.strictObject({
		kind: z.literal('indeterminate'),
		reason: z.enum(['control-unavailable', 'durability-unconfirmed']),
	}),
]);

export const DesktopRemoteTerminalCreationMainEventSchema = z.strictObject({
	requestId: DesktopRemoteTerminalCreationRequestIdSchema,
	result: DesktopRemoteTerminalCreationResultSchema,
	type: z.literal('desktop-remote-terminal-creation-settled'),
});

export type DesktopRemoteTerminalCreationFailureReason = z.infer<
	typeof DesktopRemoteTerminalCreationFailureReasonSchema
>;
export type DesktopRemoteTerminalCreationMainEvent = z.infer<
	typeof DesktopRemoteTerminalCreationMainEventSchema
>;
export type DesktopRemoteTerminalCreationRendererCommand = z.infer<
	typeof DesktopRemoteTerminalCreationRendererCommandSchema
>;
export type DesktopRemoteTerminalCreationRequestId = z.infer<
	typeof DesktopRemoteTerminalCreationRequestIdSchema
>;
export type DesktopRemoteTerminalCreationResult = z.infer<
	typeof DesktopRemoteTerminalCreationResultSchema
>;
