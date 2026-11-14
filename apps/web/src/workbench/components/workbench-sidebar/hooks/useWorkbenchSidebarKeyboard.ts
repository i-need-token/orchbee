import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../../project-workspace';
import {
	resolveSiblingReorder,
	resolveSidebarNavigationKey,
	type SidebarNavigationKey,
	type SidebarNavigationRow,
	type SidebarNavigationRowKind,
} from '../model/sidebar-navigation-keys';
import { revealSidebarRow } from '../utils/revealSidebarRow';

const TREE_KEYS: ReadonlySet<string> = new Set<SidebarNavigationKey>([
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'ArrowUp',
	'End',
	'Enter',
	'Home',
]);

function isTreeKey(key: string): key is SidebarNavigationKey {
	return TREE_KEYS.has(key);
}

interface UseWorkbenchSidebarKeyboardOptions {
	readonly activateWorkspace: (workspaceId: string) => void;
	readonly createWorkspace: () => void;
	readonly onSearch: ((returnFocus: HTMLElement) => void) | undefined;
	readonly onShortcuts: (() => void) | undefined;
	readonly reorderModifier: 'ctrl' | 'meta';
	readonly reorderProject: (projectId: string, beforeProjectId: string | null) => Promise<unknown>;
	readonly moveWorkspace: (
		workspaceId: string,
		projectId: string | null,
		beforeWorkspaceId: string | null,
	) => Promise<unknown>;
	readonly returnToTerminal: (() => void) | undefined;
	readonly state: ProjectWorkspaceState;
}

interface WorkbenchSidebarKeyboard {
	readonly listReference: React.RefObject<HTMLDivElement | null>;
	readonly onBlur: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly onFocus: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
}

function navigationTargets(root: HTMLElement): HTMLElement[] {
	return [...root.querySelectorAll<HTMLElement>('[data-sidebar-nav]')];
}

function rowKind(target: HTMLElement): SidebarNavigationRowKind | null {
	const kind = target.dataset['sidebarNav'];
	return kind === 'agent' || kind === 'heading' || kind === 'workspace' ? kind : null;
}

/** The control that folds a row: a heading's chevron is the row itself; a Workspace has its own. */
function foldControl(target: HTMLElement): HTMLElement | null {
	if (target.hasAttribute('aria-expanded')) return target;
	if (rowKind(target) !== 'workspace') return null;
	return (
		target
			.closest('[data-navigation-reorder-row]')
			?.querySelector<HTMLElement>('button[aria-expanded]') ?? null
	);
}

function navigationRow(target: HTMLElement): SidebarNavigationRow | null {
	const kind = rowKind(target);
	if (kind === null) return null;
	const control = foldControl(target);
	return {
		expanded: control === null ? null : control.getAttribute('aria-expanded') === 'true',
		kind,
	};
}

/**
 * The row Tab enters the list at: the current Workspace, else the heading folded around it, else
 * the first row. While focus is inside the list, the focused row holds the list's one tab stop.
 */
function entryTarget(root: HTMLElement): HTMLElement | null {
	return (
		root.querySelector<HTMLElement>('[data-sidebar-nav="workspace"][aria-current="page"]') ??
		root.querySelector<HTMLElement>('[data-sidebar-nav][data-holds-active-workspace]') ??
		root.querySelector<HTMLElement>('[data-sidebar-nav]')
	);
}

function holdTabStop(root: HTMLElement, holder: HTMLElement | null): void {
	for (const target of navigationTargets(root)) {
		target.tabIndex = target === holder ? 0 : -1;
	}
}

/**
 * The sidebar list's keys, as in the TUI. The list is one tab stop; the arrows, Home and End move
 * through its rows and fold them; Enter opens a row and returns to the terminal; Shift+N, / and F1
 * start a Workspace, Search and Shortcuts; Ctrl+↑↓ (⌘ on macOS) reorder; Esc returns to the
 * terminal. A row renames with F2 and opens its menu with Shift+F10 or Menu.
 */
