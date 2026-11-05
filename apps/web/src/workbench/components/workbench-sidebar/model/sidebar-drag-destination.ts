/**
 * Where a sidebar drag lands, as the TUI decides it (`tui_pointer/sidebar_reorder`): the same
 * entries, the same placement and the same Server operation. A drop is one `navigation.configure`
 * change, so the shown order is the canonical order with that change applied.
 */

/** An entry a drag can hold or land on. Recents and Agent rows are neither. */
export type SidebarDragItem =
	| { readonly kind: 'project'; readonly id: string }
	| { readonly kind: 'workspace'; readonly id: string };

/** One `navigation.configure` operation, as the Server takes it. */
export type SidebarNavigationMove =
	| {
			readonly kind: 'move-project';
			readonly projectId: string;
			readonly beforeProjectId: string | null;
	  }
	| {
			readonly kind: 'move-workspace';
			readonly workspaceId: string;
			readonly projectId: string | null;
			readonly beforeWorkspaceId: string | null;
	  };

/** The canonical navigation order: Projects in order, and Workspaces in one global order. */
export interface SidebarNavigationOrder<
	TProject extends { readonly id: string },
	TWorkspace extends { readonly id: string; readonly projectId: string | null },
> {
	readonly projects: readonly TProject[];
	readonly workspaces: readonly TWorkspace[];
}

type NavigationOrder = SidebarNavigationOrder<
	{ readonly id: string },
	{ readonly id: string; readonly projectId: string | null }
>;

/** Whether a move is refused, changes nothing, or gives a new order. */
export type SidebarNavigationMoveResult<TOrder> =
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'origin' }
	| { readonly kind: 'moved'; readonly order: TOrder };

/** What the pointer is over, read from the shown layout. */
export type SidebarDragHit =
	/** Beyond the list: the canonical order shows, and a release there keeps it. */
	| { readonly kind: 'outside' }
	/** The drop area after the list and the free space below it. */
	| { readonly kind: 'drop-area' }
	/** A gap, or a row that is no entry for this drag. */
	| { readonly kind: 'gap' }
	/** `entries[index]`; `after` places the held entry after it rather than before. */
	| { readonly kind: 'entry'; readonly index: number; readonly after: boolean };

/** What to show next: keep the shown move, show the canonical order, or show a new move. */
export type SidebarDropResolution =
	| { readonly kind: 'keep' }
	| { readonly kind: 'origin' }
	| { readonly kind: 'moved'; readonly move: SidebarNavigationMove };

/** Mirrors the Server's `moved_before`: the source taken out and put back before the anchor. */
function movedBefore(
	original: readonly string[],
	source: string,
	before: string | null,
): readonly string[] | null {
	if (before === source) return original;
	const ordered = original.filter((id) => id !== source);
	if (ordered.length + 1 !== original.length) return null;
	const index = before === null ? ordered.length : ordered.indexOf(before);
	if (index < 0) return null;
	return [...ordered.slice(0, index), source, ...ordered.slice(index)];
}

function sameOrder(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && left.every((id, index) => id === right[index]);
}

/**
 * Applies a move as the Server's `configure_navigation` does. A Workspace move refills only its
 * destination group's places in the global order, so every other group keeps its exact positions.
 * The Server also refuses a Project already at its Workspace limit; the shown move leaves that to
 * the Server's answer.
 */
export function applySidebarNavigationMove<
	TProject extends { readonly id: string },
	TWorkspace extends { readonly id: string; readonly projectId: string | null },
