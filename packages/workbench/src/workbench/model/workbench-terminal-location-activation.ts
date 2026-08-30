import type { WorkbenchActiveLocation } from '../../project-workspace';

export type WorkbenchTerminalLocationActivationOrigin = 'other' | 'pointer';

interface WorkbenchTerminalLocationActivationWaitingState {
	readonly activationAccepted: boolean;
	readonly kind: 'waiting';
	readonly originLocation: WorkbenchActiveLocation;
	readonly requestId: number;
	readonly targetLocation: WorkbenchActiveLocation;
	readonly terminalProjectionReady: boolean;
}

export type WorkbenchTerminalLocationActivationState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'idle' }
	| WorkbenchTerminalLocationActivationWaitingState;

export type WorkbenchTerminalLocationActivationEvent =
	| {
			readonly activeLocation: WorkbenchActiveLocation;
			readonly kind: 'projection-observed';
			readonly terminalTargetAvailable: boolean;
	  }
	| {
			readonly kind: 'activate';
			readonly origin: WorkbenchTerminalLocationActivationOrigin;
			readonly originLocation: WorkbenchActiveLocation;
			readonly requestId: number;
			readonly targetLocation: WorkbenchActiveLocation;
	  }
	| {
			readonly kind: 'activation-settled';
			readonly outcome: 'accepted' | 'rejected';
			readonly requestId: number;
	  }
	| { readonly kind: 'dispose' }
	| { readonly kind: 'mount' };

export type WorkbenchTerminalLocationActivationCommand =
	| {
			readonly kind: 'invoke-location-activation';
			readonly requestId: number;
			readonly source: 'button' | 'pointer';
			readonly targetLocation: WorkbenchActiveLocation;
	  }
	| {
			readonly kind: 'request-terminal-focus';
			readonly targetLocation: WorkbenchActiveLocation;
	  };

export interface WorkbenchTerminalLocationActivationTransition {
	readonly commands: readonly WorkbenchTerminalLocationActivationCommand[];
	readonly state: WorkbenchTerminalLocationActivationState;
}

export const INITIAL_WORKBENCH_TERMINAL_LOCATION_ACTIVATION_STATE = {
	kind: 'disposed',
} as const satisfies WorkbenchTerminalLocationActivationState;

export function transitionWorkbenchTerminalLocationActivation(
	state: WorkbenchTerminalLocationActivationState,
	event: WorkbenchTerminalLocationActivationEvent,
): WorkbenchTerminalLocationActivationTransition {
	if (event.kind === 'mount') {
		return state.kind === 'disposed'
			? { commands: [], state: { kind: 'idle' } }
			: { commands: [], state };
	}
	if (event.kind === 'dispose') return { commands: [], state: { kind: 'disposed' } };
	if (state.kind === 'disposed') return { commands: [], state };

	if (event.kind === 'activate') {
		const command = {
			kind: 'invoke-location-activation',
			requestId: event.requestId,
			source: event.origin === 'pointer' ? 'pointer' : 'button',
			targetLocation: event.targetLocation,
		} as const;
		return event.origin === 'pointer'
			? {
					commands: [command],
					state: {
						activationAccepted: false,
						kind: 'waiting',
						originLocation: event.originLocation,
						requestId: event.requestId,
						targetLocation: event.targetLocation,
						terminalProjectionReady: false,
					},
				}
			: { commands: [command], state: { kind: 'idle' } };
	}

	if (state.kind !== 'waiting') return { commands: [], state };
	if (event.kind === 'activation-settled') {
		if (event.requestId !== state.requestId) return { commands: [], state };
		if (event.outcome === 'rejected') return { commands: [], state: { kind: 'idle' } };
		return state.terminalProjectionReady
			? focusTransition(state.targetLocation)
			: { commands: [], state: { ...state, activationAccepted: true } };
	}

	if (workbenchActiveLocationsEqual(event.activeLocation, state.targetLocation)) {
		if (!event.terminalTargetAvailable) {
			return {
				commands: [],
				state: { ...state, terminalProjectionReady: false },
			};
		}
		return state.activationAccepted
			? focusTransition(state.targetLocation)
			: { commands: [], state: { ...state, terminalProjectionReady: true } };
	}
	if (workbenchActiveLocationsEqual(event.activeLocation, state.originLocation)) {
		return {
			commands: [],
			state: { ...state, terminalProjectionReady: false },
		};
	}
	return { commands: [], state: { kind: 'idle' } };
}

function focusTransition(
	targetLocation: WorkbenchActiveLocation,
): WorkbenchTerminalLocationActivationTransition {
	return {
		commands: [{ kind: 'request-terminal-focus', targetLocation }],
		state: { kind: 'idle' },
	};
}

export function workbenchActiveLocationsEqual(
	left: WorkbenchActiveLocation,
	right: WorkbenchActiveLocation,
): boolean {
	if (left.kind !== right.kind) return false;
	if (left.kind === 'standalone' || right.kind === 'standalone') return true;
	return left.projectId === right.projectId && left.workspaceId === right.workspaceId;
}
