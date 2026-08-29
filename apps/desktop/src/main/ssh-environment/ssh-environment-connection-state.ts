import {
	SshEnvironmentRuntimeEpochSchema,
	SshEnvironmentRuntimeInstanceIdSchema,
	type SshEnvironmentRuntimeEpoch,
	type SshEnvironmentRuntimeInstanceId,
} from '@orchbee/ssh-environment-protocol';

import {
	SshEnvironmentConnectionIdSchema,
	SshEnvironmentInteractionHandleSchema,
	SshEnvironmentTargetReferenceSchema,
	type SshEnvironmentConnectionId,
	type SshEnvironmentInteractionHandle,
	type SshEnvironmentResolvedTarget,
	type SshEnvironmentTargetReference,
} from './ssh-environment-contract';

const INITIAL_RETRY_MAXIMUM_DELAY_MILLISECONDS = 1_000;
const MAXIMUM_RETRY_DELAY_MILLISECONDS = 30_000;

type SshEnvironmentConnectionBlockedReason =
	| 'authentication-cancelled'
	| 'authentication-required'
	| 'compatibility-unsupported'
	| 'configuration-invalid'
	| 'connection-failed'
	| 'host-verification-failed'
	| 'policy-rejected'
	| 'runtime-release-unavailable';

type SshEnvironmentTransportFailureReason =
	| Exclude<SshEnvironmentConnectionBlockedReason, 'authentication-cancelled' | 'connection-failed'>
	| 'retryable';

type SshEnvironmentTargetResolutionFailureReason = Extract<
	SshEnvironmentConnectionBlockedReason,
	'compatibility-unsupported' | 'configuration-invalid' | 'policy-rejected'
>;

interface SshEnvironmentConnectionStateBase {
	readonly connectedOnce: boolean;
	readonly connectionId: SshEnvironmentConnectionId;
	readonly generation: number;
	readonly runtimeIdentity: SshEnvironmentRuntimeIdentity | null;
	readonly target: SshEnvironmentTargetReference;
}

interface SshEnvironmentRuntimeIdentity {
	readonly runtimeEpoch: SshEnvironmentRuntimeEpoch;
	readonly runtimeInstanceId: SshEnvironmentRuntimeInstanceId;
}

interface SshEnvironmentConnectionStoppedState extends SshEnvironmentConnectionStateBase {
	readonly phase: 'stopped';
}

interface SshEnvironmentConnectionResolvingState extends SshEnvironmentConnectionStateBase {
	readonly phase: 'resolving';
	readonly retryAttempt: number;
}

interface SshEnvironmentConnectionConnectingState extends SshEnvironmentConnectionStateBase {
	readonly phase: 'connecting';
	readonly resolvedTarget: SshEnvironmentResolvedTarget;
	readonly retryAttempt: number;
}

interface SshEnvironmentConnectionInteractionRequiredState extends SshEnvironmentConnectionStateBase {
	readonly handle: SshEnvironmentInteractionHandle;
	readonly phase: 'interaction-required';
	readonly resolvedTarget: SshEnvironmentResolvedTarget;
	readonly retryAttempt: number;
}

interface SshEnvironmentConnectionConnectedState extends SshEnvironmentConnectionStateBase {
	readonly connectedOnce: true;
	readonly phase: 'connected';
	readonly resolvedTarget: SshEnvironmentResolvedTarget;
	readonly runtimeIdentity: SshEnvironmentRuntimeIdentity;
}

interface SshEnvironmentConnectionRetryWaitState extends SshEnvironmentConnectionStateBase {
	readonly connectedOnce: true;
	readonly maximumDelayMilliseconds: number;
	readonly minimumDelayMilliseconds: number;
	readonly phase: 'retry-wait';
	readonly retryAttempt: number;
}

interface SshEnvironmentConnectionBlockedState extends SshEnvironmentConnectionStateBase {
	readonly phase: 'blocked';
	readonly reason: SshEnvironmentConnectionBlockedReason;
}

export type SshEnvironmentConnectionState =
	| SshEnvironmentConnectionStoppedState
	| SshEnvironmentConnectionResolvingState
	| SshEnvironmentConnectionConnectingState
	| SshEnvironmentConnectionInteractionRequiredState
	| SshEnvironmentConnectionConnectedState
	| SshEnvironmentConnectionRetryWaitState
	| SshEnvironmentConnectionBlockedState;

