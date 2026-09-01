import {
	AgentEventIdSchema,
	AgentExecutionLocationIdSchema,
	AgentProviderIdSchema,
	AgentSessionIdSchema,
} from '@orchbee/agent';
import type { DesktopAgentIntegrationId } from '@orchbee/desktop-client-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	DesktopAgentObservationRuntime,
	type DesktopAgentPassivePresenceEvidence,
} from '../DesktopAgentObservationRuntime';

afterEach(() => vi.useRealTimers());

describe('DesktopAgentObservationRuntime passive presence', () => {
	it('projects curated launch evidence as capability-free heuristic unknown state', () => {
		const runtime = createRuntime();
		const semanticListener =
			vi.fn<Parameters<DesktopAgentObservationRuntime['subscribeSemanticEvents']>[0]>();
		runtime.subscribeSemanticEvents(semanticListener);

		expect(runtime.observePassivePresence(evidence())).toBe(true);
		expect(runtime.getSnapshot().sessions).toEqual([
			expect.objectContaining({
				activityHint: null,
				capabilities: [],
				lastOutcome: null,
				lifecycle: 'live',
				observation: {
					fidelity: 'heuristic',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'heuristic',
				},
				pendingRequests: [],
				providerId: 'codex',
				turn: { kind: 'unknown' },
			}),
		]);
		expect(runtime.getPassivePresenceDiagnostics()).toEqual([
			{
				...evidence(),
				freshness: 'fresh',
				observedAt: 1_000,
			},
		]);
		expect(semanticListener).not.toHaveBeenCalled();

		const revision = runtime.getSnapshot().revision;
		expect(runtime.observePassivePresence(evidence())).toBe(true);
		expect(runtime.getSnapshot().revision).toBe(revision);
		expect(semanticListener).not.toHaveBeenCalled();
		runtime.dispose();
	});

	it('uses deterministic stale and retirement hysteresis and admits later evidence safely', () => {
		vi.useFakeTimers();
		vi.setSystemTime(1_000);
		const runtime = createRuntime({
			detachAfterMilliseconds: 200,
			now: Date.now,
			staleAfterMilliseconds: 100,
		});
		const semanticListener =
			vi.fn<Parameters<DesktopAgentObservationRuntime['subscribeSemanticEvents']>[0]>();
		runtime.subscribeSemanticEvents(semanticListener);
		runtime.observePassivePresence(evidence());

		vi.advanceTimersByTime(100);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			observation: { freshness: 'stale' },
			presentationAttachments: [{ navigable: true }],
		});
		expect(runtime.getPassivePresenceDiagnostics()[0]?.freshness).toBe('stale');

		expect(runtime.observePassivePresence(evidence())).toBe(true);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			observation: { freshness: 'fresh', generation: 1 },
		});
		vi.advanceTimersByTime(200);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			lifecycle: 'ended',
			observation: { freshness: 'disconnected' },
			presentationAttachments: [{ navigable: false, visibility: 'unknown' }],
		});
		expect(runtime.getPassivePresenceDiagnostics()[0]?.freshness).toBe('disconnected');
		expect(runtime.observePassivePresence(evidence())).toBe(true);
		const liveSession = runtime
			.getSnapshot()
			.sessions.find((session) => session.lifecycle === 'live');
		expect(liveSession).toMatchObject({
			observation: { freshness: 'fresh', generation: 0 },
			presentationAttachments: [{ navigable: true, visibility: 'unknown' }],
		});
		expect(runtime.getPassivePresenceDiagnostics()[0]?.freshness).toBe('fresh');
		expect(semanticListener).not.toHaveBeenCalled();
		runtime.dispose();
	});

	it('lets an exact rich observation replace passive presence without duplicate live state', () => {
		const runtime = createRuntime();
		const semanticListener =
			vi.fn<Parameters<DesktopAgentObservationRuntime['subscribeSemanticEvents']>[0]>();
		runtime.subscribeSemanticEvents(semanticListener);
		expect(runtime.observePassivePresence(evidence())).toBe(true);

		expect(
			runtime.dispatch({
				capabilities: ['sessionLifecycle', 'turnLifecycle'],
				eventId: AgentEventIdSchema.parse('rich-event-1'),
				executionLocation: {
					executionLocationId: AgentExecutionLocationIdSchema.parse('rich-execution-1'),
				},
				observation: {
					fidelity: 'rich',
					freshness: 'fresh',
					generation: 0,
					sequence: 1,
					source: 'hook',
				},
				parentSessionId: null,
				presentationAttachments: [
					{
						generation: evidence().generation,
						navigable: true,
						presentationId: evidence().presentationId,
						visibility: 'visible',
					},
				],
				providerId: AgentProviderIdSchema.parse('codex'),
				sessionId: AgentSessionIdSchema.parse('rich-session-1'),
				type: 'session-observed',
			}),
		).toMatchObject({ kind: 'applied' });

		const snapshot = runtime.getSnapshot();
		expect(snapshot.sessions.filter((session) => session.lifecycle === 'live')).toEqual([
			expect.objectContaining({
				observation: expect.objectContaining({ fidelity: 'rich' }),
				sessionId: 'rich-session-1',
			}),
		]);
		expect(
			snapshot.sessions.find((session) => session.observation.fidelity === 'heuristic'),
		).toMatchObject({ lifecycle: 'ended', observation: { freshness: 'disconnected' } });
		expect(runtime.getPassivePresenceDiagnostics()[0]?.freshness).toBe('disconnected');

		const revision = snapshot.revision;
		expect(runtime.observePassivePresence(evidence())).toBe(true);
		expect(runtime.getSnapshot().revision).toBe(revision);
		expect(semanticListener).not.toHaveBeenCalled();
		runtime.dispose();
	});

	it('rejects malformed evidence and cannot promote adversarial extra fields', () => {
		const runtime = createRuntime();
		const semanticListener =
			vi.fn<Parameters<DesktopAgentObservationRuntime['subscribeSemanticEvents']>[0]>();
		runtime.subscribeSemanticEvents(semanticListener);
		const malformed = [
			{ ...evidence(), integrationId: 'unknown-agent' },
			{ ...evidence(), signatureId: 'screen/private prompt' },
			{ ...evidence(), source: 'semantic-hook' },
		] as unknown as DesktopAgentPassivePresenceEvidence[];
		for (const candidate of malformed) {
			expect(runtime.observePassivePresence(candidate)).toBe(false);
		}
		expect(runtime.getSnapshot().sessions).toEqual([]);

		expect(
			runtime.observePassivePresence({
				...evidence('claude-code'),
				activity: 'thinking',
				outcome: 'succeeded',
				request: 'approval',
			} as DesktopAgentPassivePresenceEvidence),
		).toBe(true);
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			activityHint: null,
			capabilities: [],
			lastOutcome: null,
			pendingRequests: [],
			turn: { kind: 'unknown' },
		});
		expect(semanticListener).not.toHaveBeenCalled();
		runtime.dispose();
	});

	it('retires only matching presentation evidence without a semantic end event', () => {
		const runtime = createRuntime();
		const semanticListener =
			vi.fn<Parameters<DesktopAgentObservationRuntime['subscribeSemanticEvents']>[0]>();
		runtime.subscribeSemanticEvents(semanticListener);
		runtime.observePassivePresence(evidence());

		runtime.retirePassivePresencePresentation('other-presentation');
		expect(runtime.getSnapshot().sessions[0]?.presentationAttachments[0]?.navigable).toBe(true);
		runtime.retirePassivePresencePresentation('presentation-1');
		expect(runtime.getSnapshot().sessions[0]).toMatchObject({
			lifecycle: 'ended',
			observation: { freshness: 'disconnected' },
			presentationAttachments: [{ navigable: false }],
		});
		expect(semanticListener).not.toHaveBeenCalled();
		runtime.dispose();
	});
});

function createRuntime(
	overrides: Partial<ConstructorParameters<typeof DesktopAgentObservationRuntime>[0]> = {},
): DesktopAgentObservationRuntime {
	let eventSequence = 0;
	return new DesktopAgentObservationRuntime({
		allocateEventId: () => `desktop-event-${++eventSequence}`,
		detachAfterMilliseconds: 2_000,
		now: () => 1_000,
		staleAfterMilliseconds: 1_000,
		...overrides,
	});
}

function evidence(
	integrationId: DesktopAgentIntegrationId = 'codex',
): DesktopAgentPassivePresenceEvidence {
	return {
		generation: '11223344556677889900aabbccddeeff',
		integrationId,
		presentationId: 'presentation-1',
		signatureId:
			integrationId === 'codex'
				? 'shell-preexec.executable.codex'
				: 'shell-preexec.executable.claude',
		source: 'terminal-command',
	};
}
