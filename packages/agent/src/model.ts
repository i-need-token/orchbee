import { z } from 'zod';

const opaqueIdentitySchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9._:-]+$/u);

export const AgentSessionIdSchema = opaqueIdentitySchema.brand<'AgentSessionId'>();
export const AgentTurnIdSchema = opaqueIdentitySchema.brand<'AgentTurnId'>();
export const AgentRequestIdSchema = opaqueIdentitySchema.brand<'AgentRequestId'>();
export const AgentEventIdSchema = opaqueIdentitySchema.brand<'AgentEventId'>();
export const AgentExecutionLocationIdSchema =
	opaqueIdentitySchema.brand<'AgentExecutionLocationId'>();
export const AgentProviderIdSchema = z
	.string()
	.min(1)
	.max(64)
	.regex(/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/u)
	.brand<'AgentProviderId'>();

export const AgentCapabilitySchema = z.enum([
	'sessionLifecycle',
	'turnLifecycle',
	'questions',
	'approvals',
	'authentication',
	'activity',
	'subagents',
	'progress',
]);

export const AgentActivitySchema = z.enum([
	'thinking',
	'tool',
	'editing',
	'command',
	'subagent',
	'compacting',
	'retrying',
	'other',
]);

export const AgentExecutionLocationSchema = z.strictObject({
	executionLocationId: AgentExecutionLocationIdSchema,
});

export const AgentPresentationVisibilitySchema = z.enum(['visible', 'hidden', 'unknown']);

export const AgentPresentationAttachmentSchema = z.strictObject({
	generation: opaqueIdentitySchema,
	navigable: z.boolean(),
	presentationId: z.string().min(1).max(256),
	visibility: AgentPresentationVisibilitySchema,
});

export const AgentObservationSchema = z.strictObject({
	fidelity: z.enum(['rich', 'basic', 'heuristic']),
	freshness: z.enum(['fresh', 'stale', 'disconnected']),
	generation: z.number().int().nonnegative().safe(),
	sequence: z.number().int().nonnegative().safe(),
	source: z.enum(['native', 'hook', 'terminal', 'heuristic']),
});

export const AgentTurnStateSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('unknown') }),
	z.strictObject({ kind: z.literal('idle') }),
	z.strictObject({
		activity: AgentActivitySchema.nullable(),
		kind: z.literal('active'),
		phase: z.enum(['queued', 'working', 'waiting-external']),
		turnId: AgentTurnIdSchema,
	}),
]);

export const AgentAttentionRequestSchema = z.strictObject({
	actionPath: z.literal('navigate-only'),
	kind: z.enum(['question', 'approval', 'authentication', 'unclassified']),
	requestId: AgentRequestIdSchema,
	turnId: AgentTurnIdSchema.nullable(),
});

export const AgentTurnOutcomeSchema = z.strictObject({
	eventId: AgentEventIdSchema,
	kind: z.enum(['succeeded', 'failed', 'canceled', 'interrupted']),
	turnId: AgentTurnIdSchema,
});

