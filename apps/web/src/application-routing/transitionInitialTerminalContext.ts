export type InitialTerminalContextState = 'awaiting-route' | 'settled';

export type InitialTerminalContextEvent =
	| { readonly type: 'redirect-observed' }
	| { readonly type: 'route-unavailable' }
	| {
			readonly hasTabs: boolean;
			readonly type: 'terminal-route-ready';
	  };

type InitialTerminalContextCommand = { readonly type: 'create-terminal' };

export interface InitialTerminalContextTransition {
	readonly commands: readonly InitialTerminalContextCommand[];
	readonly state: InitialTerminalContextState;
}

export function transitionInitialTerminalContext(
	state: InitialTerminalContextState,
	event: InitialTerminalContextEvent,
): InitialTerminalContextTransition {
	if (state === 'settled') return { commands: [], state };

	switch (event.type) {
		case 'redirect-observed':
			return { commands: [], state };
		case 'route-unavailable':
			return { commands: [], state: 'settled' };
		case 'terminal-route-ready':
			return {
				commands: event.hasTabs ? [] : [{ type: 'create-terminal' }],
				state: 'settled',
			};
	}
}
