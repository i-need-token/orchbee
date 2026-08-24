import type {
	TerminalPresentationCloseActivity,
	TerminalPresentationCloseOperation,
	TerminalPresentationCloseRequest,
	TerminalPresentationCommand,
	TerminalPresentationCreation,
	TerminalPresentationFailure,
	TerminalPresentationId,
	TerminalPresentationLifecycle,
	TerminalPresentationOperationId,
	TerminalPresentationRemovalId,
} from '../../../../terminal-presentation';
import type { WorkbenchTerminalPresentationCommand } from '../../../model/createWorkbenchPreviewTerminalPresentationComposition';

export type PanelSplitDirection = 'above' | 'below' | 'left' | 'right';
type PanelSplitOrientation = 'horizontal' | 'vertical';

export interface TerminalPresentation {
	readonly closeActivity: TerminalPresentationCloseActivity;
	readonly currentCwd: string | null;
	readonly launchCwd: string | null;
	readonly lifecycle: TerminalPresentationLifecycle;
	readonly naturalTitle: string | null;
	readonly presentationId: TerminalPresentationId;
	readonly title: string;
}

export interface PanelLeaf {
	readonly activeChildPresentationId: TerminalPresentationId | null;
	readonly childPresentations: readonly TerminalPresentation[];
	readonly id: string;
	readonly kind: 'panel';
	readonly primaryPresentation: TerminalPresentation;
}

interface PanelSplitChild {
	readonly id: string;
	readonly node: PanelLayoutNode;
	readonly weight: number;
}

export interface PanelSplit {
	readonly children: readonly PanelSplitChild[];
	readonly id: string;
	readonly kind: 'split';
	readonly orientation: PanelSplitOrientation;
}

export type PanelLayoutNode = PanelLeaf | PanelSplit;

export interface TabPanelWorkspace {
	readonly focusedPanelId: string;
	readonly maximizedPanelId: string | null;
	readonly root: PanelLayoutNode;
	readonly tabId: string;
}

export interface PanelWorkspaceState {
	readonly nextPanelSequence: number;
	readonly nextPanelSlotSequence: number;
	readonly nextSplitSequence: number;
	readonly nextTerminalSequence: number;
	readonly pendingRemovals: readonly TerminalPresentationRemoval[];
	readonly tabs: readonly TabPanelWorkspace[];
}

export interface PanelWorkspaceTabInput {
	readonly cwd?: string | null | undefined;
	readonly id: string;
	readonly presentation?: TerminalPresentationCreation | undefined;
	readonly title: string;
}

type TerminalPresentationRemoval =
	| {
			readonly childPresentationId: TerminalPresentationId;
			readonly kind: 'child';
			readonly panelId: string;
			readonly presentationIds: readonly TerminalPresentationId[];
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
	  }
	| {
			readonly kind: 'panel';
			readonly panelId: string;
			readonly presentationIds: readonly TerminalPresentationId[];
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
	  }
	| {
			readonly kind: 'primary';
			readonly panelId: string;
			readonly presentationIds: readonly TerminalPresentationId[];
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
	  }
	| {
			readonly kind: 'tab';
			readonly presentationIds: readonly TerminalPresentationId[];
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
	  };

type PanelWorkspaceCommand = WorkbenchTerminalPresentationCommand;

export interface PanelWorkspaceInitialization {
	readonly commands: readonly PanelWorkspaceCommand[];
	readonly state: PanelWorkspaceState;
}

export interface PanelMoveTarget {
	readonly direction: PanelSplitDirection;
	readonly targetPanelId: string;
}

export type PanelWorkspaceEvent =
	| {
			readonly direction: PanelSplitDirection;
			readonly newTerminalTitle: string;
			readonly panelId: string;
			readonly presentation: TerminalPresentationCreation;
			readonly tabId: string;
			readonly type: 'panel-split';
	  }
	| {
			readonly direction: PanelSplitDirection;
			readonly panelId: string;
			readonly tabId: string;
			readonly targetPanelId: string;
			readonly type: 'panel-moved';
	  }
	| {
			readonly leadingWeight: number;
			readonly separatorIndex: number;
			readonly splitId: string;
			readonly tabId: string;
			readonly type: 'adjacent-panels-resized';
	  }
	| {
			readonly newTerminalTitle: string;
			readonly panelId: string;
			readonly presentation: TerminalPresentationCreation;
			readonly tabId: string;
			readonly type: 'child-terminal-created';
	  }
	| {
			readonly childPresentationId: TerminalPresentationId;
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'child-terminal-activated';
	  }
	| {
			readonly childPresentationId: TerminalPresentationId;
			readonly close: TerminalPresentationCloseRequest;
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'child-terminal-closed';
	  }
	| {
			readonly close: TerminalPresentationCloseRequest;
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'panel-closed';
	  }
	| {
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'panel-focused';
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
			readonly currentCwd: string | null;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-current-cwd-changed';
	  }
	| {
			readonly closeActivity: TerminalPresentationCloseActivity;
			readonly currentCwd?: string | null | undefined;
			readonly naturalTitle: string | null;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-metadata-changed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-provision-completed';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-provision-failed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-provision-retried';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-restore-requested';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-restore-started';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-restore-completed';
	  }
	| {
			readonly operations: readonly TerminalPresentationCloseOperation[];
			readonly type: 'terminal-sessions-stop-requested';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-stop-completed';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-stop-failed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-stop-retried';
	  }
	| {
			readonly presentation: Extract<TerminalPresentationCreation, { readonly kind: 'managed' }>;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-stopped-session-started';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-restore-failed';
	  }
	| {
			readonly close: Extract<TerminalPresentationCloseRequest, { readonly kind: 'managed' }>;
			readonly exitCode: number | null;
			readonly presentationId: TerminalPresentationId;
			readonly signal: string | null;
			readonly tabId: string;
			readonly type: 'terminal-exited';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly presentationId: TerminalPresentationId;
			readonly tabId: string;
			readonly type: 'terminal-failed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
			readonly type: 'terminal-close-completed';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
			readonly type: 'terminal-close-failed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
			readonly type: 'terminal-close-retried';
	  }
	| {
			readonly close: Extract<TerminalPresentationCloseRequest, { readonly kind: 'managed' }>;
			readonly tabId: string;
			readonly type: 'tab-close-requested';
	  }
	| {
			readonly tabs: readonly PanelWorkspaceTabInput[];
			readonly type: 'tabs-reconciled';
	  };

