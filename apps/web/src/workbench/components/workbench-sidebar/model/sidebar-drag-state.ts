import type {
	SidebarDragItem,
	SidebarDropResolution,
	SidebarNavigationMove,
} from './sidebar-drag-destination';

/** A press becomes a drag once the pointer has moved this far, in CSS pixels. */
const DRAG_LATCH_DISTANCE = 4;

export type SidebarDragState =
	| { readonly kind: 'idle' }
	/** Pressed, not yet moved far enough: a release here is a click. */
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly item: SidebarDragItem;
			readonly kind: 'armed';
			readonly pointerId: number;
	  }
	/** Latched: `shown` is the move the list shows, or null for the canonical order. */
	| {
			readonly dropAreaHovered: boolean;
			readonly item: SidebarDragItem;
			readonly kind: 'dragging';
			readonly pointerId: number;
			readonly shown: SidebarNavigationMove | null;
	  }
	/** Released, and shown where it was dropped until the move it asked for settles. */
	| {
			readonly item: SidebarDragItem;
			readonly kind: 'settling';
			readonly shown: SidebarNavigationMove;
	  };

type SidebarDragEvent =
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly item: SidebarDragItem;
			readonly pointerId: number;
			readonly type: 'pointer-armed';
	  }
	| {
			readonly clientX: number;
			readonly clientY: number;
			readonly pointerId: number;
			readonly type: 'pointer-moved';
	  }
	| {
			readonly dropAreaHovered: boolean;
			readonly resolution: SidebarDropResolution;
			readonly type: 'destination-resolved';
	  }
	/** `outside` is a release beyond the list, which keeps the canonical order. */
	| { readonly outside: boolean; readonly pointerId: number; readonly type: 'pointer-released' }
	/** Esc, a lost pointer or focus, a resize, or a changed canonical order. */
	| { readonly type: 'cancelled' }
	| { readonly type: 'move-settled' };

type SidebarDragCommand =
	/** A press just became a drag: a held Workspace becomes current until it lands (ORC-324). */
	| { readonly item: SidebarDragItem; readonly type: 'drag-started' }
	| { readonly move: SidebarNavigationMove; readonly type: 'commit-move' };

interface SidebarDragTransition {
	readonly command: SidebarDragCommand | null;
	readonly state: SidebarDragState;
}

const IDLE: SidebarDragState = Object.freeze({ kind: 'idle' });

export function createSidebarDragState(): SidebarDragState {
	return IDLE;
}

export function transitionSidebarDrag(
	state: SidebarDragState,
	event: SidebarDragEvent,
): SidebarDragTransition {
	const unchanged = { command: null, state };
	switch (event.type) {
		case 'pointer-armed':
			return state.kind === 'idle'
				? {
						command: null,
						state: {
							clientX: event.clientX,
							clientY: event.clientY,
							item: event.item,
							kind: 'armed',
							pointerId: event.pointerId,
						},
					}
				: unchanged;
		case 'pointer-moved': {
			if (state.kind !== 'armed' || state.pointerId !== event.pointerId) return unchanged;
			const deltaX = event.clientX - state.clientX;
			const deltaY = event.clientY - state.clientY;
			if (deltaX * deltaX + deltaY * deltaY < DRAG_LATCH_DISTANCE * DRAG_LATCH_DISTANCE) {
				return unchanged;
			}
			return {
				command: { item: state.item, type: 'drag-started' },
				state: {
					dropAreaHovered: false,
					item: state.item,
					kind: 'dragging',
					pointerId: state.pointerId,
					shown: null,
				},
			};
		}
		case 'destination-resolved': {
			if (state.kind !== 'dragging') return unchanged;
			const { resolution } = event;
			let shown = state.shown;
			if (resolution.kind === 'origin') shown = null;
			else if (resolution.kind === 'moved') shown = resolution.move;
			return shown === state.shown && event.dropAreaHovered === state.dropAreaHovered
				? unchanged
				: { command: null, state: { ...state, dropAreaHovered: event.dropAreaHovered, shown } };
		}
		case 'pointer-released':
			if (state.kind === 'armed' && state.pointerId === event.pointerId) {
				return { command: null, state: IDLE };
			}
			if (state.kind !== 'dragging' || state.pointerId !== event.pointerId) return unchanged;
			return state.shown === null || event.outside
				? { command: null, state: IDLE }
				: {
						command: { move: state.shown, type: 'commit-move' },
						state: { item: state.item, kind: 'settling', shown: state.shown },
					};
		case 'cancelled':
			return state.kind === 'armed' || state.kind === 'dragging'
				? { command: null, state: IDLE }
				: unchanged;
		case 'move-settled':
			return state.kind === 'settling' ? { command: null, state: IDLE } : unchanged;
	}
}
