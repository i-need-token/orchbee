interface WorkspaceCreationActivationTarget {
	readonly projectId: string;
	readonly workspaceId: string;
}

export type WorkspaceCreationActivationState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'idle' }
	| {
			readonly kind: 'waiting';
			readonly target: WorkspaceCreationActivationTarget;
	  };

export type WorkspaceCreationActivationEvent =
	| { readonly kind: 'dispose' }
	| { readonly kind: 'mount' }
	| {
			readonly kind: 'projection-observed';
			readonly targets: readonly WorkspaceCreationActivationTarget[];
	  }
	| {
			readonly kind: 'request';
			readonly target: WorkspaceCreationActivationTarget;
	  };

export interface WorkspaceCreationActivationCommand {
	readonly kind: 'activate-workspace';
	readonly target: WorkspaceCreationActivationTarget;
}

export interface WorkspaceCreationActivationTransition {
	readonly commands: readonly WorkspaceCreationActivationCommand[];
	readonly state: WorkspaceCreationActivationState;
}

export const INITIAL_WORKSPACE_CREATION_ACTIVATION_STATE = {
	kind: 'disposed',
} as const satisfies WorkspaceCreationActivationState;

export function transitionWorkspaceCreationActivation(
	state: WorkspaceCreationActivationState,
	event: WorkspaceCreationActivationEvent,
): WorkspaceCreationActivationTransition {
	if (event.kind === 'mount') {
		return state.kind === 'disposed'
			? { commands: [], state: { kind: 'idle' } }
			: { commands: [], state };
	}
	if (event.kind === 'dispose') return { commands: [], state: { kind: 'disposed' } };
	if (state.kind === 'disposed') return { commands: [], state };
	if (event.kind === 'request') {
		return state.kind === 'waiting' && targetsEqual(state.target, event.target)
			? { commands: [], state }
			: { commands: [], state: { kind: 'waiting', target: event.target } };
	}
	if (state.kind !== 'waiting') return { commands: [], state };
	if (!event.targets.some((target) => targetsEqual(target, state.target))) {
		return { commands: [], state };
	}
	return {
		commands: [{ kind: 'activate-workspace', target: state.target }],
		state: { kind: 'idle' },
	};
}

function targetsEqual(
	left: WorkspaceCreationActivationTarget,
	right: WorkspaceCreationActivationTarget,
): boolean {
	return left.projectId === right.projectId && left.workspaceId === right.workspaceId;
}