type PanelWorkspaceRejection =
	| 'child-terminal-not-found'
	| 'invalid-close-operations'
	| 'invalid-resize'
	| 'last-panel'
	| 'panel-not-found'
	| 'stale-operation'
	| 'split-not-found'
	| 'tab-not-found'
	| 'terminal-removal-conflict'
	| 'terminal-removal-required'
	| 'terminal-presentation-required'
	| 'terminal-session-not-stopped'
	| 'terminal-not-found';

export interface PanelWorkspaceTransition {
	readonly commands: readonly PanelWorkspaceCommand[];
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

interface PanelWorkspaceCloneIdentities {
	readonly createPresentationId: () => TerminalPresentationId;
	readonly panelIds: Map<string, string>;
	readonly panelSlotIds: Map<string, string>;
	readonly prefix: string;
	readonly splitIds: Map<string, string>;
	readonly presentationIds: Map<TerminalPresentationId, TerminalPresentationId>;
}

const WEIGHT_TOTAL = 100;
const WEIGHT_TOLERANCE = 0.000_001;

function createTerminal(
	presentation: TerminalPresentationCreation,
	title: string,
	cwd: string | null,
): {
	readonly command: TerminalPresentationCommand | null;
	readonly terminal: TerminalPresentation;
} {
	return {
		command:
			presentation.kind === 'managed'
				? {
						launchCwd: cwd,
						operationId: presentation.operationId,
						presentationId: presentation.presentationId,
						type: 'terminal-provision-requested',
					}
				: null,
		terminal: {
			closeActivity: 'unknown',
			currentCwd: cwd,
			launchCwd: cwd,
			lifecycle:
				presentation.kind === 'managed'
					? { kind: 'provisioning', operationId: presentation.operationId }
					: { kind: 'preview' },
			naturalTitle: null,
			presentationId: presentation.presentationId,
			title,
		},
	};
}

function createPanel(
	panelSequence: number,
	presentation: TerminalPresentationCreation,
	title: string,
	cwd: string | null,
): { readonly command: TerminalPresentationCommand | null; readonly panel: PanelLeaf } {
	const creation = createTerminal(presentation, title, cwd);
	return {
		command: creation.command,
		panel: {
			activeChildPresentationId: null,
			childPresentations: [],
			id: `panel-${panelSequence}`,
			kind: 'panel',
			primaryPresentation: creation.terminal,
		},
	};
}

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

function replaceStoppedTerminal(
	node: PanelLayoutNode,
	presentationId: TerminalPresentationId,
	presentation: Extract<TerminalPresentationCreation, { readonly kind: 'managed' }>,
): TreeUpdate {
	if (node.kind === 'panel') {
		if (node.primaryPresentation.presentationId === presentationId) {
			if (node.primaryPresentation.lifecycle.kind !== 'stopped') {
				return { changed: false, node };
			}
			return {
				changed: true,
				node: {
					...node,
					primaryPresentation: restartedTerminal(node.primaryPresentation, presentation),
				},
			};
		}
		const terminalIndex = node.childPresentations.findIndex(
			(terminal) => terminal.presentationId === presentationId,
		);
		const terminal = node.childPresentations[terminalIndex];
		if (terminal === undefined || terminal.lifecycle.kind !== 'stopped') {
			return { changed: false, node };
		}
		const childPresentations = [...node.childPresentations];
		childPresentations[terminalIndex] = restartedTerminal(terminal, presentation);
		return {
			changed: true,
			node: {
				...node,
				activeChildPresentationId:
					node.activeChildPresentationId === presentationId
						? presentation.presentationId
						: node.activeChildPresentationId,
				childPresentations,
			},
		};
	}
	for (const [index, child] of node.children.entries()) {
		const result = replaceStoppedTerminal(child.node, presentationId, presentation);
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}
	return { changed: false, node };
}

function restartedTerminal(
	terminal: TerminalPresentation,
	presentation: Extract<TerminalPresentationCreation, { readonly kind: 'managed' }>,
): TerminalPresentation {
	return {
		...terminal,
		closeActivity: 'unknown',
		currentCwd: terminal.launchCwd,
		lifecycle: { kind: 'provisioning', operationId: presentation.operationId },
		naturalTitle: null,
		presentationId: presentation.presentationId,
	};
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

function collectPresentationIds(node: PanelLayoutNode): readonly TerminalPresentationId[] {
	return collectPanelLeaves(node).flatMap((panel) => [
		panel.primaryPresentation.presentationId,
		...panel.childPresentations.map((terminal) => terminal.presentationId),
	]);
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

function resizeSplitNode(
	node: PanelLayoutNode,
	splitId: string,
	separatorIndex: number,
	leadingWeight: number,
): TreeUpdate | PanelWorkspaceRejection {
	if (node.kind === 'panel') {
		return { changed: false, node };
	}
	if (node.id === splitId) {
		const leadingChild = node.children[separatorIndex];
		const trailingChild = node.children[separatorIndex + 1];
		if (
			leadingChild === undefined ||
			trailingChild === undefined ||
			!Number.isFinite(leadingWeight)
		) {
			return 'invalid-resize';
		}
		const pairWeight = leadingChild.weight + trailingChild.weight;
		const clampedLeadingWeight = Math.min(Math.max(leadingWeight, 0), pairWeight);
		const trailingWeight = pairWeight - clampedLeadingWeight;
		if (
			Math.abs(leadingChild.weight - clampedLeadingWeight) <= WEIGHT_TOLERANCE &&
			Math.abs(trailingChild.weight - trailingWeight) <= WEIGHT_TOLERANCE
		) {
			return { changed: false, node };
		}
		const children = [...node.children];
		children[separatorIndex] = { ...leadingChild, weight: clampedLeadingWeight };
		children[separatorIndex + 1] = { ...trailingChild, weight: trailingWeight };
		return { changed: true, node: { ...node, children } };
	}
	for (const [index, child] of node.children.entries()) {
		const result = resizeSplitNode(child.node, splitId, separatorIndex, leadingWeight);
		if (typeof result === 'string') {
			return result;
		}
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}
	return { changed: false, node };
}

function applied(
	state: PanelWorkspaceState,
	commands: readonly PanelWorkspaceCommand[] = [],
): PanelWorkspaceTransition {
	const invariantErrors = validatePanelWorkspaceState(state);
	if (invariantErrors.length > 0) {
		throw new Error(`Panel workspace invariant violation: ${invariantErrors.join('; ')}`);
	}
	return { commands, outcome: 'applied', rejection: null, state };
}

function unchanged(state: PanelWorkspaceState): PanelWorkspaceTransition {
	return { commands: [], outcome: 'unchanged', rejection: null, state };
}

function rejected(
	state: PanelWorkspaceState,
	rejection: PanelWorkspaceRejection,
): PanelWorkspaceTransition {
	return { commands: [], outcome: 'rejected', rejection, state };
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

function replaceTerminalLifecycle(
	state: PanelWorkspaceState,
	tabIndex: number,
	tab: TabPanelWorkspace,
	presentationId: TerminalPresentationId,
	lifecycle: TerminalPresentationLifecycle,
	commands: readonly PanelWorkspaceCommand[] = [],
): PanelWorkspaceTransition {
	const result = mapTerminal(tab.root, presentationId, (terminal) =>
		terminal.lifecycle === lifecycle ? terminal : { ...terminal, lifecycle },
	);
	if (!result.changed) {
		return findTerminalInNode(tab.root, presentationId) === null
			? rejected(state, 'terminal-not-found')
			: unchanged(state);
	}
	return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }), commands);
}

function beginManagedRemoval(
	state: PanelWorkspaceState,
	tabIndex: number,
	tab: TabPanelWorkspace,
	removal: TerminalPresentationRemoval,
	close: Extract<TerminalPresentationCloseRequest, { readonly kind: 'managed' }>,
): PanelWorkspaceTransition {
	const targetIds = new Set(removal.presentationIds);
	const operationIds = new Set(close.operations.map((operation) => operation.operationId));
	const operationPresentationIds = new Set(
		close.operations.map((operation) => operation.presentationId),
	);
	if (
		removal.presentationIds.length === 0 ||
		close.removalId !== removal.removalId ||
		targetIds.size !== removal.presentationIds.length ||
		close.operations.length !== removal.presentationIds.length ||
		operationIds.size !== close.operations.length ||
		operationPresentationIds.size !== close.operations.length ||
		close.operations.some((operation) => !targetIds.has(operation.presentationId))
	) {
		return rejected(state, 'invalid-close-operations');
	}
	if (
		state.pendingRemovals.some((pending) =>
			pending.presentationIds.some((presentationId) => targetIds.has(presentationId)),
		)
	) {
		return rejected(state, 'terminal-removal-conflict');
	}
	let root = tab.root;
	const commands: TerminalPresentationCommand[] = [];
	for (const operation of close.operations) {
		const terminal = findTerminalInNode(root, operation.presentationId);
		if (
			terminal === null ||
			terminal.lifecycle.kind === 'preview' ||
			terminal.lifecycle.kind === 'closing' ||
			terminal.lifecycle.kind === 'closed' ||
			terminal.lifecycle.kind === 'close-failed'
		) {
			return rejected(
				state,
				terminal === null ? 'terminal-not-found' : 'terminal-removal-conflict',
			);
		}
		root = mapTerminal(root, operation.presentationId, (candidate) => ({
			...candidate,
			lifecycle: {
				kind: 'closing',
				operationId: operation.operationId,
				removalId: close.removalId,
			},
		})).node;
		commands.push({
			disposition: 'remove',
			operationId: operation.operationId,
			presentationId: operation.presentationId,
			type: 'terminal-close-requested',
		});
	}
	return applied(
		{
			...replaceTab(state, tabIndex, { ...tab, root }),
			pendingRemovals: [...state.pendingRemovals, removal],
		},
		commands,
	);
}

function beginManagedStop(
	state: PanelWorkspaceState,
	operations: readonly TerminalPresentationCloseOperation[],
): PanelWorkspaceTransition {
	if (operations.length === 0) return unchanged(state);
	const operationIds = new Set(operations.map((operation) => operation.operationId));
	const presentationIds = new Set(operations.map((operation) => operation.presentationId));
	if (operationIds.size !== operations.length || presentationIds.size !== operations.length) {
		return rejected(state, 'invalid-close-operations');
	}
	let tabs = state.tabs;
	const commands: TerminalPresentationCommand[] = [];
	for (const operation of operations) {
		const tabIndex = tabs.findIndex(
			(tab) => findTerminalInNode(tab.root, operation.presentationId) !== null,
		);
		const tab = tabs[tabIndex];
		const terminal =
			tab === undefined ? null : findTerminalInNode(tab.root, operation.presentationId);
		if (
			tab === undefined ||
			terminal === null ||
			terminal.lifecycle.kind === 'preview' ||
			terminal.lifecycle.kind === 'closing' ||
			terminal.lifecycle.kind === 'close-failed' ||
			terminal.lifecycle.kind === 'closed' ||
			terminal.lifecycle.kind === 'stopping' ||
			terminal.lifecycle.kind === 'stopped'
		) {
			return rejected(
				state,
				terminal === null ? 'terminal-not-found' : 'terminal-removal-conflict',
			);
		}
		const result = mapTerminal(tab.root, operation.presentationId, (candidate) => ({
			...candidate,
			lifecycle: { kind: 'stopping', operationId: operation.operationId },
		}));
		const nextTabs = [...tabs];
		nextTabs[tabIndex] = { ...tab, root: result.node };
		tabs = nextTabs;
		commands.push({
			disposition: 'stop',
			operationId: operation.operationId,
			presentationId: operation.presentationId,
			type: 'terminal-close-requested',
		});
	}
	return applied({ ...state, tabs }, commands);
}

function removePanelFromTab(
	state: PanelWorkspaceState,
	tabIndex: number,
	tab: TabPanelWorkspace,
	panelId: string,
): PanelWorkspaceState | null {
	const panels = collectPanelLeaves(tab.root);
	const removedIndex = panels.findIndex((candidate) => candidate.id === panelId);
	const nextFocusedPanel = panels[removedIndex - 1] ?? panels[removedIndex + 1];
	const result = removePanelFromNode(tab.root, panelId);
	if (!result.removed || result.node === null || nextFocusedPanel === undefined) return null;
	return replaceTab(state, tabIndex, {
		...tab,
		focusedPanelId: tab.focusedPanelId === panelId ? nextFocusedPanel.id : tab.focusedPanelId,
		maximizedPanelId: tab.maximizedPanelId === panelId ? null : tab.maximizedPanelId,
		root: result.node,
	});
}

function withoutRemoval(
	state: PanelWorkspaceState,
	removalId: TerminalPresentationRemovalId,
): PanelWorkspaceState {
	return {
		...state,
		pendingRemovals: state.pendingRemovals.filter((removal) => removal.removalId !== removalId),
	};
}

function removalTargetsAreClosed(
	tab: TabPanelWorkspace,
	removal: TerminalPresentationRemoval,
): boolean {
	return removal.presentationIds.every(
		(presentationId) => findTerminalInNode(tab.root, presentationId)?.lifecycle.kind === 'closed',
	);
}

function pendingTargetsExcept(
	state: PanelWorkspaceState,
	removalId: TerminalPresentationRemovalId,
): ReadonlySet<TerminalPresentationId> {
	return new Set(
		state.pendingRemovals.flatMap((removal) =>
			removal.removalId === removalId ? [] : removal.presentationIds,
		),
	);
}

function convertRemovalToTab(
	state: PanelWorkspaceState,
	removal: TerminalPresentationRemoval,
): PanelWorkspaceState {
	return {
		...state,
		pendingRemovals: state.pendingRemovals.map((candidate) =>
			candidate.removalId === removal.removalId
				? {
						kind: 'tab' as const,
						presentationIds: removal.presentationIds,
						removalId: removal.removalId,
						tabId: removal.tabId,
					}
				: candidate,
		),
	};
}

function settleCompletedRemovals(
	initialState: PanelWorkspaceState,
	tabId: string,
): { readonly commands: readonly PanelWorkspaceCommand[]; readonly state: PanelWorkspaceState } {
	let state = initialState;
	while (true) {
		const tabIndex = state.tabs.findIndex((candidate) => candidate.tabId === tabId);
		const tab = state.tabs[tabIndex];
		if (tab === undefined) return { commands: [], state };
		const ready = state.pendingRemovals.filter(
			(removal) => removal.tabId === tabId && removalTargetsAreClosed(tab, removal),
		);
		if (ready.length === 0) return { commands: [], state };

		const childRemoval = ready.find((removal) => removal.kind === 'child');
		if (childRemoval?.kind === 'child') {
			const panel = findPanelInNode(tab.root, childRemoval.panelId);
			if (panel === null) throw new Error('A completed child removal lost its owning Panel.');
			const result = mapPanel(tab.root, childRemoval.panelId, (candidate) => ({
				...candidate,
				activeChildPresentationId:
					candidate.activeChildPresentationId === childRemoval.childPresentationId
						? null
						: candidate.activeChildPresentationId,
				childPresentations: candidate.childPresentations.filter(
					(terminal) => terminal.presentationId !== childRemoval.childPresentationId,
				),
			}));
			state = withoutRemoval(
				replaceTab(state, tabIndex, { ...tab, root: result.node }),
				childRemoval.removalId,
			);
			continue;
		}

		let primarySettled = false;
		for (const primaryRemoval of ready) {
			if (primaryRemoval.kind !== 'primary') continue;
			const panel = findPanelInNode(tab.root, primaryRemoval.panelId);
			if (panel === null) throw new Error('A completed primary removal lost its owning Panel.');
			const unavailableTargets = pendingTargetsExcept(state, primaryRemoval.removalId);
			const activeChild = panel.childPresentations.find(
				(terminal) => terminal.presentationId === panel.activeChildPresentationId,
			);
			const promoted =
				activeChild !== undefined && !unavailableTargets.has(activeChild.presentationId)
					? activeChild
					: panel.childPresentations.find(
							(terminal) => !unavailableTargets.has(terminal.presentationId),
						);
			if (promoted !== undefined) {
				const result = mapPanel(tab.root, primaryRemoval.panelId, (candidate) => ({
					...candidate,
					activeChildPresentationId: null,
					childPresentations: candidate.childPresentations.filter(
						(terminal) => terminal.presentationId !== promoted.presentationId,
					),
					primaryPresentation: promoted,
				}));
				state = withoutRemoval(
					replaceTab(state, tabIndex, { ...tab, root: result.node }),
					primaryRemoval.removalId,
				);
				primarySettled = true;
				break;
			}
			if (panel.childPresentations.length > 0) continue;
			if (collectPanelLeaves(tab.root).length === 1) {
				const nextState = convertRemovalToTab(state, primaryRemoval);
				return {
					commands: [
						{
							removalId: primaryRemoval.removalId,
							tabId: primaryRemoval.tabId,
							type: 'tab-close-ready',
						},
					],
					state: nextState,
				};
			}
			const removedState = removePanelFromTab(state, tabIndex, tab, primaryRemoval.panelId);
			if (removedState === null) {
				throw new Error('A completed primary removal could not remove its owning Panel.');
			}
			state = withoutRemoval(removedState, primaryRemoval.removalId);
			primarySettled = true;
			break;
		}
		if (primarySettled) continue;

		const panelRemoval = ready.find((removal) => removal.kind === 'panel');
		if (panelRemoval?.kind === 'panel') {
			if (collectPanelLeaves(tab.root).length === 1) {
				const nextState = convertRemovalToTab(state, panelRemoval);
				return {
					commands: [
						{
							removalId: panelRemoval.removalId,
							tabId: panelRemoval.tabId,
							type: 'tab-close-ready',
						},
					],
					state: nextState,
				};
			}
			const removedState = removePanelFromTab(state, tabIndex, tab, panelRemoval.panelId);
			if (removedState === null) {
				throw new Error('A completed Panel removal could not remove its Panel.');
			}
			state = withoutRemoval(removedState, panelRemoval.removalId);
			continue;
		}

		const tabRemoval = ready.find((removal) => removal.kind === 'tab');
		if (tabRemoval?.kind === 'tab') {
			return {
				commands: [
					{
						removalId: tabRemoval.removalId,
						tabId: tabRemoval.tabId,
						type: 'tab-close-ready',
					},
				],
				state,
			};
		}

		return { commands: [], state };
	}
}

function removalPreventsTerminalCreation(
	state: PanelWorkspaceState,
	tabId: string,
	panelId: string,
): boolean {
	return state.pendingRemovals.some(
		(removal) =>
			removal.tabId === tabId &&
			(removal.kind === 'tab' || (removal.kind === 'panel' && removal.panelId === panelId)),
	);
}

export function initializePanelWorkspace(
	tabs: readonly PanelWorkspaceTabInput[],
): PanelWorkspaceInitialization {
	let nextPanelSequence = 1;
	let nextTerminalSequence = 1;
	const commands: PanelWorkspaceCommand[] = [];
	const workspaces = tabs.map((tab): TabPanelWorkspace => {
		if (tab.presentation === undefined) {
			throw new Error(`Initial Tab ${tab.id} is missing a terminal presentation.`);
		}
		const creation = createPanel(nextPanelSequence++, tab.presentation, tab.title, tab.cwd ?? null);
		nextTerminalSequence += 1;
		if (creation.command !== null) commands.push(creation.command);
		return {
			focusedPanelId: creation.panel.id,
			maximizedPanelId: null,
			root: creation.panel,
			tabId: tab.id,
		};
	});
	const state = {
		nextPanelSequence,
		nextPanelSlotSequence: 1,
		nextSplitSequence: 1,
		nextTerminalSequence,
		pendingRemovals: [],
		tabs: workspaces,
	};
	const invariantErrors = validatePanelWorkspaceState(state);
	if (invariantErrors.length > 0) {
		throw new Error(`Panel workspace invariant violation: ${invariantErrors.join('; ')}`);
	}
	return { commands, state };
}

export function collectPanelLeaves(root: PanelLayoutNode): readonly PanelLeaf[] {
	const panels: PanelLeaf[] = [];
	collectPanelsFromNode(root, panels);
	return panels;
}

export function terminalRequiresCloseConfirmation(terminal: TerminalPresentation): boolean {
	if (terminal.lifecycle.kind === 'running') return terminal.closeActivity !== 'idle';
	return (
		terminal.lifecycle.kind === 'preview' ||
		terminal.lifecycle.kind === 'provisioning' ||
		terminal.lifecycle.kind === 'restoring' ||
		terminal.lifecycle.kind === 'reconciling'
	);
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

export function clonePanelWorkspaceState(
	state: PanelWorkspaceState,
	tabIdMap: ReadonlyMap<string, string>,
	identityPrefix: string,
	createPresentationId: () => TerminalPresentationId,
): PanelWorkspaceState {
	const identities: PanelWorkspaceCloneIdentities = {
		createPresentationId,
		panelIds: new Map(),
		panelSlotIds: new Map(),
		prefix: identityPrefix,
		splitIds: new Map(),
		presentationIds: new Map(),
	};
	return {
		...state,
		pendingRemovals: [],
		tabs: state.tabs.map((tab) => {
			const tabId = tabIdMap.get(tab.tabId);
			if (tabId === undefined) {
				throw new Error(`Panel workspace clone is missing Tab mapping for ${tab.tabId}.`);
			}
			return {
				focusedPanelId: cloneIdentity(
					identities.panelIds,
					identities.prefix,
					'panel',
					tab.focusedPanelId,
				),
				maximizedPanelId:
					tab.maximizedPanelId === null
						? null
						: cloneIdentity(identities.panelIds, identities.prefix, 'panel', tab.maximizedPanelId),
				root: clonePanelLayoutNode(tab.root, identities),
				tabId,
			};
		}),
	};
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
		terminal.naturalTitle ??
		deriveTerminalDirectoryTitle(terminal.currentCwd) ??
		deriveTerminalDirectoryTitle(terminal.launchCwd) ??
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

function canSettleTerminalLifecycle(lifecycle: TerminalPresentationLifecycle): boolean {
	return (
		lifecycle.kind === 'provisioning' ||
		lifecycle.kind === 'restoring' ||
		lifecycle.kind === 'reconciling' ||
		lifecycle.kind === 'running'
	);
}

function failuresEqual(
	left: TerminalPresentationFailure,
	right: TerminalPresentationFailure,
): boolean {
	return left.code === right.code && left.retryable === right.retryable;
}

export function transitionPanelWorkspace(
	state: PanelWorkspaceState,
	event: PanelWorkspaceEvent,
): PanelWorkspaceTransition {
	if (event.type === 'terminal-sessions-stop-requested') {
		return beginManagedStop(state, event.operations);
	}

	if (event.type === 'tabs-reconciled') {
		const existingByTabId = new Map(state.tabs.map((tab) => [tab.tabId, tab]));
		const requestedTabIds = new Set(event.tabs.map((tab) => tab.id));
		for (const existing of state.tabs) {
			if (requestedTabIds.has(existing.tabId)) continue;
			const presentationIds = collectPresentationIds(existing.root);
			if (
				presentationIds.every(
					(presentationId) =>
						findTerminalInNode(existing.root, presentationId)?.lifecycle.kind === 'preview',
				)
			) {
				continue;
			}
			const removal = state.pendingRemovals.find(
				(candidate) => candidate.kind === 'tab' && candidate.tabId === existing.tabId,
			);
			if (
				removal === undefined ||
				!removal.presentationIds.every(
					(presentationId) =>
						findTerminalInNode(existing.root, presentationId)?.lifecycle.kind === 'closed',
				)
			) {
				return rejected(state, 'terminal-removal-required');
			}
		}
		if (event.tabs.some((tab) => !existingByTabId.has(tab.id) && tab.presentation === undefined)) {
			return rejected(state, 'terminal-presentation-required');
		}
		let nextPanelSequence = state.nextPanelSequence;
		let nextTerminalSequence = state.nextTerminalSequence;
		const commands: PanelWorkspaceCommand[] = [];
		const tabs = event.tabs.map((tab): TabPanelWorkspace => {
			const existing = existingByTabId.get(tab.id);
			if (existing !== undefined) {
				return existing;
			}
			if (tab.presentation === undefined) {
				throw new Error(`New Tab ${tab.id} is missing a terminal presentation.`);
			}
			const creation = createPanel(
				nextPanelSequence++,
				tab.presentation,
				tab.title,
				tab.cwd ?? null,
			);
			nextTerminalSequence += 1;
			if (creation.command !== null) commands.push(creation.command);
			return {
				focusedPanelId: creation.panel.id,
				maximizedPanelId: null,
				root: creation.panel,
				tabId: tab.id,
			};
		});
		const isUnchanged =
			nextPanelSequence === state.nextPanelSequence &&
			tabs.length === state.tabs.length &&
			tabs.every((tab, index) => tab === state.tabs[index]);
		const retainedTabIds = new Set(tabs.map((tab) => tab.tabId));
		const pendingRemovals = state.pendingRemovals.filter((removal) =>
			retainedTabIds.has(removal.tabId),
		);
		return isUnchanged && pendingRemovals.length === state.pendingRemovals.length
			? unchanged(state)
			: applied(
					{ ...state, nextPanelSequence, nextTerminalSequence, pendingRemovals, tabs },
					commands,
				);
	}

	const tabIndex = state.tabs.findIndex((tab) => tab.tabId === event.tabId);
	const tab = state.tabs[tabIndex];
	if (tab === undefined) {
		return rejected(state, 'tab-not-found');
	}

	if (event.type === 'panel-split') {
		const sourcePanel = findPanelInNode(tab.root, event.panelId);
		if (sourcePanel === null) {
			return rejected(state, 'panel-not-found');
		}
		if (removalPreventsTerminalCreation(state, event.tabId, event.panelId)) {
			return rejected(state, 'terminal-removal-conflict');
		}
		const sourceTerminal = getDisplayedTerminal(sourcePanel);
		const creation = createPanel(
			state.nextPanelSequence,
			event.presentation,
			event.newTerminalTitle,
			sourceTerminal.currentCwd,
		);
		const newPanel = creation.panel;
		const result = insertPanelNode(
			tab.root,
			event.panelId,
			event.direction,
			newPanel,
			`split-${state.nextSplitSequence}`,
			{
				insertedPanelSlotId: `panel-slot-${state.nextPanelSlotSequence}`,
				targetPanelSlotId: `panel-slot-${state.nextPanelSlotSequence + 1}`,
			},
		);
		if (!result.changed) {
			return rejected(state, 'panel-not-found');
		}
		return applied(
			{
				...replaceTab(state, tabIndex, {
					...tab,
					focusedPanelId: newPanel.id,
					maximizedPanelId: null,
					root: result.node,
				}),
				nextPanelSequence: state.nextPanelSequence + 1,
				nextPanelSlotSequence: state.nextPanelSlotSequence + 2,
				nextSplitSequence: state.nextSplitSequence + 1,
				nextTerminalSequence: state.nextTerminalSequence + 1,
			},
			creation.command === null ? [] : [creation.command],
		);
	}

	if (event.type === 'panel-moved') {
		if (event.panelId === event.targetPanelId) {
			return unchanged(state);
		}
		const panel = findPanelInNode(tab.root, event.panelId);
		const targetPanel = findPanelInNode(tab.root, event.targetPanelId);
		if (panel === null || targetPanel === null) {
			return rejected(state, 'panel-not-found');
		}
		if (isPanelAtMoveTarget(tab.root, event.panelId, event.targetPanelId, event.direction)) {
			return unchanged(state);
		}
		const removal = removePanelFromNode(tab.root, event.panelId);
		if (!removal.removed || removal.node === null) {
			return rejected(state, 'panel-not-found');
		}
		const result = insertPanelNode(
			removal.node,
			event.targetPanelId,
			event.direction,
			panel,
			`split-${state.nextSplitSequence}`,
			{
				insertedPanelSlotId: `panel-slot-${state.nextPanelSlotSequence}`,
				targetPanelSlotId: `panel-slot-${state.nextPanelSlotSequence + 1}`,
			},
		);
		if (!result.changed) {
			return rejected(state, 'panel-not-found');
		}
		return applied({
			...replaceTab(state, tabIndex, {
				...tab,
				focusedPanelId: panel.id,
				maximizedPanelId: null,
				root: result.node,
			}),
			nextPanelSlotSequence: state.nextPanelSlotSequence + 2,
			nextSplitSequence: state.nextSplitSequence + 1,
		});
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

	if (event.type === 'terminal-current-cwd-changed') {
		const result = mapTerminal(tab.root, event.presentationId, (terminal) =>
			terminal.currentCwd === event.currentCwd
				? terminal
				: { ...terminal, currentCwd: event.currentCwd },
		);
		if (!result.changed) {
			return findTerminalInNode(tab.root, event.presentationId) === null
				? rejected(state, 'terminal-not-found')
				: unchanged(state);
		}
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
	}

	if (event.type === 'terminal-stop-completed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'stopping' ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			kind: 'stopped',
		});
	}

	if (event.type === 'terminal-stop-failed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'stopping' ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			failure: event.failure,
			kind: 'stop-failed',
			operationId: event.operationId,
		});
	}