>(
	order: SidebarNavigationOrder<TProject, TWorkspace>,
	move: SidebarNavigationMove,
): SidebarNavigationMoveResult<SidebarNavigationOrder<TProject, TWorkspace>> {
	if (move.kind === 'move-project') {
		const original = order.projects.map((project) => project.id);
		if (
			!original.includes(move.projectId) ||
			(move.beforeProjectId !== null && !original.includes(move.beforeProjectId))
		) {
			return { kind: 'unavailable' };
		}
		const ordered = movedBefore(original, move.projectId, move.beforeProjectId);
		if (ordered === null) return { kind: 'unavailable' };
		if (sameOrder(ordered, original)) return { kind: 'origin' };
		const projects = new Map(order.projects.map((project) => [project.id, project]));
		return {
			kind: 'moved',
			order: {
				projects: ordered.flatMap((id) => projects.get(id) ?? []),
				workspaces: order.workspaces,
			},
		};
	}
	const source = order.workspaces.find((workspace) => workspace.id === move.workspaceId);
	if (source === undefined) return { kind: 'unavailable' };
	if (move.projectId !== null && !order.projects.some((project) => project.id === move.projectId)) {
		return { kind: 'unavailable' };
	}
	if (move.beforeWorkspaceId !== null) {
		const anchor = order.workspaces.find((workspace) => workspace.id === move.beforeWorkspaceId);
		if (anchor === undefined || anchor.projectId !== move.projectId) return { kind: 'unavailable' };
	}
	const original = order.workspaces.map((workspace) => workspace.id);
	const destinationIds = new Set(
		order.workspaces
			.filter(
				(workspace) => workspace.id === move.workspaceId || workspace.projectId === move.projectId,
			)
			.map((workspace) => workspace.id),
	);
	const destinationOrder = original.filter((id) => destinationIds.has(id));
	const ordered = movedBefore(destinationOrder, move.workspaceId, move.beforeWorkspaceId);
	if (ordered === null) return { kind: 'unavailable' };
	let member = 0;
	const next = original.map((id) => (destinationIds.has(id) ? (ordered[member++] ?? id) : id));
	if (source.projectId === move.projectId && sameOrder(next, original)) return { kind: 'origin' };
	const workspaces = new Map(order.workspaces.map((workspace) => [workspace.id, workspace]));
	return {
		kind: 'moved',
		order: {
			projects: order.projects,
			workspaces: next.flatMap((id) => {
				const workspace = workspaces.get(id);
				if (workspace === undefined) return [];
				return id === move.workspaceId && workspace.projectId !== move.projectId
					? [{ ...workspace, projectId: move.projectId }]
					: [workspace];
			}),
		},
	};
}

function sameItem(left: SidebarDragItem, right: SidebarDragItem): boolean {
	return left.kind === right.kind && left.id === right.id;
}

type Landing =
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'origin' }
	| { readonly kind: 'moved'; readonly move: SidebarNavigationMove };

function validate(order: NavigationOrder, move: SidebarNavigationMove): Landing {
	const result = applySidebarNavigationMove(order, move);
	return result.kind === 'moved' ? { kind: 'moved', move } : result;
}

/** The TUI's `candidate`: the operation that puts `held` before or after `target`. */
function candidate(
	order: NavigationOrder,
	held: SidebarDragItem,
	target: SidebarDragItem | null,
	after: boolean,
): SidebarNavigationMove | null {
	if (held.kind === 'project') {
		if (target === null) return { beforeProjectId: null, kind: 'move-project', projectId: held.id };
		if (target.kind === 'workspace') return null;
		const others = order.projects.filter((project) => project.id !== held.id);
		const index = others.findIndex((project) => project.id === target.id);
		if (index < 0) return null;
		return {
			beforeProjectId: others[index + (after ? 1 : 0)]?.id ?? null,
			kind: 'move-project',
			projectId: held.id,
		};
	}
	if (target === null) {
		return {
			beforeWorkspaceId: null,
			kind: 'move-workspace',
			projectId: null,
			workspaceId: held.id,
		};
	}
	if (target.kind === 'project') {
		return {
			beforeWorkspaceId: null,
			kind: 'move-workspace',
			projectId: target.id,
			workspaceId: held.id,
		};
	}
	const anchor = order.workspaces.find((workspace) => workspace.id === target.id);
	if (anchor === undefined) return null;
	const others = order.workspaces.filter(
		(workspace) => workspace.projectId === anchor.projectId && workspace.id !== held.id,
	);
	const index = others.findIndex((workspace) => workspace.id === target.id);
	if (index < 0) return null;
	return {
		beforeWorkspaceId: others[index + (after ? 1 : 0)]?.id ?? null,
		kind: 'move-workspace',
		projectId: anchor.projectId,
		workspaceId: held.id,
	};
}

