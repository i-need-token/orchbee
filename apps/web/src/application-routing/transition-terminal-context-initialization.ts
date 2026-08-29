export type TerminalContextInitializationState = readonly string[];

export type TerminalContextInitializationEvent =
	| { readonly type: 'redirect-observed' }
	| { readonly type: 'route-unavailable' }
	| {
			readonly hasTabs: boolean;
			readonly terminalContextId: string;
			readonly type: 'terminal-route-ready';
	  };

type TerminalContextInitializationCommand = {
	readonly terminalContextId: string;
	readonly type: 'create-terminal';
};

export interface TerminalContextInitializationTransition {
	readonly commands: readonly TerminalContextInitializationCommand[];
	readonly state: TerminalContextInitializationState;
}

export const EMPTY_TERMINAL_CONTEXT_INITIALIZATION_STATE: TerminalContextInitializationState =
	Object.freeze([]);

export function transitionTerminalContextInitialization(
	state: TerminalContextInitializationState,
	event: TerminalContextInitializationEvent,
): TerminalContextInitializationTransition {
	if (event.type !== 'terminal-route-ready') return { commands: [], state };
	if (state.includes(event.terminalContextId)) return { commands: [], state };

	const nextState = Object.freeze([...state, event.terminalContextId]);
	return {
		commands: event.hasTabs
			? []
			: [{ terminalContextId: event.terminalContextId, type: 'create-terminal' }],
		state: nextState,
	};
}
