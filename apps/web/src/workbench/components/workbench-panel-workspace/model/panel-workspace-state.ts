import type {
	TerminalPresentationCloseActivity,
	TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import type {
	PanelLayoutNode,
	PanelLeaf,
	PanelSplit,
	PanelSplitDirection,
	PanelSplitOrientation,
	TerminalPresentation,
} from './panel-layout-types';

export type {
	PanelLayoutNode,
	PanelLeaf,
	PanelSplit,
	PanelSplitDirection,
	TerminalPresentation,
} from './panel-layout-types';

export interface TabPanelWorkspace {
	readonly focusedPanelId: string;
	readonly maximizedPanelId: string | null;
	readonly panelFocusHistory: readonly string[];
	readonly root: PanelLayoutNode;
	readonly tabId: string;
}

export interface PanelWorkspaceState {
	readonly nextPanelSequence: number;
	readonly nextPanelSlotSequence: number;
	readonly nextSplitSequence: number;
	readonly nextTerminalSequence: number;
	readonly tabs: readonly TabPanelWorkspace[];
}

interface PanelMoveTarget {
	readonly direction: PanelSplitDirection;
	readonly targetPanelId: string;
}

export type PanelWorkspaceEvent =
	| {
			readonly childPresentationId: TerminalPresentationId;
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'child-terminal-activated';
	  }
	| {
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'panel-focused';
	  }
	/** Brings a Panel this Client just asked for into view, even over a maximized one. */
	| {
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'panel-revealed';
	  }
	| {
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'panel-maximization-toggled';
	  }
	| {
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'primary-terminal-shown';
	  }
	| {
			readonly closeActivity: TerminalPresentationCloseActivity;
			readonly currentCwd?: string | null | undefined;
			readonly naturalTitle: string | null;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-metadata-changed';
	  }
	/** Follows the Tab strip; a Tab arrives only with the Panels a publication gave it. */
	| {
			readonly tabIds: readonly string[];
			readonly type: 'tabs-reconciled';
	  };

type PanelWorkspaceRejection =
	| 'child-terminal-not-found'
	| 'panel-not-found'
	| 'tab-not-found'
	| 'terminal-presentation-required'
	| 'terminal-not-found';

export interface PanelWorkspaceTransition {
	readonly outcome: 'applied' | 'rejected' | 'unchanged';
	readonly rejection: PanelWorkspaceRejection | null;
	readonly state: PanelWorkspaceState;
}

interface TreeUpdate {
	readonly changed: boolean;
	readonly node: PanelLayoutNode;
}

interface TreeRemoval {
	readonly node: PanelLayoutNode | null;
	readonly removed: boolean;
}

interface PanelInsertionSlotIds {
	readonly insertedPanelSlotId: string;
	readonly targetPanelSlotId: string;
}

const WEIGHT_TOTAL = 100;
const WEIGHT_TOLERANCE = 0.000_001;

function findPanelInNode(node: PanelLayoutNode, panelId: string): PanelLeaf | null {
	if (node.kind === 'panel') {
		return node.id === panelId ? node : null;
	}
	for (const child of node.children) {
		const panel = findPanelInNode(child.node, panelId);
		if (panel !== null) {
			return panel;
		}
	}
	return null;
}

function collectPanelsFromNode(node: PanelLayoutNode, panels: PanelLeaf[]): void {
	if (node.kind === 'panel') {
		panels.push(node);
		return;
	}
	for (const child of node.children) {
		collectPanelsFromNode(child.node, panels);
	}
}

function mapPanel(
	node: PanelLayoutNode,
	panelId: string,
	update: (panel: PanelLeaf) => PanelLeaf | null,
): TreeUpdate {
	if (node.kind === 'panel') {
		if (node.id !== panelId) {
			return { changed: false, node };
		}
		const nextPanel = update(node);
		return nextPanel === null || nextPanel === node
			? { changed: false, node }
			: { changed: true, node: nextPanel };
	}
	let changed = false;
	const children = node.children.map((child) => {
		const result = mapPanel(child.node, panelId, update);
		changed ||= result.changed;
		return result.changed ? { ...child, node: result.node } : child;
	});
	return changed ? { changed: true, node: { ...node, children } } : { changed: false, node };
}

function mapTerminal(
	node: PanelLayoutNode,
	presentationId: TerminalPresentationId,
	update: (terminal: TerminalPresentation) => TerminalPresentation,
): TreeUpdate {
	if (node.kind === 'panel') {
		if (node.primaryPresentation.presentationId === presentationId) {
			const terminal = update(node.primaryPresentation);
			return {
				changed: terminal !== node.primaryPresentation,
				node:
					terminal === node.primaryPresentation ? node : { ...node, primaryPresentation: terminal },
			};
		}
		const terminalIndex = node.childPresentations.findIndex(
			(terminal) => terminal.presentationId === presentationId,
		);
		const terminal = node.childPresentations[terminalIndex];
		if (terminal === undefined) {
			return { changed: false, node };
		}
		const nextTerminal = update(terminal);
		if (nextTerminal === terminal) {
			return { changed: false, node };
		}
		const childPresentations = [...node.childPresentations];
		childPresentations[terminalIndex] = nextTerminal;
		return { changed: true, node: { ...node, childPresentations } };
	}
	for (const [index, child] of node.children.entries()) {
		const result = mapTerminal(child.node, presentationId, update);
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}
	return { changed: false, node };
}

function mapTerminalMetadata(
	node: PanelLayoutNode,
	presentationId: TerminalPresentationId,
	naturalTitle: string | null,
	closeActivity: TerminalPresentationCloseActivity,
	currentCwd?: string | null,
): TreeUpdate {
	return mapTerminal(node, presentationId, (terminal) => {
		const nextCurrentCwd = currentCwd === undefined ? terminal.currentCwd : currentCwd;
		return terminal.closeActivity === closeActivity &&
			terminal.currentCwd === nextCurrentCwd &&
			terminal.naturalTitle === naturalTitle
			? terminal
			: { ...terminal, closeActivity, currentCwd: nextCurrentCwd, naturalTitle };
	});
}

function findTerminalInNode(
	node: PanelLayoutNode,
	presentationId: TerminalPresentationId,
): TerminalPresentation | null {
	if (node.kind === 'panel') {
		if (node.primaryPresentation.presentationId === presentationId) {
			return node.primaryPresentation;
		}
		return (
			node.childPresentations.find((terminal) => terminal.presentationId === presentationId) ?? null
		);
	}
	for (const child of node.children) {
		const terminal = findTerminalInNode(child.node, presentationId);
		if (terminal !== null) return terminal;
	}
	return null;
}

function orientationForDirection(direction: PanelSplitDirection): PanelSplitOrientation {
	return direction === 'left' || direction === 'right' ? 'horizontal' : 'vertical';
}

function directionInsertsBefore(direction: PanelSplitDirection): boolean {
	return direction === 'above' || direction === 'left';
}

function isPanelAtMoveTarget(
	node: PanelLayoutNode,
	panelId: string,
	targetPanelId: string,
	direction: PanelSplitDirection,
): boolean {
	if (node.kind === 'panel') {
		return false;
	}
	if (node.orientation === orientationForDirection(direction)) {
		const panelIndex = node.children.findIndex(
			(child) => child.node.kind === 'panel' && child.node.id === panelId,
		);
		const targetPanelIndex = node.children.findIndex(
			(child) => child.node.kind === 'panel' && child.node.id === targetPanelId,
		);
		const expectedPanelIndex = directionInsertsBefore(direction)
			? targetPanelIndex - 1
			: targetPanelIndex + 1;
		if (panelIndex !== -1 && panelIndex === expectedPanelIndex) {
			return true;
		}
	}
	return node.children.some((child) =>
		isPanelAtMoveTarget(child.node, panelId, targetPanelId, direction),
	);
}

function insertPanelNode(
	node: PanelLayoutNode,
	panelId: string,
	direction: PanelSplitDirection,
	newPanel: PanelLeaf,
	splitId: string,
	slotIds: PanelInsertionSlotIds,
): TreeUpdate {
	const orientation = orientationForDirection(direction);
	if (node.kind === 'panel') {
		if (node.id !== panelId) {
			return { changed: false, node };
		}
		const sourceChild = {
			id: slotIds.targetPanelSlotId,
			node,
			weight: WEIGHT_TOTAL / 2,
		};
		const newChild = {
			id: slotIds.insertedPanelSlotId,
			node: newPanel,
			weight: WEIGHT_TOTAL / 2,
		};
		return {
			changed: true,
			node: {
				children: directionInsertsBefore(direction)
					? [newChild, sourceChild]
					: [sourceChild, newChild],
				id: splitId,
				kind: 'split',
				orientation,
			},
		};
	}

	for (const [index, child] of node.children.entries()) {
		const result = insertPanelNode(child.node, panelId, direction, newPanel, splitId, slotIds);
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}

	return { changed: false, node };
}

function collapseSplit(split: PanelSplit): PanelLayoutNode {
	const onlyChild = split.children.length === 1 ? split.children[0] : undefined;
	return onlyChild === undefined ? split : onlyChild.node;
}

function removePanelFromNode(node: PanelLayoutNode, panelId: string): TreeRemoval {
	if (node.kind === 'panel') {
		return node.id === panelId ? { node: null, removed: true } : { node, removed: false };
	}
	for (const [index, child] of node.children.entries()) {
		const result = removePanelFromNode(child.node, panelId);
		if (!result.removed) {
			continue;
		}
		const children = [...node.children];
		if (result.node === null) {
			children.splice(index, 1);
		} else {
			children[index] = {
				...child,
				node: result.node,
			};
		}
		return {
			node: collapseSplit({ ...node, children }),
			removed: true,
		};
	}
	return { node, removed: false };
}

function applied(state: PanelWorkspaceState): PanelWorkspaceTransition {
	const invariantErrors = validatePanelWorkspaceState(state);
	if (invariantErrors.length > 0) {
		throw new Error(`Panel workspace invariant violation: ${invariantErrors.join('; ')}`);
	}
	return { outcome: 'applied', rejection: null, state };
}

function unchanged(state: PanelWorkspaceState): PanelWorkspaceTransition {
	return { outcome: 'unchanged', rejection: null, state };
}

function rejected(
	state: PanelWorkspaceState,
	rejection: PanelWorkspaceRejection,
): PanelWorkspaceTransition {
	return { outcome: 'rejected', rejection, state };
}

function replaceTab(
	state: PanelWorkspaceState,
	tabIndex: number,
	tab: TabPanelWorkspace,
): PanelWorkspaceState {
	const tabs = [...state.tabs];
	tabs[tabIndex] = tab;
	return { ...state, tabs };
}

function focusPanelInTab(tab: TabPanelWorkspace, panelId: string): TabPanelWorkspace {
	if (tab.focusedPanelId === panelId && tab.panelFocusHistory.at(-1) === panelId) {
		return tab;
	}
	return {
		...tab,
		focusedPanelId: panelId,
		panelFocusHistory: [
			...tab.panelFocusHistory.filter((candidatePanelId) => candidatePanelId !== panelId),
			panelId,
		],
	};
}

export function collectPanelLeaves(root: PanelLayoutNode): readonly PanelLeaf[] {
	const panels: PanelLeaf[] = [];
	collectPanelsFromNode(root, panels);
	return panels;
}

/**
 * Asks as the TUI does: before ending a terminal that may still run a program, and before closing a
 * Presentation with no terminal to read. A terminal that ended, or whose launch failed, closes
 * unasked.
 */
export function terminalRequiresCloseConfirmation(terminal: TerminalPresentation): boolean {
	switch (terminal.lifecycle.kind) {
		case 'running':
			return terminal.closeActivity !== 'idle';
		case 'unbound':
			return true;
		case 'exited':
		case 'failed':
		case 'launch-failed':
			return false;
	}
}

export function countTerminalsRequiringCloseConfirmation(root: PanelLayoutNode): number {
	return collectPanelLeaves(root).reduce(
		(count, panel) =>
			count +
			[panel.primaryPresentation, ...panel.childPresentations].filter(
				terminalRequiresCloseConfirmation,
			).length,
		0,
	);
}

export function findPanel(root: PanelLayoutNode, panelId: string): PanelLeaf | null {
	return findPanelInNode(root, panelId);
}

export function getDisplayedTerminal(panel: PanelLeaf): TerminalPresentation {
	return (
		panel.childPresentations.find(
			(terminal) => terminal.presentationId === panel.activeChildPresentationId,
		) ?? panel.primaryPresentation
	);
}

export function getTerminalDisplayTitle(terminal: TerminalPresentation): string {
	return (
		terminal.customTitle ??
		terminal.naturalTitle ??
		deriveTerminalDirectoryTitle(terminal.currentCwd) ??
		terminal.title
	);
}

function deriveTerminalDirectoryTitle(cwd: string | null): string | null {
	if (cwd === null) return null;
	const trimmedCwd = cwd.trim();
	if (trimmedCwd.length === 0) return null;
	const cwdWithoutTrailingSeparators = trimmedCwd.replace(/[\\/]+$/u, '');
	if (cwdWithoutTrailingSeparators.length === 0) return trimmedCwd;
	return cwdWithoutTrailingSeparators.split(/[\\/]/u).at(-1) ?? null;
}

export function findTerminal(
	root: PanelLayoutNode,
	presentationId: TerminalPresentationId,
): TerminalPresentation | null {
	return findTerminalInNode(root, presentationId);
}

export function projectPanelDrag(
	root: PanelLayoutNode,
	panelId: string,
	target: PanelMoveTarget | null,
): PanelLayoutNode | null {
	const panel = findPanelInNode(root, panelId);
	if (
		panel === null ||
		(target !== null &&
			(target.targetPanelId === panelId || findPanelInNode(root, target.targetPanelId) === null))
	) {
		return null;
	}
	if (
		target !== null &&
		isPanelAtMoveTarget(root, panelId, target.targetPanelId, target.direction)
	) {
		return root;
	}
	const removal = removePanelFromNode(root, panelId);
	if (!removal.removed || removal.node === null || target === null) {
		return removal.node;
	}
	const result = insertPanelNode(
		removal.node,
		target.targetPanelId,
		target.direction,
		panel,
		'panel-drag-preview-split',
		{
			insertedPanelSlotId: 'panel-drag-preview-slot-inserted',
			targetPanelSlotId: 'panel-drag-preview-slot-target',
		},
	);
	return result.changed ? result.node : removal.node;
}

export function transitionPanelWorkspace(
	state: PanelWorkspaceState,
	event: PanelWorkspaceEvent,
): PanelWorkspaceTransition {
	if (event.type === 'tabs-reconciled') {
		const existingByTabId = new Map(state.tabs.map((tab) => [tab.tabId, tab]));
		const tabs: TabPanelWorkspace[] = [];
		for (const tabId of event.tabIds) {
			const existing = existingByTabId.get(tabId);
			// The Server publishes every Tab with its Panels, so a Tab arrives only with its own state.
			if (existing === undefined) return rejected(state, 'terminal-presentation-required');
			tabs.push(existing);
		}
		const isUnchanged =
			tabs.length === state.tabs.length && tabs.every((tab, index) => tab === state.tabs[index]);
		return isUnchanged ? unchanged(state) : applied({ ...state, tabs });
	}

	const tabIndex = state.tabs.findIndex((tab) => tab.tabId === event.tabId);
	const tab = state.tabs[tabIndex];
	if (tab === undefined) {
		return rejected(state, 'tab-not-found');
	}

	if (event.type === 'terminal-metadata-changed') {
		const result = mapTerminalMetadata(
			tab.root,
			event.presentationId,
			event.naturalTitle,
			event.closeActivity,
			event.currentCwd,
		);
		if (!result.changed) {
			return findTerminalInNode(tab.root, event.presentationId) === null
				? rejected(state, 'terminal-not-found')
				: unchanged(state);
		}
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
	}

	const panel = findPanelInNode(tab.root, event.panelId);
	if (panel === null) {
		return rejected(state, 'panel-not-found');
	}

	if (event.type === 'child-terminal-activated') {
		if (
			!panel.childPresentations.some(
				(terminal) => terminal.presentationId === event.childPresentationId,
			)
		) {
			return rejected(state, 'child-terminal-not-found');
		}
		if (panel.activeChildPresentationId === event.childPresentationId) {
			return unchanged(state);
		}
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildPresentationId: event.childPresentationId,
		}));
		return applied(
			replaceTab(state, tabIndex, {
				...focusPanelInTab(tab, event.panelId),
				root: result.node,
			}),
		);
	}

	if (event.type === 'primary-terminal-shown') {
		if (panel.activeChildPresentationId === null) {
			return unchanged(state);
		}
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildPresentationId: null,
		}));
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
	}

	if (event.type === 'panel-focused') {
		return tab.focusedPanelId === event.panelId
			? unchanged(state)
			: applied(replaceTab(state, tabIndex, focusPanelInTab(tab, event.panelId)));
	}

	if (event.type === 'panel-revealed') {
		return tab.focusedPanelId === event.panelId && tab.maximizedPanelId === null
			? unchanged(state)
			: applied(
					replaceTab(state, tabIndex, {
						...focusPanelInTab(tab, event.panelId),
						maximizedPanelId: null,
					}),
				);
	}

	const maximizedPanelId = tab.maximizedPanelId === event.panelId ? null : event.panelId;
	return applied(
		replaceTab(state, tabIndex, {
			...focusPanelInTab(tab, event.panelId),
			maximizedPanelId,
		}),
	);
}

