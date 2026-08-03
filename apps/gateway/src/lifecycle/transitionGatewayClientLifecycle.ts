import type { GatewayClientConnectionId } from '@orchbee/gateway-protocol';

export type GatewayClientLifecycleState =
	| { readonly phase: 'awaiting-authentication' }
	| { readonly connectionId: GatewayClientConnectionId; readonly phase: 'opening-client' }
	| { readonly connectionId: GatewayClientConnectionId; readonly phase: 'active' }
	| { readonly phase: 'closed' };

export type GatewayClientLifecycleEvent =
	| {
			readonly accepted: boolean;
			readonly connectionId: GatewayClientConnectionId;
			readonly type: 'authentication-completed';
	  }
	| { readonly connectionId: GatewayClientConnectionId; readonly type: 'desktop-client-opened' }
	| { readonly type: 'desktop-client-open-failed' }
	| { readonly type: 'protocol-rejected' }
	| { readonly type: 'socket-closed' }
	| { readonly type: 'gateway-stopping' };

export type GatewayClientLifecycleCommand =
	| { readonly connectionId: GatewayClientConnectionId; readonly type: 'request-client-open' }
	| { readonly connectionId: GatewayClientConnectionId; readonly type: 'request-client-close' }
	| { readonly type: 'accept-desktop-ports' }
	| { readonly type: 'dispose-desktop-ports' }
	| { readonly type: 'close-control-socket' };

export interface GatewayClientLifecycleTransition {
	readonly commands: readonly GatewayClientLifecycleCommand[];
	readonly state: GatewayClientLifecycleState;
}

export function transitionGatewayClientLifecycle(
	state: GatewayClientLifecycleState,
	event: GatewayClientLifecycleEvent,
): GatewayClientLifecycleTransition {
	if (state.phase === 'closed') {
		return event.type === 'desktop-client-opened'
			? { commands: [{ type: 'dispose-desktop-ports' }], state }
			: { commands: [], state };
	}
	if (event.type === 'protocol-rejected') {
		return closeClient(state, true);
	}
	if (event.type === 'socket-closed' || event.type === 'gateway-stopping') {
		return closeClient(state, event.type === 'gateway-stopping');
	}
	if (state.phase === 'awaiting-authentication') {
		if (event.type !== 'authentication-completed' || !event.accepted) {
			return {
				commands: [{ type: 'close-control-socket' }],
				state: { phase: 'closed' },
			};
		}
		return {
			commands: [{ connectionId: event.connectionId, type: 'request-client-open' }],
			state: { connectionId: event.connectionId, phase: 'opening-client' },
		};
	}
	if (state.phase === 'opening-client') {
		if (event.type === 'desktop-client-opened' && event.connectionId === state.connectionId) {
			return {
				commands: [{ type: 'accept-desktop-ports' }],
				state: { connectionId: state.connectionId, phase: 'active' },
			};
		}
		if (event.type === 'desktop-client-opened') {
			return {
				commands: [{ type: 'dispose-desktop-ports' }],
				state,
			};
		}
		if (event.type === 'desktop-client-open-failed') {
			return closeClient(state, true);
		}
	}
	return closeClient(state, true);
}

function closeClient(
	state: Exclude<GatewayClientLifecycleState, { readonly phase: 'closed' }>,
	closeSocket: boolean,
): GatewayClientLifecycleTransition {
	const commands: GatewayClientLifecycleCommand[] = [];
	if (state.phase === 'opening-client' || state.phase === 'active') {
		commands.push({ connectionId: state.connectionId, type: 'request-client-close' });
	}
	if (closeSocket) commands.push({ type: 'close-control-socket' });
	return { commands, state: { phase: 'closed' } };
}