export type SshEnvironmentConnectionEvent =
	| { readonly type: 'start-requested' }
	| { readonly type: 'stop-requested' }
	| {
			readonly generation: number;
			readonly resolvedTarget: SshEnvironmentResolvedTarget;
			readonly type: 'target-resolved';
	  }
	| {
			readonly generation: number;
			readonly reason: SshEnvironmentTargetResolutionFailureReason;
			readonly type: 'target-resolution-failed';
	  }
	| {
			readonly generation: number;
			readonly handle: string;
			readonly type: 'interaction-requested';
	  }
	| {
			readonly generation: number;
			readonly handle: string;
			readonly outcome: 'cancelled' | 'submitted';
			readonly type: 'interaction-completed';
	  }
	| {
			readonly generation: number;
			readonly runtimeEpoch: string;
			readonly runtimeInstanceId: string;
			readonly type: 'remote-runtime-ready';
	  }
	| {
			readonly generation: number;
			readonly reason: SshEnvironmentTransportFailureReason;
			readonly type: 'transport-failed';
	  }
	| { readonly generation: number; readonly type: 'transport-lost' }
	| { readonly generation: number; readonly type: 'retry-elapsed' };

export type SshEnvironmentConnectionCommand =
	| {
			readonly connectionId: SshEnvironmentConnectionId;
			readonly generation: number;
			readonly target: SshEnvironmentTargetReference;
			readonly type: 'resolve-target';
	  }
	| {
			readonly connectionId: SshEnvironmentConnectionId;
			readonly generation: number;
			readonly resolvedTarget: SshEnvironmentResolvedTarget;
			readonly type: 'open-transport';
	  }
	| {
			readonly connectionId: SshEnvironmentConnectionId;
			readonly generation: number;
			readonly handle: SshEnvironmentInteractionHandle;
			readonly type: 'present-interaction';
	  }
	| {
			readonly connectionId: SshEnvironmentConnectionId;
			readonly generation: number;
			readonly maximumDelayMilliseconds: number;
			readonly minimumDelayMilliseconds: number;
			readonly retryAttempt: number;
			readonly type: 'schedule-retry';
	  }
	| {
			readonly connectionId: SshEnvironmentConnectionId;
			readonly generation: number;
			readonly type: 'retire-generation';
	  };

export interface SshEnvironmentConnectionTransition {
	readonly commands: readonly SshEnvironmentConnectionCommand[];
	readonly outcome: 'applied' | 'unchanged';
	readonly state: SshEnvironmentConnectionState;
}

export interface SshEnvironmentConnectionObservation {
	readonly blockedReason: SshEnvironmentConnectionBlockedReason | null;
	readonly connected: boolean;
	readonly connectionId: SshEnvironmentConnectionId;
	readonly generation: number;
	readonly interactionRequired: boolean;
	readonly phase: SshEnvironmentConnectionState['phase'];
	readonly retryAttempt: number | null;
	readonly runtimeEpoch: SshEnvironmentRuntimeEpoch | null;
}

export interface SshEnvironmentConnectionConfiguration {
	readonly connectionId: string;
	readonly target: {
		readonly configurationFilePath: null | string;
		readonly destination: string;
	};
}

export function createSshEnvironmentConnectionState(
	configuration: SshEnvironmentConnectionConfiguration,
): SshEnvironmentConnectionState {
	return {
		connectedOnce: false,
		connectionId: SshEnvironmentConnectionIdSchema.parse(configuration.connectionId),
		generation: 0,
		phase: 'stopped',
		runtimeIdentity: null,
		target: SshEnvironmentTargetReferenceSchema.parse(configuration.target),
	};
}

export function observeSshEnvironmentConnection(
	state: SshEnvironmentConnectionState,
): SshEnvironmentConnectionObservation {
	return {
		blockedReason: state.phase === 'blocked' ? state.reason : null,
		connected: state.phase === 'connected',
		connectionId: state.connectionId,
		generation: state.generation,
		interactionRequired: state.phase === 'interaction-required',
		phase: state.phase,
		retryAttempt:
			state.phase === 'resolving' ||
			state.phase === 'connecting' ||
			state.phase === 'interaction-required' ||
			state.phase === 'retry-wait'
				? state.retryAttempt
				: null,
		runtimeEpoch: state.runtimeIdentity?.runtimeEpoch ?? null,
	};
}