export const AgentSessionSnapshotSchema = z
	.strictObject({
		activityHint: AgentActivitySchema.nullable(),
		capabilities: z.array(AgentCapabilitySchema).max(10),
		executionLocation: AgentExecutionLocationSchema,
		lastOutcome: AgentTurnOutcomeSchema.nullable(),
		lifecycle: z.enum(['live', 'ended']),
		observation: AgentObservationSchema,
		parentSessionId: AgentSessionIdSchema.nullable(),
		pendingRequests: z.array(AgentAttentionRequestSchema).max(128),
		presentationAttachments: z.array(AgentPresentationAttachmentSchema).max(32),
		providerId: AgentProviderIdSchema,
		sessionId: AgentSessionIdSchema,
		turn: AgentTurnStateSchema,
	})
	.superRefine((snapshot, context) => {
		if (new Set(snapshot.capabilities).size !== snapshot.capabilities.length) {
			context.addIssue({
				code: 'custom',
				message: 'Agent capabilities must be unique',
				path: ['capabilities'],
			});
		}
		const attachmentKeys = snapshot.presentationAttachments.map(
			(attachment) => `${attachment.presentationId}\0${attachment.generation}`,
		);
		if (new Set(attachmentKeys).size !== attachmentKeys.length) {
			context.addIssue({
				code: 'custom',
				message: 'Agent presentation attachments must be unique',
				path: ['presentationAttachments'],
			});
		}
		const navigablePresentationIds = snapshot.presentationAttachments
			.filter((attachment) => attachment.navigable)
			.map((attachment) => attachment.presentationId);
		if (new Set(navigablePresentationIds).size !== navigablePresentationIds.length) {
			context.addIssue({
				code: 'custom',
				message: 'An Agent Session can have only one navigable generation per presentation',
				path: ['presentationAttachments'],
			});
		}
		if (
			snapshot.activityHint !== null &&
			(!snapshot.capabilities.includes('activity') || snapshot.observation.source !== 'terminal')
		) {
			context.addIssue({
				code: 'custom',
				message: 'An Agent activity hint requires terminal-scoped activity capability',
				path: ['activityHint'],
			});
		}
		const requestIds = snapshot.pendingRequests.map((request) => request.requestId);
		if (new Set(requestIds).size !== requestIds.length) {
			context.addIssue({
				code: 'custom',
				message: 'Agent request identities must be unique',
				path: ['pendingRequests'],
			});
		}
		if (
			snapshot.lifecycle === 'ended' &&
			(snapshot.activityHint !== null ||
				snapshot.turn.kind === 'active' ||
				snapshot.pendingRequests.length > 0)
		) {
			context.addIssue({
				code: 'custom',
				message: 'An ended Agent Session cannot retain active work, activity hints, or requests',
				path: ['lifecycle'],
			});
		}
		for (const [index, request] of snapshot.pendingRequests.entries()) {
			if (
				request.turnId !== null &&
				(snapshot.turn.kind !== 'active' || snapshot.turn.turnId !== request.turnId)
			) {
				context.addIssue({
					code: 'custom',
					message: 'A Turn-scoped request must belong to the active Turn',
					path: ['pendingRequests', index, 'turnId'],
				});
			}
		}
	});

export const AgentRuntimeSnapshotSchema = z
	.strictObject({
		revision: z.number().int().nonnegative().safe(),
		sessions: z.array(AgentSessionSnapshotSchema).max(512),
	})
	.superRefine((snapshot, context) => {
		const sessionIds = snapshot.sessions.map((session) => session.sessionId);
		if (new Set(sessionIds).size !== sessionIds.length) {
			context.addIssue({
				code: 'custom',
				message: 'Agent Session identities must be unique',
				path: ['sessions'],
			});
		}
		const sessions = new Map(snapshot.sessions.map((session) => [session.sessionId, session]));
		for (const [index, session] of snapshot.sessions.entries()) {
			if (session.parentSessionId !== null && !sessions.has(session.parentSessionId)) {
				context.addIssue({
					code: 'custom',
					message: 'An Agent Session parent must exist in the same snapshot',
					path: ['sessions', index, 'parentSessionId'],
				});
				continue;
			}
			const visited = new Set([session.sessionId]);
			let parentSessionId = session.parentSessionId;
			while (parentSessionId !== null) {
				if (visited.has(parentSessionId)) {
					context.addIssue({
						code: 'custom',
						message: 'Agent Session parentage must be acyclic',
						path: ['sessions', index, 'parentSessionId'],
					});
					break;
				}
				visited.add(parentSessionId);
				parentSessionId = sessions.get(parentSessionId)?.parentSessionId ?? null;
			}
		}
	});

export type AgentActivity = z.infer<typeof AgentActivitySchema>;
export type AgentAttentionRequest = z.infer<typeof AgentAttentionRequestSchema>;
export type AgentCapability = z.infer<typeof AgentCapabilitySchema>;
export type AgentEventId = z.infer<typeof AgentEventIdSchema>;
export type AgentExecutionLocation = z.infer<typeof AgentExecutionLocationSchema>;
export type AgentExecutionLocationId = z.infer<typeof AgentExecutionLocationIdSchema>;
export type AgentObservation = z.infer<typeof AgentObservationSchema>;
export type AgentPresentationAttachment = z.infer<typeof AgentPresentationAttachmentSchema>;
export type AgentPresentationVisibility = z.infer<typeof AgentPresentationVisibilitySchema>;
export type AgentProviderId = z.infer<typeof AgentProviderIdSchema>;
export type AgentRequestId = z.infer<typeof AgentRequestIdSchema>;
export type AgentRuntimeSnapshot = z.infer<typeof AgentRuntimeSnapshotSchema>;
export type AgentSessionId = z.infer<typeof AgentSessionIdSchema>;
export type AgentSessionSnapshot = z.infer<typeof AgentSessionSnapshotSchema>;
export type AgentTurnId = z.infer<typeof AgentTurnIdSchema>;
export type AgentTurnOutcome = z.infer<typeof AgentTurnOutcomeSchema>;
export type AgentTurnState = z.infer<typeof AgentTurnStateSchema>;
