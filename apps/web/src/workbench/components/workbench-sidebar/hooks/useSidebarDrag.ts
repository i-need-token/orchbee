import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';
import { flushSync } from 'react-dom';

import type { ProjectWorkspaceState } from '../../../../project-workspace';
import {
	applySidebarNavigationMove,
	resolveSidebarDrop,
	type SidebarDragItem,
	type SidebarNavigationMove,
	type SidebarNavigationOrder,
} from '../model/sidebar-drag-destination';
import {
	createSidebarDragState,
	transitionSidebarDrag,
	type SidebarDragState,
} from '../model/sidebar-drag-state';
import { readSidebarDragHit } from '../utils/readSidebarDragHit';

type SidebarShownOrder = SidebarNavigationOrder<
	ProjectWorkspaceState['projects'][number],
	ProjectWorkspaceState['workspaces'][number]
>;

interface SidebarDragOptions {
	/** Makes a held Workspace current as its drag latches; it stays current however the drag ends. */
	readonly holdWorkspace: (workspaceId: string) => void;
	readonly moveWorkspace: (
		workspaceId: string,
		projectId: string | null,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	/** The canonical navigation the drag starts from. */
	readonly state: ProjectWorkspaceState;
}

interface SidebarDragView {
	/** The End of Projects drop area to show after the list, while a Project drag is latched. */
	readonly dropArea: 'project' | null;
	readonly dropAreaHovered: boolean;
	/** The entry a latched or settling drag holds. */
	readonly held: SidebarDragItem | null;
	readonly onEntryPointerDown: (
		item: SidebarDragItem,
		event: React.PointerEvent<HTMLElement>,
	) => void;
	/** The order to show: the canonical one with the shown move applied. */
	readonly order: SidebarShownOrder;
}

interface SidebarDragSession {
	readonly clientX: number;
	readonly clientY: number;
	readonly item: SidebarDragItem;
	readonly pointerId: number;
	/** The list's scroll region: it takes pointer capture, scrolls, and bounds the destinations. */
	readonly region: HTMLElement;
	/** The pressed row, copied as the raised copy that follows the pointer. */
	readonly source: HTMLElement;
}

/** Holding the list's top or bottom edge scrolls it by about one row this often. */
const EDGE_SCROLL_INTERVAL_MS = 120;

/** Only these change where a drop lands; a drag cancels when any of them changes under it. */
function navigationKey(state: ProjectWorkspaceState): string {
	return JSON.stringify([
		state.projects.map((project) => project.id),
		state.workspaces.map((workspace) => [workspace.id, workspace.projectId]),
	]);
}

/**
 * A press on a row leaves keyboard focus where it is, since a drag never moves it. The row is no
 * focus target, so the browser would drop focus to the page, and the terminal would claim it.
 */
function keepFocusOnPress(event: MouseEvent): void {
	if (event.button === 0) event.preventDefault();
}

function shownOrder(
	state: ProjectWorkspaceState,
	shown: SidebarNavigationMove | null,
): SidebarShownOrder {
	if (shown === null) return state;
	const result = applySidebarNavigationMove(state, shown);
	return result.kind === 'moved' ? result.order : state;
}

/** Raises a copy of the pressed row that follows the pointer 8 px to its right. */
function raiseCopy(session: SidebarDragSession): HTMLElement | null {
	const layer = session.region
		.closest('[data-slot="workbench-sidebar"]')
		?.querySelector('[data-sidebar-drag-layer]');
	const copy = session.source.cloneNode(true);
	if (!(layer instanceof HTMLElement) || !(copy instanceof HTMLElement)) return null;
	const bounds = session.source.getBoundingClientRect();
	copy.removeAttribute('data-current');
	for (const element of [copy, ...copy.querySelectorAll('[id]')]) element.removeAttribute('id');
	copy.inert = true;
	copy.setAttribute('data-drag-ghost', 'true');
	copy.style.setProperty('--ob-comp-workbench-navigation-reorder-block-start', `${bounds.top}px`);
	copy.style.setProperty('--ob-comp-workbench-navigation-reorder-inline-start', `${bounds.left}px`);
	copy.style.setProperty('--ob-comp-workbench-navigation-reorder-inline-size', `${bounds.width}px`);
	layer.append(copy);
	return copy;
}

/**
 * One drag across the whole sidebar, as the TUI's: Project headings and Workspace rows are held,
 * the held entry stands at its destination in the shown list, and a release asks the Server for
 * that one move. The shown order holds until the move settles, so the old order never flashes.
 * Every key belongs to the drag while it holds the pointer: Esc cancels, and nothing reaches the
 * terminal or the list.
 */
export function useSidebarDrag({
	holdWorkspace,
	moveWorkspace,
	reorderProject,
	state,
}: SidebarDragOptions): SidebarDragView {
	const [gesture, setGesture] = React.useState<SidebarDragState>(createSidebarDragState);
	const [session, setSession] = React.useState<SidebarDragSession | null>(null);
	const stateRef = React.useRef(state);
	const commandsRef = React.useRef({ holdWorkspace, moveWorkspace, reorderProject });
	const cancelRef = React.useRef<(() => void) | null>(null);
	const baselineKeyRef = React.useRef<string | null>(null);
	stateRef.current = state;
	commandsRef.current = { holdWorkspace, moveWorkspace, reorderProject };
	const key = navigationKey(state);

	const onEntryPointerDown = useMemoizedFn(
		(item: SidebarDragItem, event: React.PointerEvent<HTMLElement>) => {
			const target = event.target;
			const region = event.currentTarget.closest<HTMLElement>('[data-navigation-reorder-scroll]');
			if (
				session !== null ||
				gesture.kind !== 'idle' ||
				region === null ||
				event.button !== 0 ||
				(event.pointerType !== 'mouse' && event.pointerType !== 'pen') ||
				(target instanceof Element && target.closest('[data-navigation-reorder-excluded]') !== null)
			) {
				return;
			}
			event.currentTarget.addEventListener('mousedown', keepFocusOnPress, { once: true });
			baselineKeyRef.current = key;
			setGesture(
				transitionSidebarDrag(gesture, {
					clientX: event.clientX,
					clientY: event.clientY,
					item,
					pointerId: event.pointerId,
					type: 'pointer-armed',
				}).state,
			);
			setSession({
				clientX: event.clientX,
				clientY: event.clientY,
				item,
				pointerId: event.pointerId,
				region,
				source: event.currentTarget,
			});
		},
	);

	React.useEffect(() => {
		if (session === null) return;
		const { region } = session;
		const ownerDocument = region.ownerDocument;
		const ownerWindow = ownerDocument.defaultView;
		let semantic: SidebarDragState = {
			clientX: session.clientX,
			clientY: session.clientY,
			item: session.item,
			kind: 'armed',
			pointerId: session.pointerId,
		};
		let finished = false;
		let copy: HTMLElement | null = null;
		let rowSize = 0;
		let animationFrame: number | null = null;
		let lastEdgeScroll = Number.NEGATIVE_INFINITY;
		let latestClientX = session.clientX;
		let latestClientY = session.clientY;
		let refreshAfterActivation = false;
		const initialCursor = ownerDocument.documentElement.style.cursor;

		const releaseCapture = () => {
			if (
				typeof region.hasPointerCapture !== 'function' ||
				typeof region.releasePointerCapture !== 'function' ||
				!region.hasPointerCapture(session.pointerId)
			) {
				return;
			}
			try {
				region.releasePointerCapture(session.pointerId);
			} catch {
				// The browser may already have released it.
			}
		};
		const clearVisual = () => {
			copy?.remove();
			copy = null;
			ownerDocument.documentElement.style.cursor = initialCursor;
			if (animationFrame !== null) ownerWindow?.cancelAnimationFrame(animationFrame);
			animationFrame = null;
		};
		const suppressTrailingClick = () => {
			let consumed = false;
			const suppress = (event: MouseEvent) => {
				consumed = true;
				event.preventDefault();
				event.stopPropagation();
			};
			ownerDocument.addEventListener('click', suppress, { capture: true, once: true });
			ownerWindow?.setTimeout(() => {
				if (!consumed) ownerDocument.removeEventListener('click', suppress, true);
			}, 0);
		};
		const settle = () => {
			semantic = transitionSidebarDrag(semantic, { type: 'move-settled' }).state;
			flushSync(() => {
				setGesture(semantic);
				setSession(null);
			});
		};
		const commit = (move: SidebarNavigationMove) => {
			const commands = commandsRef.current;
			const request =
				move.kind === 'move-project'
					? commands.reorderProject(move.projectId, move.beforeProjectId)
					: commands.moveWorkspace(move.workspaceId, move.projectId, move.beforeWorkspaceId);
			void request.then(settle, settle);
		};
		const finish = (next: SidebarDragState, move: SidebarNavigationMove | null) => {
			const wasDragging = semantic.kind === 'dragging';
			semantic = next;
			finished = true;
			baselineKeyRef.current = null;
			clearVisual();
			releaseCapture();
			if (wasDragging) suppressTrailingClick();
			if (move !== null) {
				setGesture(semantic);
				commit(move);
				return;
			}
			flushSync(() => {
				setGesture(semantic);
				setSession(null);
			});
		};
		const cancel = () => {
			if (finished) return;
			finish(transitionSidebarDrag(semantic, { type: 'cancelled' }).state, null);
		};
		cancelRef.current = cancel;

		const latch = () => {
			if (typeof region.setPointerCapture === 'function') {
				try {
					region.setPointerCapture(session.pointerId);
				} catch {
					// Document listeners still own completion when capture is unavailable.
				}
			}
			rowSize = session.source.getBoundingClientRect().height;
			copy = raiseCopy(session);
			ownerDocument.documentElement.style.cursor = 'grabbing';
			refreshAfterActivation = true;
			if (session.item.kind === 'workspace') commandsRef.current.holdWorkspace(session.item.id);
		};
		const follow = (clientX: number, clientY: number) => {
			if (semantic.kind !== 'dragging') return;
			copy?.style.setProperty(
				'--ob-comp-workbench-navigation-reorder-translate-y',
				`${clientY - session.clientY}px`,
			);
			const { entries, hit } = readSidebarDragHit(
				region,
				session.item,
				clientX,
				clientY,
				session.clientY,
			);
			const transition = transitionSidebarDrag(semantic, {
				dropAreaHovered: hit.kind === 'drop-area',
				resolution: resolveSidebarDrop(stateRef.current, session.item, entries, hit),
				type: 'destination-resolved',
			});
			if (transition.state !== semantic) {
				semantic = transition.state;
				setGesture(semantic);
			}
		};
		const runEdgeScroll = () => {
			animationFrame = null;
			if (finished || semantic.kind !== 'dragging') return;
			const bounds = region.getBoundingClientRect();
			const edgeSize = Math.min(rowSize, bounds.height / 4);
			const now = ownerWindow?.performance.now() ?? 0;
			let step = 0;
			if (edgeSize > 0 && latestClientY < bounds.top + edgeSize) step = -rowSize;
			else if (edgeSize > 0 && latestClientY > bounds.bottom - edgeSize) step = rowSize;
			if (step === 0) lastEdgeScroll = Number.NEGATIVE_INFINITY;
			else if (now - lastEdgeScroll < EDGE_SCROLL_INTERVAL_MS) step = 0;
			else lastEdgeScroll = now;
			let scrolled = false;
			if (step !== 0) {
				const previous = region.scrollTop;
				const maximum = Math.max(0, region.scrollHeight - region.clientHeight);
				region.scrollTop = Math.max(0, Math.min(maximum, previous + step));
				scrolled = region.scrollTop !== previous;
			}
			if (scrolled || refreshAfterActivation) {
				refreshAfterActivation = false;
				follow(latestClientX, latestClientY);
			}
			animationFrame = ownerWindow?.requestAnimationFrame(runEdgeScroll) ?? null;
		};

		const handlePointerMove = (event: PointerEvent) => {
			if (finished || event.pointerId !== session.pointerId) return;
			latestClientX = event.clientX;
			latestClientY = event.clientY;
			const transition = transitionSidebarDrag(semantic, {
				clientX: event.clientX,
				clientY: event.clientY,
				pointerId: event.pointerId,
				type: 'pointer-moved',
			});
			if (transition.state !== semantic) {
				semantic = transition.state;
				setGesture(semantic);
			}
			if (transition.command?.type === 'drag-started') latch();
			if (semantic.kind !== 'dragging') return;
			event.preventDefault();
			follow(event.clientX, event.clientY);
			animationFrame ??= ownerWindow?.requestAnimationFrame(runEdgeScroll) ?? null;
		};
		const handlePointerUp = (event: PointerEvent) => {
			if (finished || event.pointerId !== session.pointerId) return;
			const bounds = region.getBoundingClientRect();
			const outside =
				event.clientX < bounds.left ||
				event.clientX >= bounds.right ||
				event.clientY < bounds.top ||
				event.clientY >= bounds.bottom;
			const transition = transitionSidebarDrag(semantic, {
				outside,
				pointerId: event.pointerId,
				type: 'pointer-released',
			});
			finish(
				transition.state,
				transition.command?.type === 'commit-move' ? transition.command.move : null,
			);
		};
		const handlePointerCancel = (event: PointerEvent) => {
			if (event.pointerId === session.pointerId) cancel();
		};
		// While the drag holds the pointer every key is its own, from wherever focus is, as in the
		// TUI: the window's capture phase takes each key before a terminal or the list sees it. A
		// plain Esc cancels; a press it cancels still ends without a click.
		const handleKey = (event: KeyboardEvent) => {
			if (finished) return;
			event.preventDefault();
			event.stopPropagation();
			if (
				event.type !== 'keydown' ||
				event.key !== 'Escape' ||
				event.altKey ||
				event.ctrlKey ||
				event.metaKey ||
				event.shiftKey
			) {
				return;
			}
			const pressed = semantic.kind === 'armed';
			cancel();
			if (pressed) {
				ownerDocument.addEventListener(
					'pointerup',
					(release) => {
						if (release.pointerId === session.pointerId) suppressTrailingClick();
					},
					{ capture: true, once: true },
				);
			}
		};

		ownerDocument.addEventListener('pointermove', handlePointerMove, { passive: false });
		ownerDocument.addEventListener('pointerup', handlePointerUp);
		ownerDocument.addEventListener('pointercancel', handlePointerCancel);
		ownerWindow?.addEventListener('keydown', handleKey, true);
		ownerWindow?.addEventListener('keypress', handleKey, true);
		ownerWindow?.addEventListener('keyup', handleKey, true);
		ownerWindow?.addEventListener('blur', cancel);
		ownerWindow?.addEventListener('resize', cancel);
		region.addEventListener('lostpointercapture', cancel);
		return () => {
			ownerDocument.removeEventListener('pointermove', handlePointerMove);
			ownerDocument.removeEventListener('pointerup', handlePointerUp);
			ownerDocument.removeEventListener('pointercancel', handlePointerCancel);
			ownerWindow?.removeEventListener('keydown', handleKey, true);
			ownerWindow?.removeEventListener('keypress', handleKey, true);
			ownerWindow?.removeEventListener('keyup', handleKey, true);
			ownerWindow?.removeEventListener('blur', cancel);
			ownerWindow?.removeEventListener('resize', cancel);
			region.removeEventListener('lostpointercapture', cancel);
			if (cancelRef.current === cancel) cancelRef.current = null;
			if (!finished && semantic.kind === 'dragging') suppressTrailingClick();
			finished = true;
			clearVisual();
			releaseCapture();
		};
	}, [session]);

	// A canonical change under a held press or drag invalidates where it would land.
	React.useEffect(() => {
		if (baselineKeyRef.current !== null && baselineKeyRef.current !== key) cancelRef.current?.();
	}, [key]);

	const shown = gesture.kind === 'dragging' || gesture.kind === 'settling' ? gesture.shown : null;
	const order = React.useMemo(() => shownOrder(state, shown), [state, shown]);
	return {
		dropArea: gesture.kind === 'dragging' && gesture.item.kind === 'project' ? 'project' : null,
		dropAreaHovered: gesture.kind === 'dragging' && gesture.dropAreaHovered,
		held: gesture.kind === 'dragging' || gesture.kind === 'settling' ? gesture.item : null,
		onEntryPointerDown,
		order,
	};
}
