import { describe, expect, it } from 'vitest';

import {
	applySidebarNavigationMove,
	placesAfter,
	resolveSidebarDrop,
	type SidebarDragHit,
	type SidebarDragItem,
	type SidebarNavigationMove,
} from '../sidebar-drag-destination';

// Projects A, B and an empty C; A holds a1 and a2, B holds b1 and b2, and r1 is outside any
// Project. The global order interleaves the groups, as the Server's may.
const ORDER = {
	projects: [{ id: 'A' }, { id: 'B' }, { id: 'C' }],
	workspaces: [
		{ id: 'a1', projectId: 'A' },
		{ id: 'b1', projectId: 'B' },
		{ id: 'a2', projectId: 'A' },
		{ id: 'r1', projectId: null },
		{ id: 'b2', projectId: 'B' },
	],
} as const;

const project = (id: string): SidebarDragItem => ({ id, kind: 'project' });
const workspace = (id: string): SidebarDragItem => ({ id, kind: 'workspace' });

/** The entries a Workspace drag sees, all groups expanded; Recents' heading is no entry. */
const WORKSPACE_ENTRIES = [
	project('A'),
	workspace('a1'),
	workspace('a2'),
	project('B'),
	workspace('b1'),
	workspace('b2'),
	project('C'),
	workspace('r1'),
];
const PROJECT_ENTRIES = [project('A'), project('B'), project('C')];

const entryAt = (entries: readonly SidebarDragItem[], item: SidebarDragItem, after: boolean) => {
	const index = entries.findIndex((entry) => entry.kind === item.kind && entry.id === item.id);
	return { after, index, kind: 'entry' } as const satisfies SidebarDragHit;
};

/** Each group's members in the order the sidebar lists them. */
function groups(move: SidebarNavigationMove): Record<string, readonly string[]> {
	const result = applySidebarNavigationMove(ORDER, move);
	if (result.kind !== 'moved') throw new Error(`Expected a move, got ${result.kind}.`);
	const listed: Record<string, string[]> = {};
	for (const { id, projectId } of result.order.workspaces) {
		(listed[projectId ?? 'none'] ??= []).push(id);
	}
	return listed;
}

function projectOrder(move: SidebarNavigationMove): readonly string[] {
	const result = applySidebarNavigationMove(ORDER, move);
	if (result.kind !== 'moved') throw new Error(`Expected a move, got ${result.kind}.`);
	return result.order.projects.map((candidate) => candidate.id);
}

function moved(
	held: SidebarDragItem,
	entries: readonly SidebarDragItem[],
	hit: SidebarDragHit,
): SidebarNavigationMove {
	const resolution = resolveSidebarDrop(ORDER, held, entries, hit);
	if (resolution.kind !== 'moved') throw new Error(`Expected a move, got ${resolution.kind}.`);
	return resolution.move;
}

