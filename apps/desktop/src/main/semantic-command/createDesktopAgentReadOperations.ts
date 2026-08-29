import {
	AGENT_SESSION_CHANGES_OPERATION,
	AGENT_SESSION_EXPLAIN_OPERATION,
	AGENT_SESSION_GET_OPERATION,
	AGENT_SESSION_LIST_OPERATION,
	AGENT_SESSION_WAIT_OPERATION,
	AgentSessionChangesInputSchema,
	AgentSessionChangesPageSchema,
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
	type AgentChangeSource,
	type AgentRuntimeChangeReadResult,
	type AgentSessionChangesInput,
	type AgentSessionWaitInput,
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
	readonly source: AgentChangeSource;
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
		createAgentChangesOperation(options, cursor),
	];
}

const AGENT_CHANGES_CURSOR_REVISION = 'agent-session-changes-v1';
const AGENT_CHANGES_CURSOR_TARGET = 'all-agent-sessions';

function createAgentChangesOperation(
	options: DesktopAgentReadOperationOptions,
	cursor: DesktopSemanticCommandPageCursor,
): DesktopSemanticCommandOperation {
	const descriptor = SemanticCommandOperationDescriptorSchema.parse(
		AGENT_SESSION_CHANGES_OPERATION,
	);
	const operationId = SemanticCommandOperationIdSchema.parse(descriptor.id);
	const cursorContext = {
		coordinatorGeneration: options.coordinatorGeneration,
		operationId,
		revision: AGENT_CHANGES_CURSOR_REVISION,
		target: AGENT_CHANGES_CURSOR_TARGET,
	};
	return {
		authorize: localUser,
		canonicalizeInput: (input) => canonicalInput(AgentSessionChangesInputSchema.safeParse(input)),
		descriptor,
		execute: async (input, context) => {
			if (context.signal.aborted) return noEffect('cancelled', 'agent-changes-cancelled');
			const parsed = AgentSessionChangesInputSchema.safeParse(input);
			if (!parsed.success) return noEffect('failed', 'operation-input-invalid');
			if (parsed.data.cursor === undefined) {
				const revision = options.source.getSnapshot().revision;
				return completed('agent-changes-baselined', {
					hasMore: false,
					items: [],
					kind: 'baseline',
					nextCursor: cursor.create(cursorContext, revision),
					observedThroughRevision: revision,
					runtimeRevision: revision,
				});
			}
			const resolved = cursor.resolve(parsed.data.cursor, cursorContext);
			if (resolved.kind !== 'resolved') {
				return noEffect('blocked', 'agent-change-cursor-invalid');
			}
			return waitForAgentChanges(
				options,
				parsed.data,
				resolved.offset,
				context.signal,
				cursor,
				cursorContext,
			);
		},
		isVisibleTo: localUser,
		validateOutput: (output) => AgentSessionChangesPageSchema.safeParse(output).success,
	};
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

type AgentChangesCursorContext = Parameters<DesktopSemanticCommandPageCursor['create']>[0];

function waitForAgentChanges(
	options: DesktopAgentReadOperationOptions,
	input: AgentSessionChangesInput,
	afterRevision: number,
	signal: AbortSignal,
	cursor: DesktopSemanticCommandPageCursor,
	cursorContext: AgentChangesCursorContext,
): Promise<DesktopSemanticCommandHandlerSettlement> {
	const initial = options.source.readChanges({ afterRevision, limit: input.limit });
	if (initial.kind !== 'current') {
		return Promise.resolve(agentChangesSettlement(initial, cursor, cursorContext));
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
			const result = options.source.readChanges({ afterRevision, limit: input.limit });
			if (result.kind === 'current' && !timedOut) return;
			finish(agentChangesSettlement(result, cursor, cursorContext));
		};
		const handleAbort = (): void => finish(noEffect('cancelled', 'agent-changes-cancelled'));

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

function agentChangesSettlement(
	result: AgentRuntimeChangeReadResult,
	cursor: DesktopSemanticCommandPageCursor,
	cursorContext: AgentChangesCursorContext,
): DesktopSemanticCommandHandlerSettlement {
	if (result.kind === 'revision-ahead') {
		return noEffect('blocked', 'agent-change-cursor-invalid');
	}
	const nextCursor = cursor.create(cursorContext, result.observedThroughRevision);
	switch (result.kind) {
		case 'changes':
			return completed('agent-changes-read', {
				hasMore: result.hasMore,
				items: result.changes,
				kind: 'changes',
				nextCursor,
				observedThroughRevision: result.observedThroughRevision,
				runtimeRevision: result.runtimeRevision,
			});
		case 'current':
			return completed('agent-changes-current', {
				hasMore: false,
				items: [],
				kind: 'current',
				nextCursor,
				observedThroughRevision: result.observedThroughRevision,
				runtimeRevision: result.runtimeRevision,
			});
		case 'reset-required':
			return completed('agent-changes-reset-required', {
				hasMore: false,
				items: [],
				kind: 'reset-required',
				nextCursor,
				observedThroughRevision: result.observedThroughRevision,
				runtimeRevision: result.runtimeRevision,
			});
	}
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
