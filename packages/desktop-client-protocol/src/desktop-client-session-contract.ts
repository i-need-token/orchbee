import {
	WorkbenchSharedDocumentRevisionSchema,
	WorkbenchSharedDocumentSchema,
} from '@orchbee/workbench/document';
import {
	TerminalPresentationEventSchema,
	TerminalPresentationIdSchema,
} from '@orchbee/workbench/terminal-presentation';
import { z } from 'zod';

export const DESKTOP_CLIENT_SESSION_BOOTSTRAP = 'orchbee.desktop.client-session-bootstrap';

const DesktopClientSessionIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopClientSessionId'>();

export const DesktopClientSessionRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'DesktopClientSessionRequestId'>();

const DesktopClientSessionSequenceSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]*)$/u)
	.brand<'DesktopClientSessionSequence'>();

const sharedDocumentStateSchema = z.discriminatedUnion('kind', [
	z.strictObject({ document: WorkbenchSharedDocumentSchema, kind: z.literal('available') }),
	z.strictObject({ kind: z.literal('missing') }),
]);

export const DesktopClientSessionBootstrapSchema = z.strictObject({
	clientSessionId: DesktopClientSessionIdSchema,
	defaultCwd: z.string().min(1),
	documentState: sharedDocumentStateSchema,
	initialPresentationId: TerminalPresentationIdSchema,
	initializationDocument: WorkbenchSharedDocumentSchema,
	sequence: DesktopClientSessionSequenceSchema,
	terminalEvents: z.array(TerminalPresentationEventSchema),
	type: z.literal(DESKTOP_CLIENT_SESSION_BOOTSTRAP),
});

export const DesktopClientSessionCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		document: WorkbenchSharedDocumentSchema,
		expectedRevision: WorkbenchSharedDocumentRevisionSchema.nullable(),
		requestId: DesktopClientSessionRequestIdSchema,
		type: z.literal('shared-document-proposed'),
	}),
]);

const documentProposalResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		document: WorkbenchSharedDocumentSchema,
		kind: z.enum(['committed', 'converged', 'stale']),
	}),
	z.strictObject({
		document: WorkbenchSharedDocumentSchema.nullable(),
		kind: z.literal('rejected'),
		reason: z.enum([
			'application-quiescing',
			'control-unavailable',
			'exit-transition',
			'revision-sequence',
		]),
	}),
	z.strictObject({
		document: WorkbenchSharedDocumentSchema.nullable(),
		kind: z.literal('unavailable'),
		reason: z.enum(['invalid', 'read-failed', 'write-failed']),
	}),
]);

export const DesktopClientSessionEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: DesktopClientSessionRequestIdSchema,
		result: documentProposalResultSchema,
		type: z.literal('shared-document-proposal-completed'),
	}),
	z.strictObject({
		document: WorkbenchSharedDocumentSchema,
		sequence: DesktopClientSessionSequenceSchema,
		type: z.literal('shared-document-published'),
	}),
	z.strictObject({
		event: TerminalPresentationEventSchema,
		sequence: DesktopClientSessionSequenceSchema,
		type: z.literal('terminal-presentation-event-published'),
	}),
	z.strictObject({
		reason: z.enum(['invalid', 'read-failed', 'write-failed']),
		sequence: DesktopClientSessionSequenceSchema,
		type: z.literal('client-session-coordinator-unavailable'),
	}),
]);

export type DesktopClientSessionBootstrap = z.infer<typeof DesktopClientSessionBootstrapSchema>;
export type DesktopClientSessionEvent = z.infer<typeof DesktopClientSessionEventSchema>;
export type DesktopClientSessionRequestId = z.infer<typeof DesktopClientSessionRequestIdSchema>;
export type DesktopDocumentProposalResult = z.infer<typeof documentProposalResultSchema>;
