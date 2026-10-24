export type NavigationReorderDragState =
	| { readonly kind: 'idle' }
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly itemId: string;
			readonly kind: 'armed';
			readonly pointerId: number;
	  }
	| {
			readonly beforeItemId: string | null;
			readonly itemId: string;
			readonly kind: 'dragging';
			readonly pointerId: number;
	  };

type NavigationReorderDragEvent =
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly itemId: string;
			readonly pointerId: number;
			readonly type: 'pointer-armed';
	  }
	| {
			readonly beforeItemId: string | null;
			readonly clientX: number;
			readonly clientY: number;
			readonly pointerId: number;
			readonly type: 'pointer-moved';
	  }
	| {
			readonly pointerId: number;
			readonly type: 'pointer-released';
	  }
	| {
			readonly pointerId: number;
			readonly type: 'pointer-cancelled';
	  }
	| { readonly type: 'escape-pressed' }
	| { readonly type: 'item-invalidated' };

interface NavigationReorderCommitCommand {
	readonly beforeItemId: string | null;
	readonly itemId: string;
	readonly type: 'commit-navigation-reorder';
}

interface NavigationReorderDragTransition {
	readonly command: NavigationReorderCommitCommand | null;
	readonly state: NavigationReorderDragState;
}

export function createNavigationReorderDragState(): NavigationReorderDragState {
	return { kind: 'idle' };
}

export function transitionNavigationReorderDrag(
	state: NavigationReorderDragState,
	event: NavigationReorderDragEvent,
): NavigationReorderDragTransition {
	if (event.type === 'escape-pressed' || event.type === 'item-invalidated') {
		return state.kind === 'idle'
			? { command: null, state }
			: { command: null, state: createNavigationReorderDragState() };
	}
	if (event.type === 'pointer-armed') {
		return state.kind === 'idle'
			? {
					command: null,
					state: {
						clientX: event.clientX,
						clientY: event.clientY,
						itemId: event.itemId,
						kind: 'armed',
						pointerId: event.pointerId,
					},
				}
			: { command: null, state };
	}
	if (state.kind === 'idle' || event.pointerId !== state.pointerId) {
		return { command: null, state };
	}
	if (event.type === 'pointer-cancelled') {
		return { command: null, state: createNavigationReorderDragState() };
	}
	if (event.type === 'pointer-released') {
		return state.kind === 'dragging'
			? {
					command: {
						beforeItemId: state.beforeItemId,
						itemId: state.itemId,
						type: 'commit-navigation-reorder',
					},
					state: createNavigationReorderDragState(),
				}
			: { command: null, state: createNavigationReorderDragState() };
	}
	if (state.kind === 'armed') {
		const deltaX = event.clientX - state.clientX;
		const deltaY = event.clientY - state.clientY;
		return deltaX * deltaX + deltaY * deltaY < 16
			? { command: null, state }
			: {
					command: null,
					state: {
						beforeItemId: event.beforeItemId,
						itemId: state.itemId,
						kind: 'dragging',
						pointerId: state.pointerId,
					},
				};
	}
	return state.beforeItemId === event.beforeItemId
		? { command: null, state }
		: { command: null, state: { ...state, beforeItemId: event.beforeItemId } };
}