	if (event.type === 'terminal-stop-retried') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'stop-failed' ||
			!terminal.lifecycle.failure.retryable ||
			terminal.lifecycle.operationId === event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(
			state,
			tabIndex,
			tab,
			event.presentationId,
			{ kind: 'stopping', operationId: event.operationId },
			[
				{
					disposition: 'stop',
					operationId: event.operationId,
					presentationId: event.presentationId,
					type: 'terminal-close-requested',
				},
			],
		);
	}

	if (event.type === 'terminal-stopped-session-started') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (terminal.lifecycle.kind !== 'stopped') {
			return rejected(state, 'terminal-session-not-stopped');
		}
		if (
			state.tabs.some(
				(candidate) =>
					findTerminalInNode(candidate.root, event.presentation.presentationId) !== null,
			)
		) {
			return rejected(state, 'invalid-close-operations');
		}
		const result = replaceStoppedTerminal(tab.root, event.presentationId, event.presentation);
		if (!result.changed) return rejected(state, 'terminal-session-not-stopped');
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }), [
			{
				launchCwd: terminal.launchCwd,
				operationId: event.presentation.operationId,
				presentationId: event.presentation.presentationId,
				type: 'terminal-provision-requested',
			},
		]);
	}

	if (event.type === 'terminal-provision-completed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'provisioning' ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			kind: 'running',
		});
	}

	if (event.type === 'terminal-provision-failed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'provisioning' ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			failure: event.failure,
			kind: 'provision-failed',
			operationId: event.operationId,
		});
	}

	if (event.type === 'terminal-provision-retried') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'provision-failed' ||
			!terminal.lifecycle.failure.retryable ||
			terminal.lifecycle.operationId === event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(
			state,
			tabIndex,
			tab,
			event.presentationId,
			{ kind: 'provisioning', operationId: event.operationId },
			[
				{
					launchCwd: terminal.launchCwd,
					operationId: event.operationId,
					presentationId: event.presentationId,
					type: 'terminal-provision-requested',
				},
			],
		);
	}

	if (event.type === 'terminal-restore-requested') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'restoring' &&
			(terminal.lifecycle.kind !== 'restore-failed' ||
				!terminal.lifecycle.failure.retryable ||
				terminal.lifecycle.operationId === event.operationId)
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(
			state,
			tabIndex,
			tab,
			event.presentationId,
			{ kind: 'reconciling', operationId: event.operationId },
			[
				{
					launchCwd: terminal.launchCwd,
					operationId: event.operationId,
					presentationId: event.presentationId,
					type: 'terminal-restore-requested',
				},
			],
		);
	}

	if (event.type === 'terminal-restore-started') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (terminal.lifecycle.kind === 'reconciling') {
			return terminal.lifecycle.operationId === event.operationId
				? unchanged(state)
				: rejected(state, 'stale-operation');
		}
		if (terminal.lifecycle.kind !== 'restoring') return rejected(state, 'stale-operation');
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			kind: 'reconciling',
			operationId: event.operationId,
		});
	}

	if (event.type === 'terminal-restore-completed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'reconciling' ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			kind: 'running',
		});
	}

	if (event.type === 'terminal-restore-failed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			terminal.lifecycle.kind !== 'reconciling' ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			failure: event.failure,
			kind: 'restore-failed',
			operationId: event.operationId,
		});
	}

	if (event.type === 'terminal-exited') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (terminal.lifecycle.kind === 'exited') {
			if (
				terminal.lifecycle.exitCode !== event.exitCode ||
				terminal.lifecycle.signal !== event.signal
			) {
				return rejected(state, 'stale-operation');
			}
		} else if (!canSettleTerminalLifecycle(terminal.lifecycle)) {
			return rejected(state, 'stale-operation');
		}
		const panel = collectPanelLeaves(tab.root).find(
			(candidate) =>
				candidate.primaryPresentation.presentationId === event.presentationId ||
				candidate.childPresentations.some((child) => child.presentationId === event.presentationId),
		);
		if (panel === undefined) return rejected(state, 'panel-not-found');
		const isPrimary = panel.primaryPresentation.presentationId === event.presentationId;
		if (!isPrimary) {
			return beginManagedRemoval(
				state,
				tabIndex,
				tab,
				{
					childPresentationId: event.presentationId,
					kind: 'child',
					panelId: panel.id,
					presentationIds: [event.presentationId],
					removalId: event.close.removalId,
					tabId: event.tabId,
				},
				event.close,
			);
		}
		if (panel.childPresentations.length > 0) {
			return beginManagedRemoval(
				state,
				tabIndex,
				tab,
				{
					kind: 'primary',
					panelId: panel.id,
					presentationIds: [event.presentationId],
					removalId: event.close.removalId,
					tabId: event.tabId,
				},
				event.close,
			);
		}
		return beginManagedRemoval(
			state,
			tabIndex,
			tab,
			collectPanelLeaves(tab.root).length === 1
				? {
						kind: 'tab',
						presentationIds: [event.presentationId],
						removalId: event.close.removalId,
						tabId: event.tabId,
					}
				: {
						kind: 'panel',
						panelId: panel.id,
						presentationIds: [event.presentationId],
						removalId: event.close.removalId,
						tabId: event.tabId,
					},
			event.close,
		);
	}

	if (event.type === 'terminal-failed') {
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (terminal.lifecycle.kind === 'failed') {
			return failuresEqual(terminal.lifecycle.failure, event.failure)
				? unchanged(state)
				: rejected(state, 'stale-operation');
		}
		if (!canSettleTerminalLifecycle(terminal.lifecycle)) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			failure: event.failure,
			kind: 'failed',
		});
	}

	if (event.type === 'terminal-close-failed') {
		const removal = state.pendingRemovals.find(
			(candidate) => candidate.removalId === event.removalId && candidate.tabId === event.tabId,
		);
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			removal === undefined ||
			!removal.presentationIds.includes(event.presentationId) ||
			terminal.lifecycle.kind !== 'closing' ||
			terminal.lifecycle.removalId !== event.removalId ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(state, tabIndex, tab, event.presentationId, {
			failure: event.failure,
			kind: 'close-failed',
			operationId: event.operationId,
			removalId: event.removalId,
		});
	}

	if (event.type === 'terminal-close-retried') {
		const removal = state.pendingRemovals.find(
			(candidate) => candidate.removalId === event.removalId && candidate.tabId === event.tabId,
		);
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			removal === undefined ||
			terminal.lifecycle.kind !== 'close-failed' ||
			terminal.lifecycle.removalId !== event.removalId ||
			!terminal.lifecycle.failure.retryable ||
			terminal.lifecycle.operationId === event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		return replaceTerminalLifecycle(
			state,
			tabIndex,
			tab,
			event.presentationId,
			{
				kind: 'closing',
				operationId: event.operationId,
				removalId: event.removalId,
			},
			[
				{
					disposition: 'remove',
					operationId: event.operationId,
					presentationId: event.presentationId,
					type: 'terminal-close-requested',
				},
			],
		);
	}

	if (event.type === 'terminal-close-completed') {
		const removal = state.pendingRemovals.find(
			(candidate) => candidate.removalId === event.removalId && candidate.tabId === event.tabId,
		);
		const terminal = findTerminalInNode(tab.root, event.presentationId);
		if (terminal === null) return rejected(state, 'terminal-not-found');
		if (
			removal === undefined ||
			!removal.presentationIds.includes(event.presentationId) ||
			terminal.lifecycle.kind !== 'closing' ||
			terminal.lifecycle.removalId !== event.removalId ||
			terminal.lifecycle.operationId !== event.operationId
		) {
			return rejected(state, 'stale-operation');
		}
		const root = mapTerminal(tab.root, event.presentationId, (candidate) => ({
			...candidate,
			lifecycle: {
				kind: 'closed',
				operationId: event.operationId,
				removalId: event.removalId,
			},
		})).node;
		const nextState = replaceTab(state, tabIndex, { ...tab, root });
		const settled = settleCompletedRemovals(nextState, event.tabId);
		return applied(settled.state, settled.commands);
	}

	if (event.type === 'tab-close-requested') {
		const presentationIds = collectPresentationIds(tab.root);
		return beginManagedRemoval(
			state,
			tabIndex,
			tab,
			{
				kind: 'tab',
				presentationIds,
				removalId: event.close.removalId,
				tabId: event.tabId,
			},
			event.close,
		);
	}

	if (event.type === 'adjacent-panels-resized') {
		const result = resizeSplitNode(
			tab.root,
			event.splitId,
			event.separatorIndex,
			event.leadingWeight,
		);
		if (typeof result === 'string') {
			return rejected(state, result);
		}
		if (!result.changed) {
			const splitExists = (function findSplit(node: PanelLayoutNode): boolean {
				return (
					(node.kind === 'split' && node.id === event.splitId) ||
					(node.kind === 'split' && node.children.some((child) => findSplit(child.node)))
				);
			})(tab.root);
			return splitExists ? unchanged(state) : rejected(state, 'split-not-found');
		}
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
	}

	const panel = findPanelInNode(tab.root, event.panelId);
	if (panel === null) {
		return rejected(state, 'panel-not-found');
	}

	if (event.type === 'child-terminal-created') {
		if (removalPreventsTerminalCreation(state, event.tabId, event.panelId)) {
			return rejected(state, 'terminal-removal-conflict');
		}
		const creation = createTerminal(
			event.presentation,
			event.newTerminalTitle,
			getDisplayedTerminal(panel).currentCwd,
		);
		const terminal = creation.terminal;
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildPresentationId: terminal.presentationId,
			childPresentations: [...currentPanel.childPresentations, terminal],
		}));
		return applied(
			{
				...replaceTab(state, tabIndex, {
					...tab,
					focusedPanelId: event.panelId,
					root: result.node,
				}),
				nextTerminalSequence: state.nextTerminalSequence + 1,
			},
			creation.command === null ? [] : [creation.command],
		);
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
				...tab,
				focusedPanelId: event.panelId,
				root: result.node,
			}),
		);
	}

	if (event.type === 'child-terminal-closed') {
		const childIndex = panel.childPresentations.findIndex(
			(terminal) => terminal.presentationId === event.childPresentationId,
		);
		if (childIndex === -1) {
			return rejected(state, 'child-terminal-not-found');
		}
		if (event.close.kind === 'managed') {
			return beginManagedRemoval(
				state,
				tabIndex,
				tab,
				{
					childPresentationId: event.childPresentationId,
					kind: 'child',
					panelId: event.panelId,
					presentationIds: [event.childPresentationId],
					removalId: event.close.removalId,
					tabId: event.tabId,
				},
				event.close,
			);
		}
		const childPresentations = [...panel.childPresentations];
		childPresentations.splice(childIndex, 1);
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildPresentationId:
				currentPanel.activeChildPresentationId === event.childPresentationId
					? null
					: currentPanel.activeChildPresentationId,
			childPresentations,
		}));
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
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
			: applied(replaceTab(state, tabIndex, { ...tab, focusedPanelId: event.panelId }));
	}

	if (event.type === 'panel-maximization-toggled') {
		const maximizedPanelId = tab.maximizedPanelId === event.panelId ? null : event.panelId;
		return applied(
			replaceTab(state, tabIndex, {
				...tab,
				focusedPanelId: event.panelId,
				maximizedPanelId,
			}),
		);
	}

	const panels = collectPanelLeaves(tab.root);
	if (panels.length === 1) {
		return rejected(state, 'last-panel');
	}
	if (event.close.kind === 'managed') {
		return beginManagedRemoval(
			state,
			tabIndex,
			tab,
			{
				kind: 'panel',
				panelId: event.panelId,
				presentationIds: [
					panel.primaryPresentation.presentationId,
					...panel.childPresentations.map((terminal) => terminal.presentationId),
				],
				removalId: event.close.removalId,
				tabId: event.tabId,
			},
			event.close,
		);
	}
	const removedState = removePanelFromTab(state, tabIndex, tab, event.panelId);
	return removedState === null ? rejected(state, 'panel-not-found') : applied(removedState);
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
	const removalPresentationIds = new Set<TerminalPresentationId>();
	const removalIds = new Set<TerminalPresentationRemovalId>();
	for (const removal of state.pendingRemovals) {
		if (removalIds.has(removal.removalId)) {
			errors.push(`Duplicate terminal removal id: ${removal.removalId}`);
		}
		removalIds.add(removal.removalId);
		const tab = state.tabs.find((candidate) => candidate.tabId === removal.tabId);
		if (tab === undefined) {
			errors.push(`Removal references missing tab: ${removal.removalId}`);
			continue;
		}
		if (removal.presentationIds.length === 0) {
			errors.push(`Removal has no terminal presentations: ${removal.removalId}`);
		}
		const expectedPresentationIds = (() => {
			if (removal.kind === 'tab') return new Set(collectPresentationIds(tab.root));
			const panel = findPanelInNode(tab.root, removal.panelId);
			if (panel === null) return null;
			if (removal.kind === 'child') {
				return panel.childPresentations.some(
					(terminal) => terminal.presentationId === removal.childPresentationId,
				)
					? new Set([removal.childPresentationId])
					: null;
			}
			if (removal.kind === 'primary') {
				return new Set([panel.primaryPresentation.presentationId]);
			}
			return new Set([
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((terminal) => terminal.presentationId),
			]);
		})();
		const actualPresentationIds = new Set(removal.presentationIds);
		if (
			expectedPresentationIds === null ||
			expectedPresentationIds.size !== actualPresentationIds.size ||
			![...expectedPresentationIds].every((presentationId) =>
				actualPresentationIds.has(presentationId),
			)
		) {
			errors.push(`Removal target mismatch: ${removal.removalId}`);
		}
		for (const presentationId of removal.presentationIds) {
			if (removalPresentationIds.has(presentationId)) {
				errors.push(`Terminal presentation belongs to overlapping removals: ${presentationId}`);
			}
			removalPresentationIds.add(presentationId);
			const terminal = findTerminalInNode(tab.root, presentationId);
			if (terminal === null) {
				errors.push(`Removal references missing terminal presentation: ${presentationId}`);
				continue;
			}
			if (
				(terminal.lifecycle.kind !== 'closing' &&
					terminal.lifecycle.kind !== 'close-failed' &&
					terminal.lifecycle.kind !== 'closed') ||
				terminal.lifecycle.removalId !== removal.removalId
			) {
				errors.push(`Removal lifecycle mismatch: ${presentationId}`);
			}
		}
	}
	return errors;
}

