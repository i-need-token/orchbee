export type PanelSplitDirection = 'above' | 'below' | 'left' | 'right';
type PanelSplitOrientation = 'horizontal' | 'vertical';
type TerminalPhase = 'exited' | 'failed' | 'idle' | 'provisioning' | 'running';

export interface PanelTerminal {
	readonly currentCwd: string | null;
	readonly id: string;
	readonly launchCwd: string | null;
	readonly phase: TerminalPhase;
	readonly title: string;
}

export interface PanelLeaf {
	readonly activeChildTerminalId: string | null;
	readonly childTerminals: readonly PanelTerminal[];
	readonly id: string;
	readonly kind: 'panel';
	readonly primaryTerminal: PanelTerminal;
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
	readonly tabs: readonly TabPanelWorkspace[];
}

export interface PanelWorkspaceTabInput {
	readonly cwd?: string | null | undefined;
	readonly id: string;
	readonly title: string;
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
			readonly tabId: string;
			readonly type: 'child-terminal-created';
	  }
	| {
			readonly childTerminalId: string;
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'child-terminal-activated';
	  }
	| {
			readonly panelId: string;
			readonly tabId: string;
			readonly type: 'child-terminal-closed';
			readonly childTerminalId: string;
	  }
	| {
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
			readonly tabId: string;
			readonly terminalId: string;
			readonly type: 'terminal-current-cwd-changed';
	  }
	| {
			readonly phase: TerminalPhase;
			readonly tabId: string;
			readonly terminalId: string;
			readonly type: 'terminal-phase-changed';
	  }
	| {
			readonly tabs: readonly PanelWorkspaceTabInput[];
			readonly type: 'tabs-reconciled';
	  };

type PanelWorkspaceRejection =
	| 'child-terminal-not-found'
	| 'invalid-resize'
	| 'last-panel'
	| 'panel-not-found'
	| 'split-not-found'
	| 'tab-not-found'
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

interface PanelWorkspaceCloneIdentities {
	readonly panelIds: Map<string, string>;
	readonly panelSlotIds: Map<string, string>;
	readonly prefix: string;
	readonly splitIds: Map<string, string>;
	readonly terminalIds: Map<string, string>;
}

const WEIGHT_TOTAL = 100;
const WEIGHT_TOLERANCE = 0.000_001;

function createTerminal(sequence: number, title: string, cwd: string | null): PanelTerminal {
	return {
		currentCwd: cwd,
		id: `terminal-${sequence}`,
		launchCwd: cwd,
		phase: 'running',
		title,
	};
}

