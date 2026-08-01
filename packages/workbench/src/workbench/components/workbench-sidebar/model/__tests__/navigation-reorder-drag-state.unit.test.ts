import { describe, expect, it } from 'vitest';

import {
	createNavigationReorderDragState,
	transitionNavigationReorderDrag,
} from '../navigation-reorder-drag-state';

describe('navigation-reorder-drag-state', () => {
	it('arms without changing order and activates at the four-pixel boundary', () => {
		const armed = transitionNavigationReorderDrag(createNavigationReorderDragState(), {
			clientX: 10,
			clientY: 20,
			itemId: 'item-1',
			pointerId: 7,
			type: 'pointer-armed',
		});
		expect(armed).toMatchObject({ command: null, state: { kind: 'armed' } });

		const belowThreshold = transitionNavigationReorderDrag(armed.state, {
			beforeItemId: 'item-2',
			clientX: 13,
			clientY: 22,
			pointerId: 7,
			type: 'pointer-moved',
		});
		expect(belowThreshold).toEqual({ command: null, state: armed.state });

		const activated = transitionNavigationReorderDrag(armed.state, {
			beforeItemId: 'item-2',
			clientX: 14,
			clientY: 20,
			pointerId: 7,
			type: 'pointer-moved',
		});
		expect(activated).toEqual({
			command: null,
			state: {
				beforeItemId: 'item-2',
				itemId: 'item-1',
				kind: 'dragging',
				pointerId: 7,
			},
		});
	});

	it('updates the reversible target and emits one commit only on matching release', () => {
		const dragging = {
			beforeItemId: 'item-2',
			itemId: 'item-1',
			kind: 'dragging' as const,
			pointerId: 7,
		};
		const moved = transitionNavigationReorderDrag(dragging, {
			beforeItemId: null,
			clientX: 10,
			clientY: 80,
			pointerId: 7,
			type: 'pointer-moved',
		});
		expect(moved.state).toMatchObject({ beforeItemId: null, kind: 'dragging' });

		expect(
			transitionNavigationReorderDrag(moved.state, {
				pointerId: 99,
				type: 'pointer-released',
			}),
		).toEqual({ command: null, state: moved.state });
		expect(
			transitionNavigationReorderDrag(moved.state, { pointerId: 7, type: 'pointer-released' }),
		).toEqual({
			command: {
				beforeItemId: null,
				itemId: 'item-1',
				type: 'commit-navigation-reorder',
			},
			state: { kind: 'idle' },
		});
	});

	it('treats release before activation as an ordinary click', () => {
		const armed = transitionNavigationReorderDrag(createNavigationReorderDragState(), {
			clientX: 0,
			clientY: 0,
			itemId: 'item-1',
			pointerId: 1,
			type: 'pointer-armed',
		}).state;

		expect(
			transitionNavigationReorderDrag(armed, { pointerId: 1, type: 'pointer-released' }),
		).toEqual({ command: null, state: { kind: 'idle' } });
	});

	it.each([
		{ pointerId: 7, type: 'pointer-cancelled' as const },
		{ type: 'escape-pressed' as const },
		{ type: 'item-invalidated' as const },
	])('cancels without a commit for $type', (event) => {
		const dragging = {
			beforeItemId: 'item-2',
			itemId: 'item-1',
			kind: 'dragging' as const,
			pointerId: 7,
		};

		expect(transitionNavigationReorderDrag(dragging, event)).toEqual({
			command: null,
			state: { kind: 'idle' },
		});
	});

	it('ignores a stale pointer cancellation', () => {
		const dragging = {
			beforeItemId: 'item-2',
			itemId: 'item-1',
			kind: 'dragging' as const,
			pointerId: 7,
		};

		expect(
			transitionNavigationReorderDrag(dragging, { pointerId: 9, type: 'pointer-cancelled' }),
		).toEqual({ command: null, state: dragging });
	});
});