export function validatePanelWorkspaceState(state: PanelWorkspaceState): readonly string[] {
	const errors: string[] = [];
	const tabIds = new Set<string>();
	const panelIds = new Set<string>();
	const panelSlotIds = new Set<string>();
	const splitIds = new Set<string>();
	const presentationIds = new Set<string>();
	for (const tab of state.tabs) {
		if (tabIds.has(tab.tabId)) {
			errors.push(`Duplicate tab id: ${tab.tabId}`);
		}
		tabIds.add(tab.tabId);
		const panels = collectPanelLeaves(tab.root);
		const localPanelIds = new Set(panels.map((panel) => panel.id));
		if (!localPanelIds.has(tab.focusedPanelId)) {
			errors.push(`Missing focused panel: ${tab.focusedPanelId}`);
		}
		if (tab.panelFocusHistory.length === 0) {
			errors.push(`Empty panel focus history: ${tab.tabId}`);
		}
		const historyPanelIds = new Set<string>();
		for (const historyPanelId of tab.panelFocusHistory) {
			if (historyPanelIds.has(historyPanelId)) {
				errors.push(`Duplicate panel focus history id: ${historyPanelId}`);
			}
			historyPanelIds.add(historyPanelId);
			if (!localPanelIds.has(historyPanelId)) {
				errors.push(`Missing panel in focus history: ${historyPanelId}`);
			}
		}
		if (tab.panelFocusHistory.at(-1) !== tab.focusedPanelId) {
			errors.push(`Panel focus history does not end with focused panel: ${tab.tabId}`);
		}
		if (tab.maximizedPanelId !== null && !localPanelIds.has(tab.maximizedPanelId)) {
			errors.push(`Missing maximized panel: ${tab.maximizedPanelId}`);
		}
		const visit = (node: PanelLayoutNode): void => {
			if (node.kind === 'panel') {
				if (panelIds.has(node.id)) {
					errors.push(`Duplicate panel id: ${node.id}`);
				}
				panelIds.add(node.id);
				const childIds = new Set(
					node.childPresentations.map((terminal) => terminal.presentationId),
				);
				if (
					node.activeChildPresentationId !== null &&
					!childIds.has(node.activeChildPresentationId)
				) {
					errors.push(`Missing active child terminal: ${node.activeChildPresentationId}`);
				}
				for (const terminal of [node.primaryPresentation, ...node.childPresentations]) {
					if (presentationIds.has(terminal.presentationId)) {
						errors.push(`Duplicate terminal id: ${terminal.presentationId}`);
					}
					presentationIds.add(terminal.presentationId);
				}
				return;
			}
			if (splitIds.has(node.id)) {
				errors.push(`Duplicate split id: ${node.id}`);
			}
			splitIds.add(node.id);
			if (node.children.length !== 2) {
				errors.push(`Split does not have exactly two children: ${node.id}`);
			}
			const childIds = new Set(node.children.map((child) => child.id));
			if (childIds.size !== node.children.length) {
				errors.push(`Split has duplicate child ids: ${node.id}`);
			}
			for (const child of node.children) {
				if (panelSlotIds.has(child.id)) {
					errors.push(`Duplicate panel slot id: ${child.id}`);
				}
				panelSlotIds.add(child.id);
			}
			const total = node.children.reduce((sum, child) => sum + child.weight, 0);
			if (
				node.children.some((child) => !Number.isFinite(child.weight) || child.weight < 0) ||
				Math.abs(total - WEIGHT_TOTAL) > WEIGHT_TOLERANCE
			) {
				errors.push(`Invalid split weights: ${node.id}`);
			}
			for (const child of node.children) visit(child.node);
		};
		visit(tab.root);
	}
	return errors;
}