function createPanel(
	panelSequence: number,
	terminalSequence: number,
	title: string,
	cwd: string | null,
): PanelLeaf {
	return {
		activeChildTerminalId: null,
		childTerminals: [],
		id: `panel-${panelSequence}`,
		kind: 'panel',
		primaryTerminal: createTerminal(terminalSequence, title, cwd),
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

function mapTerminalPhase(
	node: PanelLayoutNode,
	terminalId: string,
	phase: TerminalPhase,
): TreeUpdate {
	if (node.kind === 'panel') {
		if (node.primaryTerminal.id === terminalId) {
			return {
				changed: node.primaryTerminal.phase !== phase,
				node:
					node.primaryTerminal.phase === phase
						? node
						: {
								...node,
								primaryTerminal: { ...node.primaryTerminal, phase },
							},
			};
		}
		const terminalIndex = node.childTerminals.findIndex((terminal) => terminal.id === terminalId);
		const terminal = node.childTerminals[terminalIndex];
		if (terminal === undefined) {
			return { changed: false, node };
		}
		if (terminal.phase === phase) {
			return { changed: false, node };
		}
		const childTerminals = [...node.childTerminals];
		childTerminals[terminalIndex] = { ...terminal, phase };
		return { changed: true, node: { ...node, childTerminals } };
	}
	for (const [index, child] of node.children.entries()) {
		const result = mapTerminalPhase(child.node, terminalId, phase);
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}
	return { changed: false, node };
}

function mapTerminalCurrentCwd(
	node: PanelLayoutNode,
	terminalId: string,
	currentCwd: string | null,
): TreeUpdate {
	if (node.kind === 'panel') {
		if (node.primaryTerminal.id === terminalId) {
			return {
				changed: node.primaryTerminal.currentCwd !== currentCwd,
				node:
					node.primaryTerminal.currentCwd === currentCwd
						? node
						: {
								...node,
								primaryTerminal: { ...node.primaryTerminal, currentCwd },
							},
			};
		}
		const terminalIndex = node.childTerminals.findIndex((terminal) => terminal.id === terminalId);
		const terminal = node.childTerminals[terminalIndex];
		if (terminal === undefined) {
			return { changed: false, node };
		}
		if (terminal.currentCwd === currentCwd) {
			return { changed: false, node };
		}
		const childTerminals = [...node.childTerminals];
		childTerminals[terminalIndex] = { ...terminal, currentCwd };
		return { changed: true, node: { ...node, childTerminals } };
	}
	for (const [index, child] of node.children.entries()) {
		const result = mapTerminalCurrentCwd(child.node, terminalId, currentCwd);
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}
	return { changed: false, node };
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
		if (
			node.orientation === orientation &&
			child.node.kind === 'panel' &&
			child.node.id === panelId
		) {
			const nextWeight = child.weight / 2;
			const sourceChild = { ...child, weight: nextWeight };
			const newChild = {
				id: slotIds.insertedPanelSlotId,
				node: newPanel,
				weight: nextWeight,
			};
			const children = [...node.children];
			children.splice(
				index,
				1,
				...(directionInsertsBefore(direction) ? [newChild, sourceChild] : [sourceChild, newChild]),
			);
			return { changed: true, node: { ...node, children } };
		}

		const result = insertPanelNode(child.node, panelId, direction, newPanel, splitId, slotIds);
		if (result.changed) {
			const children = [...node.children];
			children[index] = { ...child, node: result.node };
			return { changed: true, node: { ...node, children } };
		}
	}

	return { changed: false, node };
}

function normalizeSplit(split: PanelSplit): PanelLayoutNode {
	const flattenedChildren = split.children.flatMap((child): readonly PanelSplitChild[] => {
		if (child.node.kind !== 'split' || child.node.orientation !== split.orientation) {
			return [child];
		}
		return child.node.children.map((nestedChild) => ({
			id: nestedChild.id,
			node: nestedChild.node,
			weight: (child.weight * nestedChild.weight) / WEIGHT_TOTAL,
		}));
	});
	const currentTotal = flattenedChildren.reduce((sum, child) => sum + child.weight, 0);
	const children = flattenedChildren.map((child) => ({
		id: child.id,
		node: child.node,
		weight:
			currentTotal <= WEIGHT_TOLERANCE
				? WEIGHT_TOTAL / flattenedChildren.length
				: (child.weight * WEIGHT_TOTAL) / currentTotal,
	}));
	const onlyChild = children.length === 1 ? children[0] : undefined;
	return onlyChild === undefined ? { ...split, children } : onlyChild.node;
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
			node: normalizeSplit({ ...node, children }),
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

export function createPanelWorkspaceState(
	tabs: readonly PanelWorkspaceTabInput[],
): PanelWorkspaceState {
	let nextPanelSequence = 1;
	let nextTerminalSequence = 1;
	const workspaces = tabs.map((tab): TabPanelWorkspace => {
		const panel = createPanel(
			nextPanelSequence++,
			nextTerminalSequence++,
			tab.title,
			tab.cwd ?? null,
		);
		return {
			focusedPanelId: panel.id,
			maximizedPanelId: null,
			root: panel,
			tabId: tab.id,
		};
	});
	const state = {
		nextPanelSequence,
		nextPanelSlotSequence: 1,
		nextSplitSequence: 1,
		nextTerminalSequence,
		tabs: workspaces,
	};
	const invariantErrors = validatePanelWorkspaceState(state);
	if (invariantErrors.length > 0) {
		throw new Error(`Panel workspace invariant violation: ${invariantErrors.join('; ')}`);
	}
	return state;
}

export function collectPanelLeaves(root: PanelLayoutNode): readonly PanelLeaf[] {
	const panels: PanelLeaf[] = [];
	collectPanelsFromNode(root, panels);
	return panels;
}

export function clonePanelWorkspaceState(
	state: PanelWorkspaceState,
	tabIdMap: ReadonlyMap<string, string>,
	identityPrefix: string,
): PanelWorkspaceState {
	const identities: PanelWorkspaceCloneIdentities = {
		panelIds: new Map(),
		panelSlotIds: new Map(),
		prefix: identityPrefix,
		splitIds: new Map(),
		terminalIds: new Map(),
	};
	return {
		...state,
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

export function getDisplayedTerminal(panel: PanelLeaf): PanelTerminal {
	return (
		panel.childTerminals.find((terminal) => terminal.id === panel.activeChildTerminalId) ??
		panel.primaryTerminal
	);
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
		let nextPanelSequence = state.nextPanelSequence;
		let nextTerminalSequence = state.nextTerminalSequence;
		const tabs = event.tabs.map((tab): TabPanelWorkspace => {
			const existing = existingByTabId.get(tab.id);
			if (existing !== undefined) {
				return existing;
			}
			const panel = createPanel(
				nextPanelSequence++,
				nextTerminalSequence++,
				tab.title,
				tab.cwd ?? null,
			);
			return {
				focusedPanelId: panel.id,
				maximizedPanelId: null,
				root: panel,
				tabId: tab.id,
			};
		});
		const isUnchanged =
			nextPanelSequence === state.nextPanelSequence &&
			tabs.length === state.tabs.length &&
			tabs.every((tab, index) => tab === state.tabs[index]);
		return isUnchanged
			? unchanged(state)
			: applied({ ...state, nextPanelSequence, nextTerminalSequence, tabs });
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
		const sourceTerminal = getDisplayedTerminal(sourcePanel);
		const newPanel = createPanel(
			state.nextPanelSequence,
			state.nextTerminalSequence,
			event.newTerminalTitle,
			sourceTerminal.currentCwd,
		);
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
		return applied({
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
		});
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

	if (event.type === 'terminal-phase-changed') {
		const result = mapTerminalPhase(tab.root, event.terminalId, event.phase);
		if (!result.changed) {
			const terminalExists = collectPanelLeaves(tab.root).some(
				(panel) =>
					panel.primaryTerminal.id === event.terminalId ||
					panel.childTerminals.some((terminal) => terminal.id === event.terminalId),
			);
			return terminalExists ? unchanged(state) : rejected(state, 'terminal-not-found');
		}
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
	}

	if (event.type === 'terminal-current-cwd-changed') {
		const result = mapTerminalCurrentCwd(tab.root, event.terminalId, event.currentCwd);
		if (!result.changed) {
			const terminalExists = collectPanelLeaves(tab.root).some(
				(panel) =>
					panel.primaryTerminal.id === event.terminalId ||
					panel.childTerminals.some((terminal) => terminal.id === event.terminalId),
			);
			return terminalExists ? unchanged(state) : rejected(state, 'terminal-not-found');
		}
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
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
		const terminal = createTerminal(
			state.nextTerminalSequence,
			event.newTerminalTitle,
			getDisplayedTerminal(panel).currentCwd,
		);
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildTerminalId: terminal.id,
			childTerminals: [...currentPanel.childTerminals, terminal],
		}));
		return applied({
			...replaceTab(state, tabIndex, {
				...tab,
				focusedPanelId: event.panelId,
				root: result.node,
			}),
			nextTerminalSequence: state.nextTerminalSequence + 1,
		});
	}

	if (event.type === 'child-terminal-activated') {
		if (!panel.childTerminals.some((terminal) => terminal.id === event.childTerminalId)) {
			return rejected(state, 'child-terminal-not-found');
		}
		if (panel.activeChildTerminalId === event.childTerminalId) {
			return unchanged(state);
		}
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildTerminalId: event.childTerminalId,
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
		const childIndex = panel.childTerminals.findIndex(
			(terminal) => terminal.id === event.childTerminalId,
		);
		if (childIndex === -1) {
			return rejected(state, 'child-terminal-not-found');
		}
		const childTerminals = [...panel.childTerminals];
		childTerminals.splice(childIndex, 1);
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildTerminalId:
				currentPanel.activeChildTerminalId === event.childTerminalId
					? null
					: currentPanel.activeChildTerminalId,
			childTerminals,
		}));
		return applied(replaceTab(state, tabIndex, { ...tab, root: result.node }));
	}

	if (event.type === 'primary-terminal-shown') {
		if (panel.activeChildTerminalId === null) {
			return unchanged(state);
		}
		const result = mapPanel(tab.root, event.panelId, (currentPanel) => ({
			...currentPanel,
			activeChildTerminalId: null,
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
	const removedIndex = panels.findIndex((candidate) => candidate.id === event.panelId);
	const nextFocusedPanel = panels[removedIndex + 1] ?? panels[removedIndex - 1];
	const result = removePanelFromNode(tab.root, event.panelId);
	if (!result.removed || result.node === null || nextFocusedPanel === undefined) {
		return rejected(state, 'panel-not-found');
	}
	return applied(
		replaceTab(state, tabIndex, {
			...tab,
			focusedPanelId:
				tab.focusedPanelId === event.panelId ? nextFocusedPanel.id : tab.focusedPanelId,
			maximizedPanelId: tab.maximizedPanelId === event.panelId ? null : tab.maximizedPanelId,
			root: result.node,
		}),
	);
}

function validatePanelWorkspaceState(state: PanelWorkspaceState): readonly string[] {
	const errors: string[] = [];
	const tabIds = new Set<string>();
	const panelIds = new Set<string>();
	const panelSlotIds = new Set<string>();
	const splitIds = new Set<string>();
	const terminalIds = new Set<string>();
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
				const childIds = new Set(node.childTerminals.map((terminal) => terminal.id));
				if (node.activeChildTerminalId !== null && !childIds.has(node.activeChildTerminalId)) {
					errors.push(`Missing active child terminal: ${node.activeChildTerminalId}`);
				}
				for (const terminal of [node.primaryTerminal, ...node.childTerminals]) {
					if (terminalIds.has(terminal.id)) {
						errors.push(`Duplicate terminal id: ${terminal.id}`);
					}
					terminalIds.add(terminal.id);
				}
				return;
			}
			if (splitIds.has(node.id)) {
				errors.push(`Duplicate split id: ${node.id}`);
			}
			splitIds.add(node.id);
			if (node.children.length < 2) {
				errors.push(`Split has fewer than two children: ${node.id}`);
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
			for (const child of node.children) {
				if (child.node.kind === 'split' && child.node.orientation === node.orientation) {
					errors.push(`Nested split repeats orientation: ${child.node.id}`);
				}
				visit(child.node);
			}
		};
		visit(tab.root);
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
			activeChildTerminalId:
				node.activeChildTerminalId === null
					? null
					: cloneIdentity(
							identities.terminalIds,
							identities.prefix,
							'terminal',
							node.activeChildTerminalId,
						),
			childTerminals: node.childTerminals.map((terminal) => ({
				currentCwd: terminal.launchCwd,
				id: cloneIdentity(identities.terminalIds, identities.prefix, 'terminal', terminal.id),
				launchCwd: terminal.launchCwd,
				phase: 'running',
				title: terminal.title,
			})),
			id: cloneIdentity(identities.panelIds, identities.prefix, 'panel', node.id),
			kind: node.kind,
			primaryTerminal: {
				currentCwd: node.primaryTerminal.launchCwd,
				id: cloneIdentity(
					identities.terminalIds,
					identities.prefix,
					'terminal',
					node.primaryTerminal.id,
				),
				launchCwd: node.primaryTerminal.launchCwd,
				phase: 'running',
				title: node.primaryTerminal.title,
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
