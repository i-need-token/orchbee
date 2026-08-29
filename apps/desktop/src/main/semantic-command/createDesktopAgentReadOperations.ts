import {
	AGENT_SESSION_EXPLAIN_OPERATION,
	AGENT_SESSION_GET_OPERATION,
	AGENT_SESSION_LIST_OPERATION,
	AGENT_SESSION_WAIT_OPERATION,
	AgentSessionExplainOutputSchema,
	AgentSessionGetOutputSchema,
	AgentSessionListInputSchema,
	AgentSessionListPageSchema,
	AgentSessionTargetInputSchema,
	AgentSessionWaitInputSchema,
	AgentSessionWaitOutputSchema,
	explainAgentSession,
	findAgentSession,
	matchingAgentWaitConditions,
	projectAgentSessionDetail,
	projectAgentSessionSummary,
	type AgentSessionWaitInput,
	type AgentSnapshotSource,
} from '@orchbee/agent';
import {
	SemanticCommandJsonValueSchema,
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandResultCodeSchema,
	type SemanticCommandCoordinatorGeneration,
	type SemanticCommandJsonValue,
	type SemanticCommandTerminalResult,
} from '@orchbee/semantic-command-protocol';

import type {
	DesktopSemanticCommandHandlerSettlement,
	DesktopSemanticCommandOperation,
} from './DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandPageCursor } from './DesktopSemanticCommandPageCursor';

type ScheduleTimeout = (listener: () => void, milliseconds: number) => () => void;

export interface DesktopAgentReadOperationOptions {
	readonly coordinatorGeneration: SemanticCommandCoordinatorGeneration;
	readonly cursorSecret: Uint8Array;
	readonly scheduleTimeout?: ScheduleTimeout | undefined;
	readonly source: AgentSnapshotSource;
}

const defaultScheduleTimeout: ScheduleTimeout = (listener, milliseconds) => {
	const timer = setTimeout(listener, milliseconds);
	return () => clearTimeout(timer);
};

export function createDesktopAgentReadOperations(
	options: DesktopAgentReadOperationOptions,
): readonly DesktopSemanticCommandOperation[] {
	const cursor = new DesktopSemanticCommandPageCursor(options.cursorSecret);
	return [
		createAgentListOperation(options, cursor),
		createAgentGetOperation(options),
		createAgentExplainOperation(options),
		createAgentWaitOperation(options),
	];
}

function createAgentListOperation(
	options: DesktopAgentReadOperationOptions,
	cursor: DesktopSemanticCommandPageCursor,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(AGENT_SESSION_LIST_OPERATION);
	const operationId = SemanticCommandOperationIdSchema.parse(descriptor.id);
	return {
		authorize: localUser,
		canonicalizeInput: (input) => canonicalInput(AgentSessionListInputSchema.safeParse(input)),
		descriptor,
		execute: async (input, context) => {
			if (context.signal.aborted) return noEffect('cancelled', 'agent-query-cancelled');
			const parsed = AgentSessionListInputSchema.safeParse(input);
			if (!parsed.success) return noEffect('failed', 'operation-input-invalid');
			const snapshot = options.source.getSnapshot();
			const sessions = snapshot.sessions.filter(
				(session) => parsed.data.includeEnded || session.lifecycle === 'live',
			);
			const cursorContext = {
				coordinatorGeneration: options.coordinatorGeneration,
				operationId,
				revision: String(snapshot.revision),
				target: `include-ended:${String(parsed.data.includeEnded)}`,
			};
			const resolved = cursor.resolve(parsed.data.cursor, cursorContext);
			if (resolved.kind === 'invalid') return noEffect('blocked', 'page-cursor-invalid');
			if (resolved.kind === 'stale') return noEffect('blocked', 'page-cursor-stale');
			if (resolved.offset > sessions.length) return noEffect('blocked', 'page-cursor-invalid');
			const end = Math.min(sessions.length, resolved.offset + parsed.data.limit);
			return completed('agent-sessions-listed', {
				items: sessions.slice(resolved.offset, end).map(projectAgentSessionSummary),
				nextCursor: end < sessions.length ? cursor.create(cursorContext, end) : null,
				revision: snapshot.revision,
			});
		},
		isVisibleTo: localUser,
		validateOutput: (output) => AgentSessionListPageSchema.safeParse(output).success,
	};
}

function createAgentGetOperation(
	options: DesktopAgentReadOperationOptions,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(AGENT_SESSION_GET_OPERATION);
	return {
		authorize: localUser,
		canonicalizeInput: (input) => canonicalInput(AgentSessionTargetInputSchema.safeParse(input)),
		descriptor,
		execute: async (input, context) => {
			if (context.signal.aborted) return noEffect('cancelled', 'agent-query-cancelled');
			const parsed = AgentSessionTargetInputSchema.safeParse(input);
			if (!parsed.success) return noEffect('failed', 'operation-input-invalid');
			const snapshot = options.source.getSnapshot();
			const session = findAgentSession(snapshot, parsed.data.sessionId);
			return session === null
				? noEffect('blocked', 'agent-session-not-found')
				: completed('agent-session-read', {
						revision: snapshot.revision,
						session: projectAgentSessionDetail(session),
					});
		},
		isVisibleTo: localUser,
		validateOutput: (output) => AgentSessionGetOutputSchema.safeParse(output).success,
	};
}