export function transitionSshEnvironmentConnection(
	state: SshEnvironmentConnectionState,
	event: SshEnvironmentConnectionEvent,
): SshEnvironmentConnectionTransition {
	if ('generation' in event && event.generation !== state.generation) return unchanged(state);
	switch (event.type) {
		case 'start-requested':
			return start(state);
		case 'stop-requested':
			return stop(state);
		case 'target-resolved':
			return targetResolved(state, event.resolvedTarget);
		case 'target-resolution-failed':
			return state.phase === 'resolving' ? block(state, event.reason, false) : unchanged(state);
		case 'interaction-requested':
			return interactionRequested(state, event.handle);
		case 'interaction-completed':
			return interactionCompleted(state, event.handle, event.outcome);
		case 'remote-runtime-ready':
			return runtimeReady(state, event.runtimeInstanceId, event.runtimeEpoch);
		case 'transport-failed':
			return transportFailed(state, event.reason);
		case 'transport-lost':
			return state.phase === 'connected' ? scheduleRetry(state, 1) : unchanged(state);
		case 'retry-elapsed':
			return retryElapsed(state);
	}
}

function start(state: SshEnvironmentConnectionState): SshEnvironmentConnectionTransition {
	if (state.phase !== 'stopped' && state.phase !== 'blocked') return unchanged(state);
	const generation = nextGeneration(state.generation);
	const nextState: SshEnvironmentConnectionResolvingState = {
		connectedOnce: state.connectedOnce,
		connectionId: state.connectionId,
		generation,
		phase: 'resolving',
		retryAttempt: 0,
		runtimeIdentity: state.runtimeIdentity,
		target: state.target,
	};
	return applied(nextState, [resolveTargetCommand(nextState)]);
}

function stop(state: SshEnvironmentConnectionState): SshEnvironmentConnectionTransition {
	if (state.phase === 'stopped') return unchanged(state);
	const retiredGeneration = state.generation;
	const nextState: SshEnvironmentConnectionStoppedState = {
		connectedOnce: state.connectedOnce,
		connectionId: state.connectionId,
		generation: nextGeneration(state.generation),
		phase: 'stopped',
		runtimeIdentity: state.runtimeIdentity,
		target: state.target,
	};
	return applied(
		nextState,
		state.phase === 'blocked' ? [] : [retireGenerationCommand(state, retiredGeneration)],
	);
}

function targetResolved(
	state: SshEnvironmentConnectionState,
	resolvedTarget: SshEnvironmentResolvedTarget,
): SshEnvironmentConnectionTransition {
	if (state.phase !== 'resolving') return unchanged(state);
	if (resolvedTarget.destination !== state.target.destination) {
		return block(state, 'configuration-invalid', false);
	}
	const nextState: SshEnvironmentConnectionConnectingState = {
		...state,
		phase: 'connecting',
		resolvedTarget,
	};
	return applied(nextState, [
		{
			connectionId: state.connectionId,
			generation: state.generation,
			resolvedTarget,
			type: 'open-transport',
		},
	]);
}

function interactionRequested(
	state: SshEnvironmentConnectionState,
	handle: string,
): SshEnvironmentConnectionTransition {
	if (state.phase === 'interaction-required') {
		return state.handle === handle ? unchanged(state) : block(state, 'policy-rejected', true);
	}
	if (state.phase !== 'connecting') return unchanged(state);
	const parsedHandle = SshEnvironmentInteractionHandleSchema.safeParse(handle);
	if (!parsedHandle.success) return block(state, 'policy-rejected', true);
	const nextState: SshEnvironmentConnectionInteractionRequiredState = {
		...state,
		handle: parsedHandle.data,
		phase: 'interaction-required',
	};
	return applied(nextState, [
		{
			connectionId: state.connectionId,
			generation: state.generation,
			handle: parsedHandle.data,
			type: 'present-interaction',
		},
	]);
}

function interactionCompleted(
	state: SshEnvironmentConnectionState,
	handle: string,
	outcome: 'cancelled' | 'submitted',
): SshEnvironmentConnectionTransition {
	if (state.phase !== 'interaction-required' || state.handle !== handle) return unchanged(state);
	if (outcome === 'cancelled') return block(state, 'authentication-cancelled', true);
	const nextState: SshEnvironmentConnectionConnectingState = {
		connectedOnce: state.connectedOnce,
		connectionId: state.connectionId,
		generation: state.generation,
		phase: 'connecting',
		resolvedTarget: state.resolvedTarget,
		retryAttempt: state.retryAttempt,
		runtimeIdentity: state.runtimeIdentity,
		target: state.target,
	};
	return applied(nextState, []);
}

function runtimeReady(
	state: SshEnvironmentConnectionState,
	runtimeInstanceId: string,
	runtimeEpoch: string,
): SshEnvironmentConnectionTransition {
	if (state.phase !== 'connecting') return unchanged(state);
	const parsedInstanceId = SshEnvironmentRuntimeInstanceIdSchema.safeParse(runtimeInstanceId);
	const parsedEpoch = SshEnvironmentRuntimeEpochSchema.safeParse(runtimeEpoch);
	if (!parsedInstanceId.success || !parsedEpoch.success) {
		return block(state, 'policy-rejected', true);
	}
	const nextState: SshEnvironmentConnectionConnectedState = {
		connectedOnce: true,
		connectionId: state.connectionId,
		generation: state.generation,
		phase: 'connected',
		resolvedTarget: state.resolvedTarget,
		runtimeIdentity: {
			runtimeEpoch: parsedEpoch.data,
			runtimeInstanceId: parsedInstanceId.data,
		},
		target: state.target,
	};
	return applied(nextState, []);
}

