import { describe, expect, it } from 'vitest';

import type { SshEnvironmentResolvedTarget } from '../ssh-environment-contract';
import {
	createSshEnvironmentConnectionState,
	observeSshEnvironmentConnection,
	transitionSshEnvironmentConnection,
	type SshEnvironmentConnectionState,
} from '../ssh-environment-connection-state';

const connectionId = 'connection-1';
const target = createSshEnvironmentConnectionState({
	connectionId,
	target: { configurationFilePath: null, destination: 'build-host' },
}).target;
const resolvedTarget: SshEnvironmentResolvedTarget = {
	destination: target.destination,
	hostKeyLookupName: 'build.internal.example',
	hostKeyPolicy: 'ask',
	hostname: '10.0.0.8',
	port: 22,
	user: 'builder',
};
const promptHandle = 'prompt-1';
const loginShell = '/bin/zsh';
const runtimeEpoch = '2'.repeat(32);
const runtimeInstanceId = '1'.repeat(32);

describe('SSH Environment connection state', () => {
	it('requires target resolution and a remote-runtime handshake before becoming connected', () => {
		const stopped = createSshEnvironmentConnectionState({ connectionId, target });
		const started = transitionSshEnvironmentConnection(stopped, { type: 'start-requested' });
		expect(started).toMatchObject({
			commands: [{ generation: 1, type: 'resolve-target' }],
			outcome: 'applied',
			state: { generation: 1, phase: 'resolving' },
		});

		const resolved = transitionSshEnvironmentConnection(started.state, {
			generation: 1,
			resolvedTarget,
			type: 'target-resolved',
		});
		expect(resolved).toMatchObject({
			commands: [{ generation: 1, type: 'open-transport' }],
			state: { phase: 'connecting' },
		});
		expect(observeSshEnvironmentConnection(resolved.state)).toMatchObject({
			connected: false,
			phase: 'connecting',
		});

		const ready = transitionSshEnvironmentConnection(resolved.state, {
			generation: 1,
			loginShell,
			runtimeEpoch,
			runtimeInstanceId,
			type: 'remote-runtime-ready',
		});
		expect(ready).toMatchObject({
			commands: [],
			state: { connectedOnce: true, generation: 1, phase: 'connected' },
		});
		expect(observeSshEnvironmentConnection(ready.state)).toMatchObject({
			connected: true,
			loginShell,
			phase: 'connected',
			runtimeEpoch,
		});
	});

	it('rejects a resolved target for a different destination', () => {
		const stopped = createSshEnvironmentConnectionState({ connectionId, target });
		const started = transitionSshEnvironmentConnection(stopped, { type: 'start-requested' });
		const mismatchedTarget: SshEnvironmentResolvedTarget = {
			...resolvedTarget,
			destination: createSshEnvironmentConnectionState({
				connectionId,
				target: { configurationFilePath: null, destination: 'other-host' },
			}).target.destination,
		};

		expect(
			transitionSshEnvironmentConnection(started.state, {
				generation: 1,
				resolvedTarget: mismatchedTarget,
				type: 'target-resolved',
			}),
		).toMatchObject({
			commands: [],
			state: { phase: 'blocked', reason: 'configuration-invalid' },
		});
	});

	it('tracks only an opaque interaction handle and blocks a cancelled interaction', () => {
		const connecting = createConnectingState();
		const requested = transitionSshEnvironmentConnection(connecting, {
			generation: 1,
			handle: promptHandle,
			type: 'interaction-requested',
		});
		expect(requested).toMatchObject({
			commands: [{ generation: 1, handle: promptHandle, type: 'present-interaction' }],
			state: { handle: promptHandle, phase: 'interaction-required' },
		});
		expect(
			transitionSshEnvironmentConnection(requested.state, {
				generation: 1,
				handle: promptHandle,
				type: 'interaction-requested',
			}),
		).toMatchObject({ commands: [], outcome: 'unchanged' });

		const cancelled = transitionSshEnvironmentConnection(requested.state, {
			generation: 1,
			handle: promptHandle,
			outcome: 'cancelled',
			type: 'interaction-completed',
		});
		expect(cancelled).toMatchObject({
			commands: [{ generation: 1, type: 'retire-generation' }],
			state: { phase: 'blocked', reason: 'authentication-cancelled' },
		});
		expect(observeSshEnvironmentConnection(cancelled.state)).toEqual({
			blockedReason: 'authentication-cancelled',
			connected: false,
			connectionId,
			generation: 1,
			interactionRequired: false,
			loginShell: null,
			phase: 'blocked',
			retryAttempt: null,
			runtimeEpoch: null,
		});
	});

	it('fails closed on an invalid interaction handle from the process adapter', () => {
		expect(
			transitionSshEnvironmentConnection(createConnectingState(), {
				generation: 1,
				handle: 'prompt with spaces',
				type: 'interaction-requested',
			}),
		).toMatchObject({
			commands: [{ generation: 1, type: 'retire-generation' }],
			state: { phase: 'blocked', reason: 'policy-rejected' },
		});
	});

	it('resumes the same connecting generation after interaction submission', () => {
		const requested = transitionSshEnvironmentConnection(createConnectingState(), {
			generation: 1,
			handle: promptHandle,
			type: 'interaction-requested',
		});
		const submitted = transitionSshEnvironmentConnection(requested.state, {
			generation: 1,
			handle: promptHandle,
			outcome: 'submitted',
			type: 'interaction-completed',
		});
		expect(submitted).toMatchObject({
			commands: [],
			state: { generation: 1, phase: 'connecting', retryAttempt: 0 },
		});
	});

	it('blocks an initial retryable failure instead of starting an unattended retry loop', () => {
		const failed = transitionSshEnvironmentConnection(createConnectingState(), {
			generation: 1,
			reason: 'retryable',
			type: 'transport-failed',
		});
		expect(failed).toMatchObject({
			commands: [{ generation: 1, type: 'retire-generation' }],
			state: { phase: 'blocked', reason: 'connection-failed' },
		});
	});

	it('uses one fresh generation and bounded equal jitter after a connected transport is lost', () => {
		const connected = createConnectedState();
		const lost = transitionSshEnvironmentConnection(connected, {
			generation: 1,
			type: 'transport-lost',
		});
		expect(lost).toMatchObject({
			commands: [
				{ generation: 1, type: 'retire-generation' },
				{
					generation: 2,
					maximumDelayMilliseconds: 1_000,
					minimumDelayMilliseconds: 500,
					retryAttempt: 1,
					type: 'schedule-retry',
				},
			],
			state: { generation: 2, phase: 'retry-wait', retryAttempt: 1 },
		});

		const elapsed = transitionSshEnvironmentConnection(lost.state, {
			generation: 2,
			type: 'retry-elapsed',
		});
		expect(elapsed).toMatchObject({
			commands: [{ generation: 2, type: 'resolve-target' }],
			state: { phase: 'resolving', retryAttempt: 1 },
		});
		const resolved = transitionSshEnvironmentConnection(elapsed.state, {
			generation: 2,
			resolvedTarget,
			type: 'target-resolved',
		});
		const failed = transitionSshEnvironmentConnection(resolved.state, {
			generation: 2,
			reason: 'retryable',
			type: 'transport-failed',
		});
		expect(failed).toMatchObject({
			commands: [
				{ generation: 2, type: 'retire-generation' },
				{
					generation: 3,
					maximumDelayMilliseconds: 2_000,
					minimumDelayMilliseconds: 1_000,
					retryAttempt: 2,
					type: 'schedule-retry',
				},
			],
			state: { generation: 3, phase: 'retry-wait', retryAttempt: 2 },
		});
	});

	it('caps the retry interval at thirty seconds', () => {
		const state = createConnectingState();
		if (state.phase !== 'connecting') throw new Error('Expected a connecting test state.');
		const reconnecting = { ...state, connectedOnce: true as const, retryAttempt: 5 };

		expect(
			transitionSshEnvironmentConnection(reconnecting, {
				generation: 1,
				reason: 'retryable',
				type: 'transport-failed',
			}),
		).toMatchObject({
			commands: [
				{ generation: 1, type: 'retire-generation' },
				{
					generation: 2,
					maximumDelayMilliseconds: 30_000,
					minimumDelayMilliseconds: 15_000,
					retryAttempt: 6,
					type: 'schedule-retry',
				},
			],
		});
	});

	it.each([
		'authentication-required',
		'compatibility-unsupported',
		'configuration-invalid',
		'host-verification-failed',
		'policy-rejected',
	] as const)('blocks the permanent transport failure %s', (reason) => {
		const failed = transitionSshEnvironmentConnection(createConnectingState(), {
			generation: 1,
			reason,
			type: 'transport-failed',
		});
		expect(failed).toMatchObject({ state: { phase: 'blocked', reason } });
	});

	it('ignores stale ready, timer, interaction, and transport completions', () => {
		const connected = createConnectedState();
		for (const event of [
			{
				generation: 0,
				loginShell,
				runtimeEpoch,
				runtimeInstanceId,
				type: 'remote-runtime-ready' as const,
			},
			{ generation: 0, type: 'retry-elapsed' as const },
			{
				generation: 0,
				handle: promptHandle,
				type: 'interaction-requested' as const,
			},
			{ generation: 0, reason: 'retryable' as const, type: 'transport-failed' as const },
		]) {
			const transition = transitionSshEnvironmentConnection(connected, event);
			expect(transition).toEqual({ commands: [], outcome: 'unchanged', state: connected });
		}
	});

	it('fences active work on stop and ignores its later completion', () => {
		const connecting = createConnectingState();
		const stopped = transitionSshEnvironmentConnection(connecting, { type: 'stop-requested' });
		expect(stopped).toMatchObject({
			commands: [{ generation: 1, type: 'retire-generation' }],
			state: { generation: 2, phase: 'stopped' },
		});
		const staleReady = transitionSshEnvironmentConnection(stopped.state, {
			generation: 1,
			loginShell,
			runtimeEpoch,
			runtimeInstanceId,
			type: 'remote-runtime-ready',
		});
		expect(staleReady).toEqual({
			commands: [],
			outcome: 'unchanged',
			state: stopped.state,
		});
	});

	it('retires the exact current generation when every active phase stops', () => {
		const resolving = transitionSshEnvironmentConnection(
			createSshEnvironmentConnectionState({ connectionId, target }),
			{ type: 'start-requested' },
		).state;
		const connecting = createConnectingState();
		const interaction = transitionSshEnvironmentConnection(connecting, {
			generation: 1,
			handle: promptHandle,
			type: 'interaction-requested',
		}).state;
		const connected = createConnectedState();
		const retryWait = transitionSshEnvironmentConnection(connected, {
			generation: 1,
			type: 'transport-lost',
		}).state;

		for (const state of [resolving, connecting, interaction, connected, retryWait]) {
			const stopped = transitionSshEnvironmentConnection(state, { type: 'stop-requested' });
			expect(stopped).toMatchObject({
				commands: [{ generation: state.generation, type: 'retire-generation' }],
				state: { generation: state.generation + 1, phase: 'stopped' },
			});
		}
	});

	it('blocks target resolution failure and allows an explicit manual restart', () => {
		const started = transitionSshEnvironmentConnection(
			createSshEnvironmentConnectionState({ connectionId, target }),
			{ type: 'start-requested' },
		);
		const blocked = transitionSshEnvironmentConnection(started.state, {
			generation: 1,
			reason: 'configuration-invalid',
			type: 'target-resolution-failed',
		});
		expect(blocked).toMatchObject({ state: { phase: 'blocked' } });
		const restarted = transitionSshEnvironmentConnection(blocked.state, {
			type: 'start-requested',
		});
		expect(restarted).toMatchObject({
			commands: [{ generation: 2, type: 'resolve-target' }],
			state: { generation: 2, phase: 'resolving', retryAttempt: 0 },
		});
	});
});

function createConnectingState(): SshEnvironmentConnectionState {
	const started = transitionSshEnvironmentConnection(
		createSshEnvironmentConnectionState({ connectionId, target }),
		{ type: 'start-requested' },
	);
	return transitionSshEnvironmentConnection(started.state, {
		generation: 1,
		resolvedTarget,
		type: 'target-resolved',
	}).state;
}

function createConnectedState(): SshEnvironmentConnectionState {
	return transitionSshEnvironmentConnection(createConnectingState(), {
		generation: 1,
		loginShell,
		runtimeEpoch,
		runtimeInstanceId,
		type: 'remote-runtime-ready',
	}).state;
}