function createAgentExplainOperation(
	options: DesktopAgentReadOperationOptions,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(
		AGENT_SESSION_EXPLAIN_OPERATION,
	);
	return {
		authorize: localUser,
		canonicalizeInput: (input) => canonicalInput(AgentSessionTargetInputSchema.safeParse(input)),
		descriptor,
		execute: async (input, context) => {
			if (context.signal.aborted) return noEffect('cancelled', 'agent-query-cancelled');
			const parsed = AgentSessionTargetInputSchema.safeParse(input);
			if (!parsed.success) return noEffect('failed', 'operation-input-invalid');
			const snapshot = options.source.getSnapshot();
			const session = findAgentSession(snapshot, parsed.data.sessionId);
			return session === null
				? noEffect('blocked', 'agent-session-not-found')
				: completed('agent-session-explained', {
						explanation: explainAgentSession(session),
						revision: snapshot.revision,
						session: projectAgentSessionDetail(session),
					});
		},
		isVisibleTo: localUser,
		validateOutput: (output) => AgentSessionExplainOutputSchema.safeParse(output).success,
	};
}

function createAgentWaitOperation(
	options: DesktopAgentReadOperationOptions,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(AGENT_SESSION_WAIT_OPERATION);
	return {
		authorize: localUser,
		canonicalizeInput: (input) => canonicalInput(AgentSessionWaitInputSchema.safeParse(input)),
		descriptor,
		execute: async (input, context) => {
			if (context.signal.aborted) return noEffect('cancelled', 'agent-wait-cancelled');
			const parsed = AgentSessionWaitInputSchema.safeParse(input);
			if (!parsed.success) return noEffect('failed', 'operation-input-invalid');
			return waitForAgentSnapshot(options, parsed.data, context.signal);
		},
		isVisibleTo: localUser,
		validateOutput: (output) => AgentSessionWaitOutputSchema.safeParse(output).success,
	};
}

function waitForAgentSnapshot(
	options: DesktopAgentReadOperationOptions,
	input: AgentSessionWaitInput,
	signal: AbortSignal,
): Promise<DesktopSemanticCommandHandlerSettlement> {
	const initial = options.source.getSnapshot();
	if (findAgentSession(initial, input.sessionId) === null) {
		return Promise.resolve(noEffect('blocked', 'agent-session-not-found'));
	}
	if (input.afterRevision !== undefined && input.afterRevision > initial.revision) {
		return Promise.resolve(noEffect('blocked', 'agent-revision-ahead'));
	}

	return new Promise((resolve) => {
		let cancelTimeout = (): void => undefined;
		let settled = false;
		let unsubscribe = (): void => undefined;
		const finish = (result: DesktopSemanticCommandHandlerSettlement): void => {
			if (settled) return;
			settled = true;
			cancelTimeout();
			unsubscribe();
			signal.removeEventListener('abort', handleAbort);
			resolve(result);
		};
		const evaluate = (timedOut: boolean): void => {
			if (settled) return;
			const snapshot = options.source.getSnapshot();
			const session = findAgentSession(snapshot, input.sessionId);
			if (session === null) {
				finish(noEffect('blocked', 'agent-session-not-found'));
				return;
			}
			const revisionEligible =
				input.afterRevision === undefined || snapshot.revision > input.afterRevision;
			const matched = revisionEligible ? matchingAgentWaitConditions(session, input.until) : [];
			if (matched.length > 0) {
				finish(
					completed('agent-wait-matched', {
						matchedConditions: matched,
						revision: snapshot.revision,
						session: projectAgentSessionDetail(session),
					}),
				);
				return;
			}
			if (timedOut) finish(noEffect('blocked', 'agent-wait-timeout'));
		};
		const handleAbort = (): void => finish(noEffect('cancelled', 'agent-wait-cancelled'));

		signal.addEventListener('abort', handleAbort, { once: true });
		const stop = options.source.subscribe(() => evaluate(false));
		unsubscribe = stop;
		if (settled) {
			stop();
			return;
		}
		evaluate(false);
		if (settled) return;
		const stopTimeout = (options.scheduleTimeout ?? defaultScheduleTimeout)(
			() => evaluate(true),
			input.timeoutMilliseconds,
		);
		cancelTimeout = stopTimeout;
		if (settled) stopTimeout();
		if (signal.aborted) handleAbort();
	});
}

function canonicalInput(
	parsed: { readonly data: unknown; readonly success: true } | { readonly success: false },
): ReturnType<DesktopSemanticCommandOperation['canonicalizeInput']> {
	return parsed.success
		? { input: SemanticCommandJsonValueSchema.parse(parsed.data), kind: 'accepted' }
		: { kind: 'rejected' };
}

function localUser(caller: { readonly kind: string }): boolean {
	return caller.kind === 'local-user';
}

function completed(
	code: string,
	output: SemanticCommandJsonValue,
): DesktopSemanticCommandHandlerSettlement {
	return {
		effect: 'none',
		result: {
			code: SemanticCommandResultCodeSchema.parse(code),
			kind: 'completed',
			output: SemanticCommandJsonValueSchema.parse(output),
		},
	};
}

function noEffect(
	kind: Exclude<SemanticCommandTerminalResult['kind'], 'completed' | 'unchanged'>,
	code: string,
): DesktopSemanticCommandHandlerSettlement {
	return { effect: 'none', result: { code: SemanticCommandResultCodeSchema.parse(code), kind } };
}
