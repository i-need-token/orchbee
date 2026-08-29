import { describe, expect, it, vi } from 'vitest';

import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentRuntime,
	AgentSessionIdSchema,
	AgentTurnIdSchema,
	type AgentChangeSource,
	type AgentObservationEvent,
} from '@orchbee/agent';
import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandOperationIdSchema,
	type SemanticCommandInvocationRequest,
} from '@orchbee/semantic-command-protocol';

import { DesktopSemanticCommandCatalog } from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';
import {
	createDesktopAgentReadOperations,
	type DesktopAgentReadOperationOptions,
} from '../createDesktopAgentReadOperations';

const generation = SemanticCommandCoordinatorGenerationSchema.parse(
	'desktop:generation:agent-read',
);

describe('Desktop Agent read operation catalog', () => {
	it('admits exactly the five local-user Agent read operations', () => {
		const { service } = createService(runtimeWithWorkingSession());

		expect(service.list({ kind: 'local-user' }).operations.map((entry) => entry.id)).toEqual([
			'agent.session.changes',
			'agent.session.explain',
			'agent.session.get',
			'agent.session.list',
			'agent.session.wait',
		]);
	});

	it('baselines and follows ordered Agent changes through a signed cursor', async () => {
		const runtime = runtimeWithWorkingSession();
		const { service } = createService(runtime);
		const baseline = await invokeAndRead(service, request(1, 'agent.session.changes', {}));
		expect(baseline).toMatchObject({
			code: 'agent-changes-baselined',
			kind: 'completed',
			output: {
				hasMore: false,
				items: [],
				kind: 'baseline',
				observedThroughRevision: 2,
				runtimeRevision: 2,
			},
		});
		const cursor = resultOutputProperty(baseline, 'nextCursor');
		expect(cursor).toBeTypeOf('string');
		const follow = request(2, 'agent.session.changes', {
			cursor,
			timeoutMilliseconds: 1_000,
		});
		expect(service.invoke({ kind: 'local-user' }, follow)).toMatchObject({
			execution: { phase: 'active' },
			kind: 'accepted',
		});

		runtime.dispatch(finished('session-working', 'event-changes-finished', 3));

		expect(await waitForTerminal(service, follow)).toMatchObject({
			code: 'agent-changes-read',
			kind: 'completed',
			output: {
				hasMore: false,
				items: [
					{
						revision: 3,
						session: { sessionId: 'session-working', status: { kind: 'succeeded' } },
						transition: 'turn-finished',
					},
				],
				kind: 'changes',
				observedThroughRevision: 3,
				runtimeRevision: 3,
			},
		});
	});

	it('closes the change subscription race and completes a timeout as current', async () => {
		const runtime = runtimeWithWorkingSession();
		const unsubscribe = vi.fn<() => void>();
		let dispatchDuringSubscribe = true;
		const source: AgentChangeSource = {
			getSnapshot: runtime.getSnapshot,
			readChanges: runtime.readChanges,
			subscribe: (listener) => {
				const stop = runtime.subscribe(listener);
				if (dispatchDuringSubscribe) {
					dispatchDuringSubscribe = false;
					runtime.dispatch(finished('session-working', 'event-changes-race', 3));
				}
				return () => {
					stop();
					unsubscribe();
				};
			},
		};
		let expire: (() => void) | undefined;
		const { service } = createService(source, {
			scheduleTimeout: (listener) => {
				expire = listener;
				return () => undefined;
			},
		});
		const baseline = await invokeAndRead(service, request(1, 'agent.session.changes', {}));
		const baselineCursor = resultOutputProperty(baseline, 'nextCursor');
		const raced = await invokeAndRead(
			service,
			request(2, 'agent.session.changes', {
				cursor: baselineCursor,
				timeoutMilliseconds: 1_000,
			}),
		);
		expect(raced).toMatchObject({
			code: 'agent-changes-read',
			output: { items: [{ revision: 3 }] },
		});
		expect(unsubscribe).toHaveBeenCalledOnce();

		const currentCursor = resultOutputProperty(raced, 'nextCursor');
		const currentRequest = request(3, 'agent.session.changes', {
			cursor: currentCursor,
			timeoutMilliseconds: 1,
		});
		service.invoke({ kind: 'local-user' }, currentRequest);
		await vi.waitFor(() => expect(expire).toBeTypeOf('function'));
		expire?.();
		expect(await waitForTerminal(service, currentRequest)).toMatchObject({
			code: 'agent-changes-current',
			kind: 'completed',
			output: { items: [], kind: 'current', observedThroughRevision: 3 },
		});
	});

	it('rejects malformed change cursors and settles follow cancellation', async () => {
		const runtime = runtimeWithWorkingSession();
		const { service } = createService(runtime);
		expect(
			await invokeAndRead(
				service,
				request(1, 'agent.session.changes', { cursor: 'not-a-signed-cursor' }),
			),
		).toEqual({ code: 'agent-change-cursor-invalid', kind: 'blocked' });

		const baseline = await invokeAndRead(service, request(2, 'agent.session.changes', {}));
		const invocation = request(3, 'agent.session.changes', {
			cursor: resultOutputProperty(baseline, 'nextCursor'),
			timeoutMilliseconds: 1_000,
		});
		service.invoke({ kind: 'local-user' }, invocation);
		await vi.waitFor(() =>
			expect(
				service.get({ kind: 'request', requestIdentity: invocation.requestIdentity }),
			).toMatchObject({ execution: { phase: 'active' }, kind: 'found' }),
		);
		service.cancel({ kind: 'request', requestIdentity: invocation.requestIdentity });
		expect(await waitForTerminal(service, invocation)).toEqual({
			code: 'agent-changes-cancelled',
			kind: 'cancelled',
		});
	});

	it('returns an explicit reset page after retained changes overflow', async () => {
		const runtime = new AgentRuntime();
		const { service } = createService(runtime);
		const baseline = await invokeAndRead(service, request(1, 'agent.session.changes', {}));
		const cursor = resultOutputProperty(baseline, 'nextCursor');

		runtime.dispatch(terminalObserved());
		for (let sequence = 2; sequence <= 513; sequence += 1) {
			runtime.dispatch({
				activity: sequence % 2 === 0 ? 'command' : 'thinking',
				eventId: AgentEventIdSchema.parse(`event-desktop-change-${sequence}`),
				generation: 0,
				sequence,
				sessionId: AgentSessionIdSchema.parse('session-desktop-retention'),
				type: 'limited-activity-observed',
			});
		}

		expect(
			await invokeAndRead(
				service,
				request(2, 'agent.session.changes', { cursor, timeoutMilliseconds: 1 }),
			),
		).toMatchObject({
			code: 'agent-changes-reset-required',
			kind: 'completed',
			output: {
				items: [],
				kind: 'reset-required',
				observedThroughRevision: 513,
				runtimeRevision: 513,
			},
		});
	});

	it('pages revision-fenced summaries and excludes ended Sessions by default', async () => {
		const runtime = new AgentRuntime();
		runtime.dispatch(observed('session-a', 'event-a', 'execution-a'));
		runtime.dispatch(observed('session-b', 'event-b', 'execution-b'));
		runtime.dispatch(ended('session-b', 'event-b-ended', 2));
		const { service } = createService(runtime);

		const live = await invokeAndRead(service, request(1, 'agent.session.list', { limit: 1 }));
		expect(live).toMatchObject({
			kind: 'completed',
			output: {
				items: [{ lifecycle: 'live', sessionId: 'session-a' }],
				nextCursor: null,
				revision: 3,
			},
		});

		const first = await invokeAndRead(
			service,
			request(2, 'agent.session.list', { includeEnded: true, limit: 1 }),
		);
		const cursor = resultOutputProperty(first, 'nextCursor');
		expect(typeof cursor).toBe('string');
		expect(
			await invokeAndRead(
				service,
				request(3, 'agent.session.list', { cursor, includeEnded: true, limit: 1 }),
			),
		).toMatchObject({ kind: 'completed', output: { items: [{ sessionId: 'session-b' }] } });

		runtime.dispatch(stale('session-a', 'event-a-stale', 2));
		expect(
			await invokeAndRead(
				service,
				request(4, 'agent.session.list', { cursor, includeEnded: true, limit: 1 }),
			),
		).toEqual({ code: 'page-cursor-stale', kind: 'blocked' });
	});

	it('returns exact detail and a content-free canonical explanation', async () => {
		const { service } = createService(runtimeWithWorkingSession());

		expect(
			await invokeAndRead(
				service,
				request(1, 'agent.session.get', { sessionId: 'session-working' }),
			),
		).toMatchObject({
			code: 'agent-session-read',
			kind: 'completed',
			output: {
				revision: 2,
				session: { status: { activity: 'thinking', kind: 'working' } },
			},
		});
		const explained = await invokeAndRead(
			service,
			request(2, 'agent.session.explain', { sessionId: 'session-working' }),
		);
		expect(explained).toMatchObject({
			code: 'agent-session-explained',
			kind: 'completed',
			output: {
				explanation: {
					actionPath: 'navigate-only',
					contentAvailable: false,
					statusBasis: 'active-turn',
				},
			},
		});
		expect(JSON.stringify(explained)).not.toContain('prompt');
		expect(
			await invokeAndRead(service, request(3, 'agent.session.get', { sessionId: 'missing' })),
		).toEqual({ code: 'agent-session-not-found', kind: 'blocked' });
	});

	it('waits for an exact newer matching Session snapshot', async () => {
		const runtime = runtimeWithWorkingSession();
		const { service } = createService(runtime);
		const invocation = request(1, 'agent.session.wait', {
			afterRevision: runtime.getSnapshot().revision,
			sessionId: 'session-working',
			timeoutMilliseconds: 1_000,
			until: ['succeeded'],
		});
		const admitted = service.invoke({ kind: 'local-user' }, invocation);
		expect(admitted).toMatchObject({ kind: 'accepted', execution: { phase: 'active' } });

		runtime.dispatch(finished('session-working', 'event-finished', 3));

		const result = await waitForTerminal(service, invocation);
		expect(result).toMatchObject({
			code: 'agent-wait-matched',
			kind: 'completed',
			output: {
				matchedConditions: ['succeeded'],
				revision: 3,
				session: { status: { kind: 'succeeded' } },
			},
		});
	});

	it('does not match a stale semantic status unless degraded health is requested', async () => {
		const runtime = runtimeWithWorkingSession();
		runtime.dispatch(stale('session-working', 'event-stale', 3));
		let expire: (() => void) | undefined;
		const { service } = createService(runtime, {
			scheduleTimeout: (listener) => {
				expire = listener;
				return () => undefined;
			},
		});
		const waitWorking = request(1, 'agent.session.wait', {
			sessionId: 'session-working',
			timeoutMilliseconds: 1,
			until: ['working'],
		});
		service.invoke({ kind: 'local-user' }, waitWorking);
		await vi.waitFor(() => expect(expire).toBeTypeOf('function'));
		expire?.();
		expect(await waitForTerminal(service, waitWorking)).toEqual({
			code: 'agent-wait-timeout',
			kind: 'blocked',
		});
		expect(
			await invokeAndRead(
				service,
				request(2, 'agent.session.wait', {
					sessionId: 'session-working',
					timeoutMilliseconds: 1,
					until: ['stale'],
				}),
			),
		).toMatchObject({ kind: 'completed', output: { matchedConditions: ['stale'] } });
	});

	it('settles cancellation and rejects an ahead revision without observing invented state', async () => {
		const runtime = runtimeWithWorkingSession();
		const { service } = createService(runtime);
		expect(
			await invokeAndRead(
				service,
				request(1, 'agent.session.wait', {
					afterRevision: 99,
					sessionId: 'session-working',
					timeoutMilliseconds: 1,
					until: ['idle'],
				}),
			),
		).toEqual({ code: 'agent-revision-ahead', kind: 'blocked' });

		const invocation = request(2, 'agent.session.wait', {
			sessionId: 'session-working',
			timeoutMilliseconds: 1_000,
			until: ['idle'],
		});
		service.invoke({ kind: 'local-user' }, invocation);
		await vi.waitFor(() => {
			expect(
				service.get({ kind: 'request', requestIdentity: invocation.requestIdentity }),
			).toMatchObject({
				kind: 'found',
				execution: { phase: 'active' },
			});
		});
		expect(
			service.cancel({ kind: 'request', requestIdentity: invocation.requestIdentity }),
		).toMatchObject({ kind: 'cancellation-observed', forwarded: true });
		expect(await waitForTerminal(service, invocation)).toEqual({
			code: 'agent-wait-cancelled',
			kind: 'cancelled',
		});
	});

	it('closes the initial-read subscription race with a decisive recheck', async () => {
		const runtime = runtimeWithWorkingSession();
		const before = runtime.getSnapshot();
		runtime.dispatch(finished('session-working', 'event-race-finished', 3));
		const after = runtime.getSnapshot();
		let current = before;
		const unsubscribe = vi.fn<() => void>();
		const source: AgentChangeSource = {
			getSnapshot: () => current,
			readChanges: runtime.readChanges,
			subscribe: (listener) => {
				current = after;
				listener();
				return unsubscribe;
			},
		};
		const { service } = createService(source);

		expect(
			await invokeAndRead(
				service,
				request(1, 'agent.session.wait', {
					afterRevision: before.revision,
					sessionId: 'session-working',
					timeoutMilliseconds: 1_000,
					until: ['succeeded'],
				}),
			),
		).toMatchObject({ kind: 'completed', output: { revision: after.revision } });
		expect(unsubscribe).toHaveBeenCalledOnce();
	});

	it('cleans up a timeout scheduler that settles synchronously', async () => {
		const cancelTimeout = vi.fn<() => void>();
		const { service } = createService(runtimeWithWorkingSession(), {
			scheduleTimeout: (listener) => {
				listener();
				return cancelTimeout;
			},
		});

		expect(
			await invokeAndRead(
				service,
				request(1, 'agent.session.wait', {
					sessionId: 'session-working',
					timeoutMilliseconds: 1,
					until: ['idle'],
				}),
			),
		).toEqual({ code: 'agent-wait-timeout', kind: 'blocked' });
		expect(cancelTimeout).toHaveBeenCalledOnce();
	});
});

