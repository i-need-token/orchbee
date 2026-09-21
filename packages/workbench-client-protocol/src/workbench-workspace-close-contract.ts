import { z } from 'zod';

import {
	SiteIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	TerminalPresentationIdSchema,
	WorkbenchPanelIdSchema,
	WorkbenchProjectIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
} from './workbench-semantic-contract';
import {
	WorkbenchTabCloseAttemptGenerationSchema,
	WorkbenchTabCloseBlockedReasonSchema,
	WorkbenchTabClosePhaseSchema,
	WorkbenchTabClosePublicationReasonSchema,
	WorkbenchTabCloseResumeRejectionReasonSchema,
	WorkbenchTabCloseStatusSchema,
	WorkbenchTabCloseUnavailableReasonSchema,
	WorkbenchTabCloseUnknownPhaseSchema,
	WorkbenchTabCloseUnknownReasonSchema,
} from './workbench-tab-close-contract';

/// Exact encoded bound of a `workbench.workspace-close.result` document whose
/// `available` outcome carries the protocol's maximum captured membership
/// (4096 member entries at their longest legal status spelling). The Rust
/// protocol asserts equality against the constructed worst document.
export const WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES = 1_000_054;
export const WORKBENCH_MAXIMUM_WORKSPACE_CLOSE_MEMBERS = 4096;

/// A member entry carries only its own terminal disposition; the close
/// revision lives solely in the scope-level status.
export const WorkbenchWorkspaceCloseMemberStatusSchema = z.discriminatedUnion('kind', [
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
]);

export const WorkbenchWorkspaceCloseMemberSchema = z
	.strictObject({
		tabId: WorkbenchTabIdSchema,
		panelId: WorkbenchPanelIdSchema,
		presentationId: TerminalPresentationIdSchema,
		status: WorkbenchWorkspaceCloseMemberStatusSchema,
	})
	.readonly();

export const WorkbenchWorkspaceCloseCountsSchema = z
	.strictObject({
		closing: z.number().int().min(0).max(4_294_967_295),
		cleanupBlocked: z.number().int().min(0).max(4_294_967_295),
		outcomeUnknown: z.number().int().min(0).max(4_294_967_295),
		disposedAwaitingPublication: z.number().int().min(0).max(4_294_967_295),
	})
	.readonly();

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
};
const scope = {
	siteId: SiteIdSchema,
	projectId: WorkbenchProjectIdSchema,
	workspaceId: WorkbenchWorkspaceIdSchema,
};
const deniedSchema = z.strictObject({ kind: z.literal('denied') }).readonly();
const unavailableSchema = z
	.strictObject({
		kind: z.literal('unavailable'),
		reason: WorkbenchTabCloseUnavailableReasonSchema,
	})
	.readonly();

export const WorkbenchWorkspaceCloseReadSchema = z
	.strictObject({ kind: z.literal('workbench.workspace-close.read'), ...correlation, ...scope })
	.readonly();

export const WorkbenchWorkspaceCloseReadOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('not-closing') }).readonly(),
	deniedSchema,
	unavailableSchema,
	z
		.strictObject({
			kind: z.literal('available'),
			closeRequestId: WorkbenchRequestIdSchema,
			projectId: WorkbenchProjectIdSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			attemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
			status: WorkbenchTabCloseStatusSchema,
			counts: WorkbenchWorkspaceCloseCountsSchema,
			members: z
				.array(WorkbenchWorkspaceCloseMemberSchema)
				.max(WORKBENCH_MAXIMUM_WORKSPACE_CLOSE_MEMBERS)
				.readonly(),
		})
		.readonly(),
]);

export const WorkbenchWorkspaceCloseReadResultSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-close.result'),
		...correlation,
		outcome: WorkbenchWorkspaceCloseReadOutcomeSchema,
	})
	.readonly();

export const WorkbenchWorkspaceCloseResumeSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-close.resume'),
		...correlation,
		...scope,
		closeRequestId: WorkbenchRequestIdSchema,
		expectedAttemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
	})
	.readonly();

export const WorkbenchWorkspaceCloseResumeOutcomeSchema = z.discriminatedUnion('kind', [
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

export const WorkbenchWorkspaceCloseResumeResultSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-close.resume.result'),
		...correlation,
		...scope,
		closeRequestId: WorkbenchRequestIdSchema,
		outcome: WorkbenchWorkspaceCloseResumeOutcomeSchema,
	})
	.readonly();

export type WorkbenchWorkspaceCloseMemberStatus = z.infer<
	typeof WorkbenchWorkspaceCloseMemberStatusSchema
>;
export type WorkbenchWorkspaceCloseMember = z.infer<typeof WorkbenchWorkspaceCloseMemberSchema>;
export type WorkbenchWorkspaceCloseCounts = z.infer<typeof WorkbenchWorkspaceCloseCountsSchema>;
export type WorkbenchWorkspaceCloseRead = z.infer<typeof WorkbenchWorkspaceCloseReadSchema>;
export type WorkbenchWorkspaceCloseReadOutcome = z.infer<
	typeof WorkbenchWorkspaceCloseReadOutcomeSchema
>;
export type WorkbenchWorkspaceCloseReadResult = z.infer<
	typeof WorkbenchWorkspaceCloseReadResultSchema
>;
export type WorkbenchWorkspaceCloseResume = z.infer<typeof WorkbenchWorkspaceCloseResumeSchema>;
export type WorkbenchWorkspaceCloseResumeOutcome = z.infer<
	typeof WorkbenchWorkspaceCloseResumeOutcomeSchema
>;
export type WorkbenchWorkspaceCloseResumeResult = z.infer<
	typeof WorkbenchWorkspaceCloseResumeResultSchema
>;