function cloneIdentity(
	identities: Map<string, string>,
	prefix: string,
	kind: string,
	sourceId: string,
): string {
	const existingId = identities.get(sourceId);
	if (existingId !== undefined) {
		return existingId;
	}
	const clonedId = `${prefix}-${kind}-${identities.size + 1}`;
	identities.set(sourceId, clonedId);
	return clonedId;
}

function clonePanelLayoutNode(
	node: PanelLayoutNode,
	identities: PanelWorkspaceCloneIdentities,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			activeChildPresentationId:
				node.activeChildPresentationId === null
					? null
					: clonePresentationIdentity(identities, node.activeChildPresentationId),
			childPresentations: node.childPresentations.map((terminal) => ({
				closeActivity: 'unknown',
				currentCwd: terminal.launchCwd,
				launchCwd: terminal.launchCwd,
				lifecycle: { kind: 'preview' },
				naturalTitle: null,
				presentationId: clonePresentationIdentity(identities, terminal.presentationId),
				title: terminal.title,
			})),
			id: cloneIdentity(identities.panelIds, identities.prefix, 'panel', node.id),
			kind: node.kind,
			primaryPresentation: {
				closeActivity: 'unknown',
				currentCwd: node.primaryPresentation.launchCwd,
				launchCwd: node.primaryPresentation.launchCwd,
				lifecycle: { kind: 'preview' },
				naturalTitle: null,
				presentationId: clonePresentationIdentity(
					identities,
					node.primaryPresentation.presentationId,
				),
				title: node.primaryPresentation.title,
			},
		};
	}
	return {
		id: cloneIdentity(identities.splitIds, identities.prefix, 'split', node.id),
		children: node.children.map((child) => ({
			...child,
			id: cloneIdentity(identities.panelSlotIds, identities.prefix, 'panel-slot', child.id),
			node: clonePanelLayoutNode(child.node, identities),
		})),
		kind: node.kind,
		orientation: node.orientation,
	};
}

function clonePresentationIdentity(
	identities: PanelWorkspaceCloneIdentities,
	sourceId: TerminalPresentationId,
): TerminalPresentationId {
	const existingId = identities.presentationIds.get(sourceId);
	if (existingId !== undefined) return existingId;
	const clonedId = identities.createPresentationId();
	identities.presentationIds.set(sourceId, clonedId);
	return clonedId;
}