function createService(
	source: AgentChangeSource,
	overrides: Partial<DesktopAgentReadOperationOptions> = {},
) {
	let execution = 0;
	const service = new DesktopSemanticCommandService(
		new DesktopSemanticCommandCatalog(
			createDesktopAgentReadOperations({
				coordinatorGeneration: generation,
				cursorSecret: new Uint8Array(32).fill(7),
				source,
				...overrides,
			}),
		),
		{
			coordinatorGeneration: generation,
			createExecutionIdentity: () =>
				SemanticCommandExecutionIdentitySchema.parse(`desktop:agent-execution:${++execution}`),
		},
	);
	return { service };
}

function request(index: number, operationId: string, input: unknown) {
	return SemanticCommandInvocationRequestSchema.parse({
		input,
		operationId: SemanticCommandOperationIdSchema.parse(operationId),
		operationSchemaVersion: 1,
		protocolVersion: 1,
		requestIdentity: `cli:agent-read:${index}`,
	});
}

async function invokeAndRead(
	service: DesktopSemanticCommandService,
	invocation: SemanticCommandInvocationRequest,
) {
	const admission = service.invoke({ kind: 'local-user' }, invocation);
	if (admission.kind !== 'accepted') return admission;
	return waitForTerminal(service, invocation);
}

