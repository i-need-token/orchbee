import {
	WorkbenchSharedDocumentRevisionSchema,
	WorkbenchSharedDocumentSchema,
} from '@orchbee/workbench/document';
import {
	TerminalPresentationEventSchema,
	TerminalPresentationIdSchema,
} from '@orchbee/workbench/terminal-presentation';
import { CanonicalUint64TextSchema, TerminalIdentitySchema } from '@orchbee/terminal-protocol';
import { z } from 'zod';

export const WORKBENCH_CLIENT_SESSION_BOOTSTRAP = 'orchbee.workbench.client-session-bootstrap';

const WorkbenchClientSessionIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'WorkbenchClientSessionId'>();

export const WorkbenchClientSessionRequestIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9:._-]+$/u)
	.brand<'WorkbenchClientSessionRequestId'>();

const WorkbenchClientSessionSequenceSchema = z
	.string()
	.regex(/^(?:0|[1-9][0-9]*)$/u)
	.brand<'WorkbenchClientSessionSequence'>();

const WorkbenchTerminalMetadataSchema = z.strictObject({
	currentDirectory: z.string().nullable(),
	identity: TerminalIdentitySchema,
	presentationId: TerminalPresentationIdSchema,
	revision: CanonicalUint64TextSchema.refine((value) => value !== '0'),
	title: z.string().nullable(),
});

const sharedDocumentStateSchema = z.discriminatedUnion('kind', [
	z.strictObject({ document: WorkbenchSharedDocumentSchema, kind: z.literal('available') }),
	z.strictObject({ kind: z.literal('missing') }),
]);

export const WorkbenchClientSessionBootstrapSchema = z.strictObject({
	clientSessionId: WorkbenchClientSessionIdSchema,
	defaultCwd: z.string().min(1),
	documentState: sharedDocumentStateSchema,
	initialPresentationId: TerminalPresentationIdSchema,
	initializationDocument: WorkbenchSharedDocumentSchema,
	sequence: WorkbenchClientSessionSequenceSchema,
	terminalHostName: z.string().min(1).max(253),
	terminalMetadata: z.array(WorkbenchTerminalMetadataSchema).default([]),
	terminalEvents: z.array(TerminalPresentationEventSchema),
	type: z.literal(WORKBENCH_CLIENT_SESSION_BOOTSTRAP),
});

export const WorkbenchClientSessionCommandSchema = z.discriminatedUnion('type', [
	z.strictObject({
		document: WorkbenchSharedDocumentSchema,
		expectedRevision: WorkbenchSharedDocumentRevisionSchema.nullable(),
		requestId: WorkbenchClientSessionRequestIdSchema,
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

export const WorkbenchClientSessionEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		requestId: WorkbenchClientSessionRequestIdSchema,
		result: documentProposalResultSchema,
		type: z.literal('shared-document-proposal-completed'),
	}),
	z.strictObject({
		document: WorkbenchSharedDocumentSchema,
		sequence: WorkbenchClientSessionSequenceSchema,
		type: z.literal('shared-document-published'),
	}),
	z.strictObject({
		event: TerminalPresentationEventSchema,
		sequence: WorkbenchClientSessionSequenceSchema,
		type: z.literal('terminal-presentation-event-published'),
	}),
	z.strictObject({
		metadata: WorkbenchTerminalMetadataSchema,
		sequence: WorkbenchClientSessionSequenceSchema,
		type: z.literal('terminal-metadata-published'),
	}),
	z.strictObject({
		reason: z.enum(['invalid', 'read-failed', 'write-failed']),
		sequence: WorkbenchClientSessionSequenceSchema,
		type: z.literal('client-session-coordinator-unavailable'),
	}),
]);

export type WorkbenchClientSessionBootstrap = z.infer<typeof WorkbenchClientSessionBootstrapSchema>;
export type WorkbenchClientSessionEvent = z.infer<typeof WorkbenchClientSessionEventSchema>;
export type WorkbenchClientSessionRequestId = z.infer<typeof WorkbenchClientSessionRequestIdSchema>;
export type WorkbenchDocumentProposalResult = z.infer<typeof documentProposalResultSchema>;
export type WorkbenchTerminalMetadata = z.infer<typeof WorkbenchTerminalMetadataSchema>;
