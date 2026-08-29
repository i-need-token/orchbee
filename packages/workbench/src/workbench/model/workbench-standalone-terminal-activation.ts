import type { WorkbenchActiveLocation } from '../../project-workspace';

export type WorkbenchStandaloneTerminalActivationOrigin = 'other' | 'pointer';

interface WorkbenchStandaloneTerminalActivationWaitingState {
	readonly activationAccepted: boolean;
	readonly kind: 'waiting';
	readonly originLocation: WorkbenchActiveLocation;
	readonly requestId: number;
	readonly terminalProjectionReady: boolean;
}

export type WorkbenchStandaloneTerminalActivationState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'idle' }
	| WorkbenchStandaloneTerminalActivationWaitingState;

export type WorkbenchStandaloneTerminalActivationEvent =
	| {
			readonly activeLocation: WorkbenchActiveLocation;
			readonly kind: 'projection-observed';
			readonly terminalTargetAvailable: boolean;
	  }
	| {
			readonly kind: 'activate';
			readonly origin: WorkbenchStandaloneTerminalActivationOrigin;
			readonly originLocation: WorkbenchActiveLocation;
			readonly requestId: number;
	  }
	| {
			readonly kind: 'activation-settled';
			readonly outcome: 'accepted' | 'rejected';
			readonly requestId: number;
	  }
	| { readonly kind: 'dispose' }
	| { readonly kind: 'mount' };

export type WorkbenchStandaloneTerminalActivationCommand =
	| {
			readonly kind: 'invoke-standalone-activation';
			readonly requestId: number;
			readonly source: 'button' | 'pointer';
	  }
	| { readonly kind: 'request-terminal-focus' };

export interface WorkbenchStandaloneTerminalActivationTransition {
	readonly commands: readonly WorkbenchStandaloneTerminalActivationCommand[];
	readonly state: WorkbenchStandaloneTerminalActivationState;
}

export const INITIAL_WORKBENCH_STANDALONE_TERMINAL_ACTIVATION_STATE = {
	kind: 'disposed',
} as const satisfies WorkbenchStandaloneTerminalActivationState;

export function transitionWorkbenchStandaloneTerminalActivation(
	state: WorkbenchStandaloneTerminalActivationState,
	event: WorkbenchStandaloneTerminalActivationEvent,
): WorkbenchStandaloneTerminalActivationTransition {
	if (event.kind === 'mount') {
		return state.kind === 'disposed'
			? { commands: [], state: { kind: 'idle' } }
			: { commands: [], state };
	}
	if (event.kind === 'dispose') return { commands: [], state: { kind: 'disposed' } };
	if (state.kind === 'disposed') return { commands: [], state };

	if (event.kind === 'activate') {
		const command = {
			kind: 'invoke-standalone-activation',
			requestId: event.requestId,
			source: event.origin === 'pointer' ? 'pointer' : 'button',
		} as const;
		return event.origin === 'pointer'
			? {
					commands: [command],
					state: {
						activationAccepted: false,
						kind: 'waiting',
						originLocation: event.originLocation,
						requestId: event.requestId,
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
			? focusTransition()
			: { commands: [], state: { ...state, activationAccepted: true } };
	}

	if (event.activeLocation.kind === 'standalone') {
		if (!event.terminalTargetAvailable) {
			return {
				commands: [],
				state: { ...state, terminalProjectionReady: false },
			};
		}
		return state.activationAccepted
			? focusTransition()
			: { commands: [], state: { ...state, terminalProjectionReady: true } };
	}
	if (locationsEqual(event.activeLocation, state.originLocation)) {
		return {
			commands: [],
			state: { ...state, terminalProjectionReady: false },
		};
	}
	return { commands: [], state: { kind: 'idle' } };
}

function focusTransition(): WorkbenchStandaloneTerminalActivationTransition {
	return {
		commands: [{ kind: 'request-terminal-focus' }],
		state: { kind: 'idle' },
	};
}

function locationsEqual(left: WorkbenchActiveLocation, right: WorkbenchActiveLocation): boolean {
	if (left.kind !== right.kind) return false;
	if (left.kind === 'standalone' || right.kind === 'standalone') return true;
	return left.projectId === right.projectId && left.workspaceId === right.workspaceId;
}