async function waitForTerminal(
	service: DesktopSemanticCommandService,
	invocation: SemanticCommandInvocationRequest,
) {
	let result: unknown;
	await vi.waitFor(() => {
		const observation = service.get({
			kind: 'request',
			requestIdentity: invocation.requestIdentity,
		});
		if (observation.kind !== 'found' || observation.execution.phase !== 'terminal') {
			throw new Error('Agent read operation has not settled yet.');
		}
		result = observation.execution.result;
	});
	return result;
}

function resultOutputProperty(result: unknown, key: string): unknown {
	if (typeof result !== 'object' || result === null) return undefined;
	const output = Reflect.get(result, 'output');
	return typeof output === 'object' && output !== null ? Reflect.get(output, key) : undefined;
}

function runtimeWithWorkingSession(): AgentRuntime {
	const runtime = new AgentRuntime();
	runtime.dispatch(observed('session-working', 'event-observed', 'execution-working'));
	runtime.dispatch(started('session-working', 'event-started', 2));
	return runtime;
}

function observed(
	session: string,
	event: string,
	execution: string,
): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['sessionLifecycle', 'turnLifecycle'],
		eventId: AgentEventIdSchema.parse(event),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse(execution),
		},
		observation: {
			fidelity: 'rich',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'hook',
		},
		parentSessionId: null,
		presentationAttachments: [],
		providerId: AgentProviderIdSchema.parse('codex'),
		sessionId: AgentSessionIdSchema.parse(session),
		type: 'session-observed',
	};
}