function transportFailed(
	state: SshEnvironmentConnectionState,
	reason: SshEnvironmentTransportFailureReason,
): SshEnvironmentConnectionTransition {
	if (state.phase !== 'connecting' && state.phase !== 'interaction-required') {
		return unchanged(state);
	}
	if (
		reason === 'retryable' &&
		state.phase === 'connecting' &&
		state.connectedOnce &&
		state.retryAttempt > 0
	) {
		return scheduleRetry(state, state.retryAttempt + 1);
	}
	return block(state, reason === 'retryable' ? 'connection-failed' : reason, true);
}

function retryElapsed(state: SshEnvironmentConnectionState): SshEnvironmentConnectionTransition {
	if (state.phase !== 'retry-wait') return unchanged(state);
	const nextState: SshEnvironmentConnectionResolvingState = {
		connectedOnce: true,
		connectionId: state.connectionId,
		generation: state.generation,
		phase: 'resolving',
		retryAttempt: state.retryAttempt,
		runtimeIdentity: state.runtimeIdentity,
		target: state.target,
	};
	return applied(nextState, [resolveTargetCommand(nextState)]);
}

function scheduleRetry(
	state: SshEnvironmentConnectionConnectingState | SshEnvironmentConnectionConnectedState,
	retryAttempt: number,
): SshEnvironmentConnectionTransition {
	const generation = nextGeneration(state.generation);
	const maximumDelayMilliseconds = Math.min(
		MAXIMUM_RETRY_DELAY_MILLISECONDS,
		INITIAL_RETRY_MAXIMUM_DELAY_MILLISECONDS * 2 ** (retryAttempt - 1),
	);
	const minimumDelayMilliseconds = Math.ceil(maximumDelayMilliseconds / 2);
	const nextState: SshEnvironmentConnectionRetryWaitState = {
		connectedOnce: true,
		connectionId: state.connectionId,
		generation,
		maximumDelayMilliseconds,
		minimumDelayMilliseconds,
		phase: 'retry-wait',
		retryAttempt,
		runtimeIdentity: state.runtimeIdentity,
		target: state.target,
	};
	return applied(nextState, [
		retireGenerationCommand(state, state.generation),
		{
			connectionId: state.connectionId,
			generation,
			maximumDelayMilliseconds,
			minimumDelayMilliseconds,
			retryAttempt,
			type: 'schedule-retry',
		},
	]);
}

function block(
	state: SshEnvironmentConnectionState,
	reason: SshEnvironmentConnectionBlockedReason,
	retireGeneration: boolean,
): SshEnvironmentConnectionTransition {
	const nextState: SshEnvironmentConnectionBlockedState = {
		connectedOnce: state.connectedOnce,
		connectionId: state.connectionId,
		generation: state.generation,
		phase: 'blocked',
		reason,
		runtimeIdentity: state.runtimeIdentity,
		target: state.target,
	};
	return applied(
		nextState,
		retireGeneration ? [retireGenerationCommand(state, state.generation)] : [],
	);
}

function resolveTargetCommand(
	state: SshEnvironmentConnectionResolvingState,
): SshEnvironmentConnectionCommand {
	return {
		connectionId: state.connectionId,
		generation: state.generation,
		target: state.target,
		type: 'resolve-target',
	};
}

function retireGenerationCommand(
	state: SshEnvironmentConnectionState,
	generation: number,
): SshEnvironmentConnectionCommand {
	return { connectionId: state.connectionId, generation, type: 'retire-generation' };
}

function nextGeneration(generation: number): number {
	if (
		!Number.isSafeInteger(generation) ||
		generation < 0 ||
		generation >= Number.MAX_SAFE_INTEGER
	) {
		throw new Error('SSH Environment connection generation is exhausted.');
	}
	return generation + 1;
}

function applied(
	state: SshEnvironmentConnectionState,
	commands: readonly SshEnvironmentConnectionCommand[],
): SshEnvironmentConnectionTransition {
	return { commands, outcome: 'applied', state };
}

function unchanged(state: SshEnvironmentConnectionState): SshEnvironmentConnectionTransition {
	return { commands: [], outcome: 'unchanged', state };
}
