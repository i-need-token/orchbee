import { describe, expect, it } from 'vitest';

import type { SidebarNavigationMove } from '../sidebar-drag-destination';
import {
	createSidebarDragState,
	transitionSidebarDrag,
	type SidebarDragState,
} from '../sidebar-drag-state';

const ITEM = { id: 'workspace-1', kind: 'workspace' } as const;
const MOVE: SidebarNavigationMove = {
	beforeWorkspaceId: null,
	kind: 'move-workspace',
	projectId: null,
	workspaceId: 'workspace-1',
};

function armed(): SidebarDragState {
	return transitionSidebarDrag(createSidebarDragState(), {
		clientX: 10,
		clientY: 10,
		item: ITEM,
		pointerId: 1,
		type: 'pointer-armed',
	}).state;
}

function dragging(shown: SidebarNavigationMove | null): SidebarDragState {
	const latched = transitionSidebarDrag(armed(), {
		clientX: 10,
		clientY: 14,
		pointerId: 1,
		type: 'pointer-moved',
	}).state;
	return transitionSidebarDrag(latched, {
		dropAreaHovered: false,
		resolution: shown === null ? { kind: 'origin' } : { kind: 'moved', move: shown },
		type: 'destination-resolved',
	}).state;
}

describe('sidebar drag gesture', () => {
	it('latches at four pixels and announces the held entry once; a release before that is a click', () => {
		const pressed = armed();
		expect(
			transitionSidebarDrag(pressed, {
				clientX: 12,
				clientY: 12,
				pointerId: 1,
				type: 'pointer-moved',
			}).state,
		).toBe(pressed);
		const latched = transitionSidebarDrag(pressed, {
			clientX: 10,
			clientY: 14,
			pointerId: 1,
			type: 'pointer-moved',
		});
		expect(latched.command).toEqual({ item: ITEM, type: 'drag-started' });
		expect(latched.state).toMatchObject({ kind: 'dragging', shown: null });

		expect(
			transitionSidebarDrag(pressed, { outside: false, pointerId: 1, type: 'pointer-released' }),
		).toEqual({ command: null, state: createSidebarDragState() });
	});

	it('commits the shown move on release and holds it until the move settles', () => {
		const release = transitionSidebarDrag(dragging(MOVE), {
			outside: false,
			pointerId: 1,
			type: 'pointer-released',
		});
		expect(release.command).toEqual({ move: MOVE, type: 'commit-move' });
		expect(release.state).toEqual({ item: ITEM, kind: 'settling', shown: MOVE });
		expect(transitionSidebarDrag(release.state, { type: 'move-settled' }).state).toEqual(
			createSidebarDragState(),
		);
	});

	it('restores the canonical order without a commit on cancel, outside release, or no move', () => {
		expect(transitionSidebarDrag(dragging(MOVE), { type: 'cancelled' })).toEqual({
			command: null,
			state: createSidebarDragState(),
		});
		for (const [state, outside] of [
			[dragging(MOVE), true],
			[dragging(null), false],
		] as const) {
			expect(
				transitionSidebarDrag(state, { outside, pointerId: 1, type: 'pointer-released' }),
			).toEqual({ command: null, state: createSidebarDragState() });
		}
	});

	it('keeps the shown move where the destination resolves to keep', () => {
		const shown = dragging(MOVE);
		expect(
			transitionSidebarDrag(shown, {
				dropAreaHovered: false,
				resolution: { kind: 'keep' },
				type: 'destination-resolved',
			}).state,
		).toBe(shown);
	});
});