function terminalObserved(): Extract<AgentObservationEvent, { readonly type: 'session-observed' }> {
	return {
		capabilities: ['activity'],
		eventId: AgentEventIdSchema.parse('event-desktop-retention-observed'),
		executionLocation: {
			executionLocationId: AgentExecutionLocationIdSchema.parse('execution-desktop-retention'),
		},
		observation: {
			fidelity: 'basic',
			freshness: 'fresh',
			generation: 0,
			sequence: 1,
			source: 'terminal',
		},
		parentSessionId: null,
		presentationAttachments: [],
		providerId: AgentProviderIdSchema.parse('terminal-signal'),
		sessionId: AgentSessionIdSchema.parse('session-desktop-retention'),
		type: 'session-observed',
	};
}

function started(
	session: string,
	event: string,
	sequence: number,
): Extract<AgentObservationEvent, { readonly type: 'turn-started' }> {
	return {
		activity: 'thinking',
		eventId: AgentEventIdSchema.parse(event),
		generation: 0,
		phase: 'working',
		sequence,
		sessionId: AgentSessionIdSchema.parse(session),
		turnId: AgentTurnIdSchema.parse(`turn:${session}`),
		type: 'turn-started',
	};
}

function finished(
	session: string,
	event: string,
	sequence: number,
): Extract<AgentObservationEvent, { readonly type: 'turn-finished' }> {
	return {
		eventId: AgentEventIdSchema.parse(event),
		generation: 0,
		outcome: 'succeeded',
		sequence,
		sessionId: AgentSessionIdSchema.parse(session),
		turnId: AgentTurnIdSchema.parse(`turn:${session}`),
		type: 'turn-finished',
	};
}

function stale(
	session: string,
	event: string,
	sequence: number,
): Extract<AgentObservationEvent, { readonly type: 'observation-became-stale' }> {
	return {
		eventId: AgentEventIdSchema.parse(event),
		generation: 0,
		sequence,
		sessionId: AgentSessionIdSchema.parse(session),
		type: 'observation-became-stale',
	};
}

function ended(
	session: string,
	event: string,
	sequence: number,
): Extract<AgentObservationEvent, { readonly type: 'session-ended' }> {
	return {
		eventId: AgentEventIdSchema.parse(event),
		generation: 0,
		sequence,
		sessionId: AgentSessionIdSchema.parse(session),
		type: 'session-ended',
	};
}
