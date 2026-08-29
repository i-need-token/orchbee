import { z } from 'zod';

import {
	AgentAttentionRequestSchema,
	AgentEventIdSchema,
	AgentRequestIdSchema,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
	AgentTurnOutcomeSchema,
	type AgentActivity,
	type AgentAttentionRequest,
	type AgentCapability,
	type AgentEventId,
	type AgentExecutionLocation,
	type AgentObservation,
	type AgentPresentationAttachment,
	type AgentPresentationVisibility,
	type AgentProviderId,
	type AgentRequestId,
	type AgentSessionId,
	type AgentTurnId,
	type AgentTurnOutcome,
	type AgentTurnState,
} from './model';

interface AgentSessionEventBase {
	readonly eventId: AgentEventId;
	readonly generation: number;
	readonly sequence: number;
	readonly sessionId: AgentSessionId;
}

export type AgentObservationEvent =
	| {
			readonly capabilities: readonly AgentCapability[];
			readonly eventId: AgentEventId;
			readonly executionLocation: AgentExecutionLocation;
			readonly observation: AgentObservation;
			readonly parentSessionId: AgentSessionId | null;
			readonly presentationAttachments: readonly AgentPresentationAttachment[];
			readonly providerId: AgentProviderId;
			readonly sessionId: AgentSessionId;
			readonly type: 'session-observed';
	  }
	| (AgentSessionEventBase & {
			readonly activity: AgentActivity | null;
			readonly phase: 'queued' | 'working';
			readonly turnId: AgentTurnId;
			readonly type: 'turn-started';
	  })
	| (AgentSessionEventBase & {
			readonly activity: AgentActivity | null;
			readonly phase: 'working' | 'waiting-external';
			readonly turnId: AgentTurnId;
			readonly type: 'turn-activity-changed';
	  })
	| (AgentSessionEventBase & {
			readonly activity: AgentActivity | null;
			readonly type: 'limited-activity-observed';
	  })
	| (AgentSessionEventBase & {
			readonly request: AgentAttentionRequest;
			readonly type: 'attention-requested';
	  })
	| (AgentSessionEventBase & {
			readonly requestId: AgentRequestId;
			readonly type: 'attention-resolved';
	  })
	| (AgentSessionEventBase & {
			readonly outcome: AgentTurnOutcome['kind'];
			readonly turnId: AgentTurnId;
			readonly type: 'turn-finished';
	  })
	| (AgentSessionEventBase & {
			readonly type: 'session-idle-observed';
	  })
	| (AgentSessionEventBase & {
			readonly type: 'observation-became-stale';
	  })
	| {
			readonly activityHint: AgentActivity | null;
			readonly capabilities: readonly AgentCapability[];
			readonly eventId: AgentEventId;
			readonly generation: number;
			readonly lastOutcome: AgentTurnOutcome | null;
			readonly pendingRequests: readonly AgentAttentionRequest[];
			readonly sequence: number;
			readonly sessionId: AgentSessionId;
			readonly turn: AgentTurnState;
			readonly type: 'observation-reconnected';
	  }
	| (AgentSessionEventBase & {
			readonly attachment: AgentPresentationAttachment;
			readonly type: 'presentation-attached';
	  })
	| (AgentSessionEventBase & {
			readonly presentationGeneration: string;
			readonly presentationId: string;
			readonly type: 'presentation-visibility-changed';
			readonly visibility: AgentPresentationVisibility;
	  })
	| (AgentSessionEventBase & {
			readonly presentationGeneration: string;
			readonly presentationId: string;
			readonly type: 'presentation-detached';
	  })
	| (AgentSessionEventBase & {
			readonly type: 'session-ended';
	  });

export type AgentSemanticEvent =
	| {
			readonly eventId: AgentEventId;
			readonly request: AgentAttentionRequest;
			readonly sessionId: AgentSessionId;
			readonly type: 'attention-requested';
	  }
	| {
			readonly eventId: AgentEventId;
			readonly requestId: AgentRequestId;
			readonly sessionId: AgentSessionId;
			readonly type: 'attention-resolved';
	  }
	| {
			readonly eventId: AgentEventId;
			readonly outcome: AgentTurnOutcome['kind'];
			readonly sessionId: AgentSessionId;
			readonly turnId: AgentTurnId;
			readonly type: 'turn-outcome';
	  }
	| {
			readonly eventId: AgentEventId;
			readonly sessionId: AgentSessionId;
			readonly type: 'session-ended';
	  };

export const AgentSemanticEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		eventId: AgentEventIdSchema,
		request: AgentAttentionRequestSchema,
		sessionId: AgentSessionIdSchema,
		type: z.literal('attention-requested'),
	}),
	z.strictObject({
		eventId: AgentEventIdSchema,
		requestId: AgentRequestIdSchema,
		sessionId: AgentSessionIdSchema,
		type: z.literal('attention-resolved'),
	}),
	z.strictObject({
		eventId: AgentEventIdSchema,
		outcome: AgentTurnOutcomeSchema.shape.kind,
		sessionId: AgentSessionIdSchema,
		turnId: AgentTurnIdSchema,
		type: z.literal('turn-outcome'),
	}),
	z.strictObject({
		eventId: AgentEventIdSchema,
		sessionId: AgentSessionIdSchema,
		type: z.literal('session-ended'),
	}),
]);
