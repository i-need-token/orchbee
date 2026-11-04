export type WorkspaceCreationActivationState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'idle' }
	| { readonly kind: 'preparing'; readonly intent: number }
	| { readonly kind: 'waiting'; readonly intent: number; readonly workspaceId: string };

export type WorkspaceCreationActivationEvent =
	| { readonly kind: 'dispose' | 'mount' | 'navigation' }
	| { readonly kind: 'begin'; readonly intent: number }
	| { readonly kind: 'projection-observed'; readonly workspaceIds: readonly string[] }
	| { readonly kind: 'completed'; readonly intent: number; readonly workspaceId: string };

export interface WorkspaceCreationActivationCommand {
	readonly kind: 'activate-workspace';
	readonly workspaceId: string;
}

interface WorkspaceCreationActivationTransition {
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
	if (event.kind === 'mount')
		return { commands: [], state: state.kind === 'disposed' ? { kind: 'idle' } : state };
	if (event.kind === 'dispose') return { commands: [], state: { kind: 'disposed' } };
	if (state.kind === 'disposed') return { commands: [], state };
	if (event.kind === 'navigation') return { commands: [], state: { kind: 'idle' } };
	if (event.kind === 'begin')
		return { commands: [], state: { kind: 'preparing', intent: event.intent } };
	if (event.kind === 'completed') {
		return state.kind === 'preparing' && state.intent === event.intent
			? {
					commands: [],
					state: { kind: 'waiting', intent: event.intent, workspaceId: event.workspaceId },
				}
			: { commands: [], state };
	}
	if (
		event.kind !== 'projection-observed' ||
		state.kind !== 'waiting' ||
		!event.workspaceIds.includes(state.workspaceId)
	)
		return { commands: [], state };
	return {
		commands: [{ kind: 'activate-workspace', workspaceId: state.workspaceId }],
		state: { kind: 'idle' },
	};
}
