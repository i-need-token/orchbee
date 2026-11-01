import { z } from 'zod';

import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
} from './workbench-semantic-contract';

const MAXIMUM_UINT64_TEXT = '18446744073709551615';

export const WorkbenchTabCloseAttemptGenerationSchema = z
	.string()
	.regex(/^[1-9][0-9]{0,19}$/u)
	.refine(
		(value) =>
			value.length < MAXIMUM_UINT64_TEXT.length ||
			(value.length === MAXIMUM_UINT64_TEXT.length && value <= MAXIMUM_UINT64_TEXT),
		'Close attempt generation exceeds unsigned 64-bit range',
	)
	.brand<'WorkbenchTabCloseAttemptGeneration'>();

export const WorkbenchTabClosePhaseSchema = z.enum([
	'queued',
	'waiting-grace',
	'waiting-outcome',
	'disposing',
]);
export const WorkbenchTabCloseUnknownPhaseSchema = z.enum(['terminate', 'force', 'dispose']);
export const WorkbenchTabCloseBlockedReasonSchema = z.enum([
	'worker-unavailable',
	'host-refused',
	'cleanup-incomplete',
	'inventory-incomplete',
	'identity-conflict',
	'continuation-budget-exhausted',
	'resume-generation-exhausted',
]);
export const WorkbenchTabCloseUnknownReasonSchema = z.enum([
	'transport-lost',
	'deadline',
	'invalid-response',
]);
export const WorkbenchTabClosePublicationReasonSchema = z.enum([
	'target-cleanup-pending',
	'preparing',
	'preparation-failed',
	'authority-retired',
	'workspace-retained',
]);
export const WorkbenchTabCloseUnavailableReasonSchema = z.enum(['server-busy']);
export const WorkbenchTabCloseResumeRejectionReasonSchema = z.enum([
	'operation-unavailable',
	'operation-mismatch',
	'request-identity-conflict',
	'generation-mismatch',
	'not-resumable',
	'generation-exhausted',
]);

export const WorkbenchTabCloseStatusSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('closing'), phase: WorkbenchTabClosePhaseSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('cleanup-blocked'),
			reason: WorkbenchTabCloseBlockedReasonSchema,
			resumable: z.boolean(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('outcome-unknown'),
			phase: WorkbenchTabCloseUnknownPhaseSchema,
			reason: WorkbenchTabCloseUnknownReasonSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('disposed-awaiting-publication'),
			reason: WorkbenchTabClosePublicationReasonSchema,
			resumable: z.boolean(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('closed'), revision: WorkbenchRevisionSchema }).readonly(),
]);

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
};
const target = {
	sessionId: SessionIdSchema,
	terminalContextId: WorkbenchTerminalContextIdSchema,
	tabId: WorkbenchTabIdSchema,
};
const deniedSchema = z.strictObject({ kind: z.literal('denied') }).readonly();
const unavailableSchema = z
	.strictObject({
		kind: z.literal('unavailable'),
		reason: WorkbenchTabCloseUnavailableReasonSchema,
	})
	.readonly();

export const WorkbenchTabCloseReadSchema = z
	.strictObject({ kind: z.literal('workbench.tab-close.read'), ...correlation, ...target })
	.readonly();

const WorkbenchTabCloseReadOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('not-closing') }).readonly(),
	deniedSchema,
	unavailableSchema,
	z
		.strictObject({
			kind: z.literal('available'),
			closeRequestId: WorkbenchRequestIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabId: WorkbenchTabIdSchema,
			attemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
			status: WorkbenchTabCloseStatusSchema,
		})
		.readonly(),
]);

export const WorkbenchTabCloseReadResultSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-close.result'),
		...correlation,
		outcome: WorkbenchTabCloseReadOutcomeSchema,
	})
	.readonly();

export const WorkbenchTabCloseResumeSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-close.resume'),
		...correlation,
		...target,
		closeRequestId: WorkbenchRequestIdSchema,
		expectedAttemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
	})
	.readonly();

const WorkbenchTabCloseResumeOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('resumed'),
			attemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('stale'),
			attemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
		})
		.readonly(),
	deniedSchema,
	unavailableSchema,
	z
		.strictObject({
			kind: z.literal('rejected'),
			reason: WorkbenchTabCloseResumeRejectionReasonSchema,
		})
		.readonly(),
]);

export const WorkbenchTabCloseResumeResultSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-close.resume.result'),
		...correlation,
		...target,
		closeRequestId: WorkbenchRequestIdSchema,
		outcome: WorkbenchTabCloseResumeOutcomeSchema,
	})
	.readonly();

export type WorkbenchTabCloseResume = z.infer<typeof WorkbenchTabCloseResumeSchema>;

export type WorkbenchTabCloseResumeResult = z.infer<typeof WorkbenchTabCloseResumeResultSchema>;
