import { describe, expect, it } from 'vitest';

import {
	createTerminalTouchGestureState,
	transitionTerminalTouchGesture,
	type TerminalTouchGestureState,
} from '../terminal-touch-gesture';

describe('terminal touch gesture', () => {
	it.each([
		{ distance: 8 - 1, expected: 'pending-tap' },
		{ distance: 8, expected: 'scrolling' },
		{ distance: 8 + 1, expected: 'scrolling' },
	] as const)('classifies vertical movement at $distance CSS pixels', ({ distance, expected }) => {
		const state = begin(4, 20, 30);
		const result = transitionTerminalTouchGesture(state, {
			clientX: 20,
			clientY: 30 + distance,
			kind: 'pointer-move',
			pointerId: 4,
		});

		expect(result.state.kind).toBe(expected);
		expect(result.commands).toEqual(
			expected === 'scrolling' ? [{ deltaY: -distance, kind: 'scroll' }] : [],
		);
	});

	it('cancels an axis tie and a horizontally dominant gesture without a tap', () => {
		for (const [clientX, clientY] of [
			[18, 18],
			[20, 14],
		] as const) {
			const result = transitionTerminalTouchGesture(begin(9, 10, 10), {
				clientX,
				clientY,
				kind: 'pointer-move',
				pointerId: 9,
			});
			expect(result).toEqual({
				commands: [{ kind: 'release-pointer', pointerId: 9 }],
				state: { kind: 'idle' },
			});
		}
	});

	it('emits natural scroll deltas from each accepted movement', () => {
		const upward = transitionTerminalTouchGesture(begin(2, 0, 20), {
			clientX: 0,
			clientY: 10,
			kind: 'pointer-move',
			pointerId: 2,
		});
		expect(upward).toEqual({
			commands: [{ deltaY: 10, kind: 'scroll' }],
			state: { kind: 'scrolling', lastY: 10, pointerId: 2 },
		});
		expect(
			transitionTerminalTouchGesture(upward.state, {
				clientX: 0,
				clientY: 14,
				kind: 'pointer-move',
				pointerId: 2,
			}).commands,
		).toEqual([{ deltaY: -4, kind: 'scroll' }]);
	});

	it('completes a pending gesture as one final-position tap', () => {
		const result = transitionTerminalTouchGesture(begin(7, 10, 10), {
			clientX: 14,
			clientY: 12,
			kind: 'pointer-up',
			pointerId: 7,
		});

		expect(result).toEqual({
			commands: [
				{ clientX: 14, clientY: 12, kind: 'tap' },
				{ kind: 'release-pointer', pointerId: 7 },
			],
			state: { kind: 'idle' },
		});
	});

	it.each(['explicit', 'stale', 'replacement'] as const)(
		'cancels active ownership for %s cancellation',
		(kind) => {
			const state = begin(3, 1, 1);
			let pointerId: number | undefined;
			if (kind === 'stale') pointerId = 99;
			else if (kind === 'explicit') pointerId = 3;
			const result = transitionTerminalTouchGesture(state, { kind: 'cancel', pointerId });

			expect(result).toEqual(
				kind === 'stale'
					? { commands: [], state }
					: {
							commands: [{ kind: 'release-pointer', pointerId: 3 }],
							state: { kind: 'idle' },
						},
			);
		},
	);

	it('cancels the owned gesture when an additional touch arrives', () => {
		const result = transitionTerminalTouchGesture(begin(1, 0, 0), {
			clientX: 5,
			clientY: 6,
			kind: 'pointer-down',
			pointerId: 2,
		});

		expect(result).toEqual({
			commands: [{ kind: 'release-pointer', pointerId: 1 }],
			state: { kind: 'idle' },
		});
	});

	it('rejects non-finite geometry without changing idle state', () => {
		expect(
			transitionTerminalTouchGesture(createTerminalTouchGestureState(), {
				clientX: Number.NaN,
				clientY: 0,
				kind: 'pointer-down',
				pointerId: 1,
			}),
		).toEqual({ commands: [], state: { kind: 'idle' } });
	});
});

function begin(pointerId: number, clientX: number, clientY: number): TerminalTouchGestureState {
	return transitionTerminalTouchGesture(createTerminalTouchGestureState(), {
		clientX,
		clientY,
		kind: 'pointer-down',
		pointerId,
	}).state;
}
