const TERMINAL_TOUCH_ACTIVATION_DISTANCE_PX = 8;

export type TerminalTouchGestureState =
	| { readonly kind: 'idle' }
	| {
			readonly kind: 'pending-tap';
			readonly lastX: number;
			readonly lastY: number;
			readonly pointerId: number;
			readonly startX: number;
			readonly startY: number;
	  }
	| {
			readonly kind: 'scrolling';
			readonly lastY: number;
			readonly pointerId: number;
	  };

export type TerminalTouchGestureEvent =
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly kind: 'pointer-down';
			readonly pointerId: number;
	  }
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly kind: 'pointer-move';
			readonly pointerId: number;
	  }
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly kind: 'pointer-up';
			readonly pointerId: number;
	  }
	| { readonly kind: 'cancel'; readonly pointerId?: number | undefined };

export type TerminalTouchGestureCommand =
	| { readonly kind: 'capture-pointer'; readonly pointerId: number }
	| { readonly kind: 'release-pointer'; readonly pointerId: number }
	| { readonly clientX: number; readonly clientY: number; readonly kind: 'tap' }
	| { readonly deltaY: number; readonly kind: 'scroll' };

export interface TerminalTouchGestureTransition {
	readonly commands: readonly TerminalTouchGestureCommand[];
	readonly state: TerminalTouchGestureState;
}

const idleState: TerminalTouchGestureState = { kind: 'idle' };

export function createTerminalTouchGestureState(): TerminalTouchGestureState {
	return idleState;
}

export function transitionTerminalTouchGesture(
	state: TerminalTouchGestureState,
	event: TerminalTouchGestureEvent,
): TerminalTouchGestureTransition {
	if (event.kind === 'pointer-down') return beginTouchGesture(state, event);
	if (event.kind === 'cancel') return cancelTouchGesture(state, event.pointerId);
	if (state.kind === 'idle' || state.pointerId !== event.pointerId) {
		return { commands: [], state };
	}
	if (!finitePoint(event.clientX, event.clientY)) return cancelTouchGesture(state);
	if (event.kind === 'pointer-up') {
		return state.kind === 'pending-tap'
			? {
					commands: [
						{ clientX: event.clientX, clientY: event.clientY, kind: 'tap' },
						{ kind: 'release-pointer', pointerId: state.pointerId },
					],
					state: idleState,
				}
			: {
					commands: [{ kind: 'release-pointer', pointerId: state.pointerId }],
					state: idleState,
				};
	}
	if (state.kind === 'scrolling') {
		return {
			commands: [{ deltaY: state.lastY - event.clientY, kind: 'scroll' }],
			state: { kind: 'scrolling', lastY: event.clientY, pointerId: state.pointerId },
		};
	}
	const displacementX = event.clientX - state.startX;
	const displacementY = event.clientY - state.startY;
	const absoluteX = Math.abs(displacementX);
	const absoluteY = Math.abs(displacementY);
	if (
		absoluteX < TERMINAL_TOUCH_ACTIVATION_DISTANCE_PX &&
		absoluteY < TERMINAL_TOUCH_ACTIVATION_DISTANCE_PX
	) {
		return {
			commands: [],
			state: { ...state, lastX: event.clientX, lastY: event.clientY },
		};
	}
	if (absoluteY <= absoluteX) return cancelTouchGesture(state);
	return {
		commands: [{ deltaY: state.startY - event.clientY, kind: 'scroll' }],
		state: { kind: 'scrolling', lastY: event.clientY, pointerId: state.pointerId },
	};
}

function beginTouchGesture(
	state: TerminalTouchGestureState,
	event: Extract<TerminalTouchGestureEvent, { readonly kind: 'pointer-down' }>,
): TerminalTouchGestureTransition {
	if (!finitePoint(event.clientX, event.clientY)) return cancelTouchGesture(state);
	if (state.kind !== 'idle') return cancelTouchGesture(state);
	return {
		commands: [{ kind: 'capture-pointer', pointerId: event.pointerId }],
		state: {
			kind: 'pending-tap',
			lastX: event.clientX,
			lastY: event.clientY,
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
		},
	};
}

function cancelTouchGesture(
	state: TerminalTouchGestureState,
	pointerId?: number | undefined,
): TerminalTouchGestureTransition {
	if (state.kind === 'idle' || (pointerId !== undefined && state.pointerId !== pointerId)) {
		return { commands: [], state };
	}
	return {
		commands: [{ kind: 'release-pointer', pointerId: state.pointerId }],
		state: idleState,
	};
}

function finitePoint(clientX: number, clientY: number): boolean {
	return Number.isFinite(clientX) && Number.isFinite(clientY);
}