export function useWorkbenchSidebarKeyboard({
	activateWorkspace,
	createWorkspace,
	onSearch,
	onShortcuts,
	reorderModifier,
	reorderProject,
	moveWorkspace,
	returnToTerminal,
	state,
}: UseWorkbenchSidebarKeyboardOptions): WorkbenchSidebarKeyboard {
	const listReference = React.useRef<HTMLDivElement>(null);

	// Rows come and go with folding and data; outside the list, the tab stop follows the current row.
	React.useLayoutEffect(() => {
		const root = listReference.current;
		if (root === null || root.contains(root.ownerDocument.activeElement)) return;
		holdTabStop(root, entryTarget(root));
	});

	const focusRow = (root: HTMLElement, target: HTMLElement) => {
		holdTabStop(root, target);
		target.focus({ preventScroll: true });
		revealSidebarRow(root, target);
	};

	const reorder = (target: HTMLElement, direction: 'down' | 'up'): boolean => {
		const kind = rowKind(target);
		if (kind === 'heading') {
			const projectId = target.closest<HTMLElement>('[data-project-id]')?.dataset['projectId'];
			if (projectId === undefined) return false;
			const move = resolveSiblingReorder(
				state.projects.map((project) => project.id),
				projectId,
				direction,
			);
			if (move !== undefined) void reorderProject(projectId, move.beforeId);
			return true;
		}
		if (kind !== 'workspace') return false;
		const workspaceId = target.closest<HTMLElement>('[data-workspace-id]')?.dataset['workspaceId'];
		const workspace = state.workspaces.find((candidate) => candidate.id === workspaceId);
		if (workspace === undefined) return false;
		const move = resolveSiblingReorder(
			state.workspaces
				.filter((candidate) => candidate.projectId === workspace.projectId)
				.map((candidate) => candidate.id),
			workspace.id,
			direction,
		);
		if (move !== undefined) void moveWorkspace(workspace.id, workspace.projectId, move.beforeId);
		return true;
	};

	const runTreeKey = (root: HTMLElement, target: HTMLElement, key: SidebarNavigationKey) => {
		const targets = navigationTargets(root);
		const rows: SidebarNavigationRow[] = [];
		for (const candidate of targets) {
			const row = navigationRow(candidate);
			if (row !== null) rows.push(row);
		}
		if (rows.length !== targets.length) return false;
		const intent = resolveSidebarNavigationKey(rows, targets.indexOf(target), key);
		if (intent === null) return key !== 'Enter';
		const subject = targets[intent.index];
		if (subject === undefined) return false;
		switch (intent.kind) {
			case 'focus':
				focusRow(root, subject);
				return true;
			case 'fold':
			case 'unfold':
				foldControl(subject)?.click();
				return true;
			case 'open': {
				if (rowKind(subject) === 'agent') {
					// Agent navigation hands focus to the Agent's terminal itself.
					subject.click();
					return true;
				}
				const workspaceId =
					subject.closest<HTMLElement>('[data-workspace-id]')?.dataset['workspaceId'];
				if (workspaceId !== undefined) activateWorkspace(workspaceId);
				return true;
			}
		}
	};

	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLDivElement>) => {
		const root = listReference.current;
		const target = event.target;
		if (
			root === null ||
			event.defaultPrevented ||
			event.nativeEvent.isComposing ||
			!(target instanceof HTMLElement) ||
			!target.hasAttribute('data-sidebar-nav') ||
			!root.contains(target) ||
			// A drag owns the pointer and Esc until it ends.
			root.querySelector('[data-dragging]') !== null
		) {
			return;
		}
		const { altKey, ctrlKey, key, metaKey, shiftKey } = event;
		const reorderHeld = reorderModifier === 'meta' ? metaKey && !ctrlKey : ctrlKey && !metaKey;
		let handled = false;
		if (reorderHeld && !altKey && !shiftKey && (key === 'ArrowUp' || key === 'ArrowDown')) {
			handled = reorder(target, key === 'ArrowUp' ? 'up' : 'down');
		} else if (altKey || ctrlKey || metaKey) {
			return;
		} else if (key === '/') {
			// A window that withholds Search leaves the key unbound.
			if (onSearch !== undefined) {
				onSearch(target);
				handled = true;
			}
		} else if (shiftKey) {
			if (key === 'N') {
				createWorkspace();
				handled = true;
			}
		} else if (key === 'F1') {
			onShortcuts?.();
			handled = true;
		} else if (key === 'Escape') {
			returnToTerminal?.();
			handled = true;
		} else if (key === ' ') {
			handled = runTreeKey(root, target, 'Enter');
		} else if (isTreeKey(key)) {
			handled = runTreeKey(root, target, key);
		}
		if (handled) event.preventDefault();
	});

	const onFocus = useMemoizedFn((event: React.FocusEvent<HTMLDivElement>) => {
		const root = listReference.current;
		const target = event.target;
		if (root === null || !(target instanceof HTMLElement)) return;
		if (target.hasAttribute('data-sidebar-nav')) holdTabStop(root, target);
	});

	const onBlur = useMemoizedFn((event: React.FocusEvent<HTMLDivElement>) => {
		const root = listReference.current;
		if (root === null) return;
		const next = event.relatedTarget;
		if (next instanceof Node && root.contains(next)) return;
		holdTabStop(root, entryTarget(root));
	});

	return { listReference, onBlur, onFocus, onKeyDown };
}