function destination(
	order: NavigationOrder,
	held: SidebarDragItem,
	target: SidebarDragItem | null,
	after: boolean,
): Landing {
	const move = candidate(order, held, target, after);
	return move === null ? { kind: 'unavailable' } : validate(order, move);
}

/** The Workspace becomes the Project's first, or its only one. */
function firstIn(order: NavigationOrder, workspaceId: string, projectId: string): Landing {
	const first = order.workspaces.find(
		(workspace) => workspace.projectId === projectId && workspace.id !== workspaceId,
	);
	if (first === undefined) {
		return destination(
			order,
			{ id: workspaceId, kind: 'workspace' },
			{ id: projectId, kind: 'project' },
			false,
		);
	}
	return validate(order, {
		beforeWorkspaceId: first.id,
		kind: 'move-workspace',
		projectId,
		workspaceId,
	});
}

/**
 * A Workspace held over the Project heading at `entries[index]`. A heading with no other Workspace
 * shown under it takes the Workspace in, at the end of its list. Otherwise the heading is the
 * Project's top edge: from above, the Workspace becomes its first; from below, it follows the
 * Workspace shown just above the heading. Null keeps the Workspace where it is shown, just above.
 */
function ontoHeading(
	order: NavigationOrder,
	workspaceId: string,
	projectId: string,
	entries: readonly SidebarDragItem[],
	index: number,
	after: boolean,
): Landing | null {
	const held: SidebarDragItem = { id: workspaceId, kind: 'workspace' };
	let others = false;
	for (const entry of entries.slice(index + 1)) {
		if (entry.kind !== 'workspace') break;
		if (entry.id === workspaceId) continue;
		const workspace = order.workspaces.find((member) => member.id === entry.id);
		if (workspace?.projectId !== projectId) break;
		others = true;
		break;
	}
	if (!others) return destination(order, held, { id: projectId, kind: 'project' }, false);
	const above = entries[index - 1];
	if (!after && above?.kind === 'workspace') {
		if (above.id === workspaceId) return null;
		return destination(order, held, above, true);
	}
	return firstIn(order, workspaceId, projectId);
}

/**
 * The destination under the pointer. `entries` are the entries shown in display order: Project
 * headings and Workspace rows for a Workspace drag, Project headings alone for a Project drag. Over
 * the held entry, gaps, rows that are no entry, and destinations the order refuses, the shown move
 * stays, so a pointer at rest never makes the list oscillate.
 */
export function resolveSidebarDrop(
	order: NavigationOrder,
	held: SidebarDragItem,
	entries: readonly SidebarDragItem[],
	hit: SidebarDragHit,
): SidebarDropResolution {
	let landing: Landing | null;
	if (hit.kind === 'outside') return { kind: 'origin' };
	if (hit.kind === 'gap') return { kind: 'keep' };
	if (hit.kind === 'drop-area') {
		landing = destination(order, held, null, false);
	} else {
		const entry = entries[hit.index];
		if (entry === undefined || sameItem(entry, held)) return { kind: 'keep' };
		landing =
			held.kind === 'workspace' && entry.kind === 'project'
				? ontoHeading(order, held.id, entry.id, entries, hit.index, hit.after)
				: destination(order, held, entry, hit.after);
	}
	if (landing === null || landing.kind === 'unavailable') return { kind: 'keep' };
	return landing.kind === 'origin' ? { kind: 'origin' } : { kind: 'moved', move: landing.move };
}

/**
 * Whether a pointer at `clientY` over an entry places the held entry after it, as the TUI's rule:
 * the lower half of a Workspace entry, and the far side of a one-line Project heading, the side away
 * from the held entry, so touching a heading is enough.
 */
export function placesAfter(
	entry: { readonly height: number; readonly top: number },
	clientY: number,
	{ belowHeld, heading }: { readonly belowHeld: boolean; readonly heading: boolean },
): boolean {
	return heading ? belowHeld : clientY >= entry.top + entry.height / 2;
}