describe('sidebar drag destinations', () => {
	it('places a Workspace before or after a Workspace of another Project, joining that Project', () => {
		const before = moved(
			workspace('a1'),
			WORKSPACE_ENTRIES,
			entryAt(WORKSPACE_ENTRIES, workspace('b2'), false),
		);
		expect(before).toEqual({
			beforeWorkspaceId: 'b2',
			kind: 'move-workspace',
			projectId: 'B',
			workspaceId: 'a1',
		});
		expect(groups(before)).toMatchObject({ A: ['a2'], B: ['b1', 'a1', 'b2'] });

		const after = moved(
			workspace('a1'),
			WORKSPACE_ENTRIES,
			entryAt(WORKSPACE_ENTRIES, workspace('b2'), true),
		);
		expect(after).toMatchObject({ beforeWorkspaceId: null, projectId: 'B' });
		expect(groups(after)).toMatchObject({ A: ['a2'], B: ['b1', 'b2', 'a1'] });

		// A Workspace outside any Project is a destination too: the held one leaves its Project.
		expect(
			groups(
				moved(
					workspace('b1'),
					WORKSPACE_ENTRIES,
					entryAt(WORKSPACE_ENTRIES, workspace('r1'), false),
				),
			),
		).toMatchObject({ B: ['b2'], none: ['b1', 'r1'] });
	});

	it('refills only the destination group, so every other group keeps its exact positions', () => {
		const result = applySidebarNavigationMove(ORDER, {
			beforeWorkspaceId: 'b1',
			kind: 'move-workspace',
			projectId: 'B',
			workspaceId: 'a2',
		});
		expect(result.kind === 'moved' && result.order.workspaces.map(({ id }) => id)).toEqual([
			'a1',
			'a2',
			'b1',
			'r1',
			'b2',
		]);
	});

	it('takes a Workspace in at the end through a heading with no other Workspace under it', () => {
		const into = moved(
			workspace('a1'),
			WORKSPACE_ENTRIES,
			entryAt(WORKSPACE_ENTRIES, project('C'), true),
		);
		expect(into).toEqual({
			beforeWorkspaceId: null,
			kind: 'move-workspace',
			projectId: 'C',
			workspaceId: 'a1',
		});
		// A folded Project shows no Workspace under its heading, so it takes the Workspace in too.
		const folded = [project('A'), workspace('a1'), workspace('a2'), project('B'), project('C')];
		expect(
			groups(moved(workspace('a1'), folded, entryAt(folded, project('B'), true))),
		).toMatchObject({
			B: ['b1', 'b2', 'a1'],
		});
	});

	it('makes a heading the top edge of a listed Project: first from above, after the Workspace above from below', () => {
		// Reached from above, the heading puts the Workspace first in its Project.
		const first = moved(
			workspace('a1'),
			WORKSPACE_ENTRIES,
			entryAt(WORKSPACE_ENTRIES, project('B'), true),
		);
		expect(groups(first)).toMatchObject({ A: ['a2'], B: ['a1', 'b1', 'b2'] });
		// Reached from below, the heading puts it after the Workspace shown just above it.
		const above = moved(
			workspace('b2'),
			WORKSPACE_ENTRIES,
			entryAt(WORKSPACE_ENTRIES, project('B'), false),
		);
		expect(groups(above)).toMatchObject({ A: ['a1', 'a2', 'b2'], B: ['b1'] });
		// When that Workspace is the held one, the shown move stays.
		const heldAbove = [
			project('A'),
			workspace('a1'),
			workspace('b2'),
			project('B'),
			workspace('b1'),
		];
		expect(
			resolveSidebarDrop(
				ORDER,
				workspace('b2'),
				heldAbove,
				entryAt(heldAbove, project('B'), false),
			),
		).toEqual({ kind: 'keep' });
	});

	it('moves a Workspace out of every Project through the drop area, to the end of that list', () => {
		const out = moved(workspace('a2'), WORKSPACE_ENTRIES, { kind: 'drop-area' });
		expect(out).toEqual({
			beforeWorkspaceId: null,
			kind: 'move-workspace',
			projectId: null,
			workspaceId: 'a2',
		});
		expect(groups(out)).toMatchObject({ A: ['a1'], none: ['r1', 'a2'] });
	});

	it('reorders Projects on headings and makes a Project last through the drop area', () => {
		expect(
			projectOrder(
				moved(project('C'), PROJECT_ENTRIES, entryAt(PROJECT_ENTRIES, project('A'), false)),
			),
		).toEqual(['C', 'A', 'B']);
		expect(
			projectOrder(
				moved(project('A'), PROJECT_ENTRIES, entryAt(PROJECT_ENTRIES, project('B'), true)),
			),
		).toEqual(['B', 'A', 'C']);
		expect(projectOrder(moved(project('A'), PROJECT_ENTRIES, { kind: 'drop-area' }))).toEqual([
			'B',
			'C',
			'A',
		]);
	});

	it('keeps the shown move where no drop is legal, and shows the canonical order outside the list', () => {
		// A Project never lands on a Workspace, nor anywhere it already stands.
		expect(
			resolveSidebarDrop(
				ORDER,
				project('A'),
				WORKSPACE_ENTRIES,
				entryAt(WORKSPACE_ENTRIES, workspace('b1'), false),
			),
		).toEqual({ kind: 'keep' });
		for (const hit of [
			{ kind: 'gap' },
			entryAt(WORKSPACE_ENTRIES, workspace('a1'), true),
		] satisfies SidebarDragHit[]) {
			expect(resolveSidebarDrop(ORDER, workspace('a1'), WORKSPACE_ENTRIES, hit)).toEqual({
				kind: 'keep',
			});
		}
		expect(
			resolveSidebarDrop(ORDER, workspace('a1'), WORKSPACE_ENTRIES, { kind: 'outside' }),
		).toEqual({
			kind: 'origin',
		});
		// A destination equal to the canonical place shows the canonical order.
		expect(
			resolveSidebarDrop(
				ORDER,
				workspace('a1'),
				WORKSPACE_ENTRIES,
				entryAt(WORKSPACE_ENTRIES, workspace('a2'), false),
			),
		).toEqual({ kind: 'origin' });
		expect(resolveSidebarDrop(ORDER, project('C'), PROJECT_ENTRIES, { kind: 'drop-area' })).toEqual(
			{
				kind: 'origin',
			},
		);
	});

	it('refuses an anchor outside the destination group, as the Server does', () => {
		expect(
			applySidebarNavigationMove(ORDER, {
				beforeWorkspaceId: 'b1',
				kind: 'move-workspace',
				projectId: 'A',
				workspaceId: 'a1',
			}),
		).toEqual({ kind: 'unavailable' });
		expect(
			applySidebarNavigationMove(ORDER, {
				beforeWorkspaceId: null,
				kind: 'move-workspace',
				projectId: 'missing',
				workspaceId: 'a1',
			}),
		).toEqual({ kind: 'unavailable' });
	});

	it('places after from a Workspace entry’s lower half, and from a heading’s side away from the held entry', () => {
		const entry = { height: 40, top: 100 };
		expect(placesAfter(entry, 119, { belowHeld: true, heading: false })).toBe(false);
		expect(placesAfter(entry, 120, { belowHeld: false, heading: false })).toBe(true);
		expect(placesAfter(entry, 101, { belowHeld: true, heading: true })).toBe(true);
		expect(placesAfter(entry, 139, { belowHeld: false, heading: true })).toBe(false);
	});
});
