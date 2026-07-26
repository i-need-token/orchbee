import {
	array,
	assert,
	constantFrom,
	double,
	nat,
	property,
	record,
	uniqueArray,
} from 'fast-check';
import { describe, expect, it } from 'vitest';

import {
	collectPanelLeaves,
	createPanelWorkspaceState,
	findPanel,
	getDisplayedTerminal,
	projectPanelDrag,
	transitionPanelWorkspace,
	type PanelLayoutNode,
	type PanelSplit,
	type PanelSplitDirection,
	type PanelWorkspaceEvent,
	type PanelWorkspaceState,
} from '../panel-workspace-state';

const PANEL_SPLIT_DIRECTIONS = ['above', 'below', 'left', 'right'] as const;

function apply(state: PanelWorkspaceState, event: PanelWorkspaceEvent): PanelWorkspaceState {
	return transitionPanelWorkspace(state, event).state;
}

function requireSplit(node: PanelLayoutNode): PanelSplit {
	if (node.kind !== 'split') {
		throw new Error(`Expected split node, received ${node.kind}.`);
	}
	return node;
}

function requireDefined<Value>(value: Value | null | undefined): Value {
	if (value === null || value === undefined) {
		throw new Error('Expected a defined test fixture value.');
	}
	return value;
}

function terminalTitle(state: PanelWorkspaceState): string {
	return `Terminal ${state.nextTerminalSequence}`;
}

function collectUniquePanelSlotIds(node: PanelLayoutNode, slotIds: Set<string>): boolean {
	if (node.kind === 'panel') {
		return true;
	}
	for (const child of node.children) {
		if (slotIds.has(child.id)) {
			return false;
		}
		slotIds.add(child.id);
		if (!collectUniquePanelSlotIds(child.node, slotIds)) {
			return false;
		}
	}
	return true;
}

function hasUniquePanelSlotIds(node: PanelLayoutNode): boolean {
	return collectUniquePanelSlotIds(node, new Set());
}

function hasPanelMembership(node: PanelLayoutNode, expectedPanelIds: ReadonlySet<string>): boolean {
	const panelIds = new Set(collectPanelLeaves(node).map((panel) => panel.id));
	return (
		panelIds.size === expectedPanelIds.size &&
		[...expectedPanelIds].every((panelId) => panelIds.has(panelId))
	);
}

function collectPanelSplits(
	node: PanelLayoutNode,
	splits: PanelSplit[] = [],
): readonly PanelSplit[] {
	if (node.kind === 'panel') {
		return splits;
	}
	splits.push(node);
	for (const child of node.children) {
		collectPanelSplits(child.node, splits);
	}
	return splits;
}

interface EquivalentPanelMove {
	readonly direction: PanelSplitDirection;
	readonly panelId: string;
	readonly targetPanelId: string;
}

function collectEquivalentPanelMoves(
	node: PanelLayoutNode,
	moves: EquivalentPanelMove[] = [],
): readonly EquivalentPanelMove[] {
	if (node.kind === 'panel') {
		return moves;
	}
	for (let index = 0; index < node.children.length - 1; index += 1) {
		const leadingPanel = requireDefined(node.children[index]).node;
		const trailingPanel = requireDefined(node.children[index + 1]).node;
		if (leadingPanel.kind === 'panel' && trailingPanel.kind === 'panel') {
			moves.push(
				node.orientation === 'horizontal'
					? {
							direction: 'left',
							panelId: leadingPanel.id,
							targetPanelId: trailingPanel.id,
						}
					: {
							direction: 'above',
							panelId: leadingPanel.id,
							targetPanelId: trailingPanel.id,
						},
				node.orientation === 'horizontal'
					? {
							direction: 'right',
							panelId: trailingPanel.id,
							targetPanelId: leadingPanel.id,
						}
					: {
							direction: 'below',
							panelId: trailingPanel.id,
							targetPanelId: leadingPanel.id,
						},
			);
		}
	}
	for (const child of node.children) {
		collectEquivalentPanelMoves(child.node, moves);
	}
	return moves;
}

function requireTab(state: PanelWorkspaceState) {
	return requireDefined(state.tabs[0]);
}

function assertValidPanelMove(
	state: PanelWorkspaceState,
	sourcePanelId: string,
	targetPanelId: string,
	direction: PanelSplitDirection,
): void {
	const tab = requireDefined(state.tabs[0]);
	const panelIds = new Set(collectPanelLeaves(tab.root).map((panel) => panel.id));
	const previewRoot = projectPanelDrag(tab.root, sourcePanelId, {
		direction,
		targetPanelId,
	});
	if (previewRoot === null || !hasUniquePanelSlotIds(previewRoot)) {
		throw new Error(`Invalid move preview: ${sourcePanelId} ${direction} ${targetPanelId}`);
	}
	if (!hasPanelMembership(previewRoot, panelIds)) {
		throw new Error(`Move preview changed panel membership: ${sourcePanelId}`);
	}

	const result = transitionPanelWorkspace(state, {
		direction,
		panelId: sourcePanelId,
		tabId: tab.tabId,
		targetPanelId,
		type: 'panel-moved',
	});
	if (result.outcome === 'rejected') {
		throw new Error(`Valid panel move was rejected: ${result.rejection}`);
	}
	const committedRoot = requireDefined(result.state.tabs[0]).root;
	if (!hasUniquePanelSlotIds(committedRoot)) {
		throw new Error(`Invalid committed move: ${sourcePanelId} ${direction} ${targetPanelId}`);
	}
	if (!hasPanelMembership(committedRoot, panelIds)) {
		throw new Error(`Committed move changed panel membership: ${sourcePanelId}`);
	}
}

function enumerateSplitLayouts(panelCount: number): readonly PanelWorkspaceState[] {
	let states = [createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }])];
	for (let currentPanelCount = 1; currentPanelCount < panelCount; currentPanelCount += 1) {
		const nextStates: PanelWorkspaceState[] = [];
		for (const state of states) {
			const tab = requireDefined(state.tabs[0]);
			for (const panel of collectPanelLeaves(tab.root)) {
				for (const direction of PANEL_SPLIT_DIRECTIONS) {
					nextStates.push(
						apply(state, {
							direction,
							newTerminalTitle: terminalTitle(state),
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'panel-split',
						}),
					);
				}
			}
		}
		states = nextStates;
	}
	return states;
}

function enumerateOrderedSubsets(values: readonly string[]): readonly (readonly string[])[] {
	const orders: string[][] = [[]];
	const visit = (prefix: readonly string[], remaining: readonly string[]): void => {
		for (const [index, value] of remaining.entries()) {
			const nextPrefix = [...prefix, value];
			orders.push(nextPrefix);
			visit(
				nextPrefix,
				remaining.filter((_, candidateIndex) => candidateIndex !== index),
			);
		}
	};
	visit([], values);
	return orders;
}

describe('panel workspace state', () => {
	it.each([
		['left', 'horizontal', ['panel-2', 'panel-1']],
		['right', 'horizontal', ['panel-1', 'panel-2']],
		['above', 'vertical', ['panel-2', 'panel-1']],
		['below', 'vertical', ['panel-1', 'panel-2']],
	] as const)('splits a panel locally toward %s', (direction, orientation, expectedPanelIds) => {
		const initial = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		const result = transitionPanelWorkspace(initial, {
			direction,
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const root = requireSplit(requireDefined(result.state.tabs[0]).root);

		expect(result.outcome).toBe('applied');
		expect(root.orientation).toBe(orientation);
		expect(root.children.map((child) => child.weight)).toEqual([50, 50]);
		expect(
			root.children.map((child) => (child.node.kind === 'panel' ? child.node.id : child.node.kind)),
		).toEqual(expectedPanelIds);
	});

	it.each([
		['left', 'panel-2', 'panel-1', 'horizontal', ['panel-2', 'panel-1']],
		['right', 'panel-1', 'panel-2', 'horizontal', ['panel-2', 'panel-1']],
		['above', 'panel-1', 'panel-2', 'vertical', ['panel-1', 'panel-2']],
		['below', 'panel-1', 'panel-2', 'vertical', ['panel-2', 'panel-1']],
	] as const)(
		'moves an existing panel toward %s without replacing its terminal state',
		(direction, panelId, targetPanelId, orientation, expectedPanelIds) => {
			let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
			state = apply(state, {
				direction: 'right',
				newTerminalTitle: 'Terminal 2',
				panelId: 'panel-1',
				tabId: 'tab-a',
				type: 'panel-split',
			});
			state = apply(state, {
				newTerminalTitle: 'Terminal 3',
				panelId,
				tabId: 'tab-a',
				type: 'child-terminal-created',
			});
			const panelBeforeMove = requireDefined(
				findPanel(requireDefined(state.tabs[0]).root, panelId),
			);
			const panelSequence = state.nextPanelSequence;
			const terminalSequence = state.nextTerminalSequence;

			const result = transitionPanelWorkspace(state, {
				direction,
				panelId,
				tabId: 'tab-a',
				targetPanelId,
				type: 'panel-moved',
			});
			const tab = requireDefined(result.state.tabs[0]);
			const root = requireSplit(tab.root);

			expect(result.outcome).toBe('applied');
			expect(root.orientation).toBe(orientation);
			expect(root.children.map((child) => child.node.id)).toEqual(expectedPanelIds);
			expect(findPanel(root, panelId)).toBe(panelBeforeMove);
			expect(collectPanelLeaves(root)).toHaveLength(2);
			expect(tab.focusedPanelId).toBe(panelId);
			expect(result.state.nextPanelSequence).toBe(panelSequence);
			expect(result.state.nextTerminalSequence).toBe(terminalSequence);
		},
	);

	it('treats a move onto the source as a no-op and rejects an unknown target', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});

		const unchangedResult = transitionPanelWorkspace(state, {
			direction: 'left',
			panelId: 'panel-1',
			tabId: 'tab-a',
			targetPanelId: 'panel-1',
			type: 'panel-moved',
		});
		const rejectedResult = transitionPanelWorkspace(state, {
			direction: 'left',
			panelId: 'panel-1',
			tabId: 'tab-a',
			targetPanelId: 'missing-panel',
			type: 'panel-moved',
		});

		expect(unchangedResult).toMatchObject({ outcome: 'unchanged', state });
		expect(rejectedResult).toMatchObject({
			outcome: 'rejected',
			rejection: 'panel-not-found',
			state,
		});
	});

	it.each([
		['left panel before right panel', 'right', 'panel-1', 'panel-2', 'left'],
		['right panel after left panel', 'right', 'panel-2', 'panel-1', 'right'],
		['upper panel above lower panel', 'below', 'panel-1', 'panel-2', 'above'],
		['lower panel below upper panel', 'below', 'panel-2', 'panel-1', 'below'],
	] as const)(
		'preserves layout when moving the %s',
		(_, splitDirection, panelId, targetPanelId, moveDirection) => {
			let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
			state = apply(state, {
				direction: splitDirection,
				newTerminalTitle: 'Terminal 2',
				panelId: 'panel-1',
				tabId: 'tab-a',
				type: 'panel-split',
			});
			const split = requireSplit(requireTab(state).root);
			state = apply(state, {
				leadingWeight: 35,
				separatorIndex: 0,
				splitId: split.id,
				tabId: 'tab-a',
				type: 'adjacent-panels-resized',
			});

			const result = transitionPanelWorkspace(state, {
				direction: moveDirection,
				panelId,
				tabId: 'tab-a',
				targetPanelId,
				type: 'panel-moved',
			});

			expect(result).toMatchObject({ outcome: 'unchanged', state });
			expect(result.state).toBe(state);
		},
	);

	it('projects the reversible drag grid before committing a panel move', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			direction: 'below',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-2',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const root = requireDefined(state.tabs[0]).root;
		const sourcePanel = requireDefined(findPanel(root, 'panel-1'));

		const liftedRoot = requireDefined(projectPanelDrag(root, 'panel-1', null));
		const targetRoot = requireDefined(
			projectPanelDrag(root, 'panel-1', {
				direction: 'left',
				targetPanelId: 'panel-3',
			}),
		);
		const targetSplit = requireSplit(targetRoot);
		const nestedTarget = requireSplit(requireDefined(targetSplit.children[1]).node);

		expect(collectPanelLeaves(root)).toHaveLength(3);
		expect(collectPanelLeaves(liftedRoot).map((panel) => panel.id)).toEqual(['panel-2', 'panel-3']);
		expect(nestedTarget.orientation).toBe('horizontal');
		expect(nestedTarget.children.map((child) => child.node.id)).toEqual(['panel-1', 'panel-3']);
		expect(findPanel(targetRoot, 'panel-1')).toBe(sourcePanel);
	});

	it('keeps panel slot ids unique when dragging a panel out of a collapsing nested split', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		for (const panelId of ['panel-1', 'panel-2']) {
			state = apply(state, {
				direction: 'right',
				newTerminalTitle: terminalTitle(state),
				panelId,
				tabId: 'tab-a',
				type: 'panel-split',
			});
		}
		state = apply(state, {
			direction: 'below',
			newTerminalTitle: terminalTitle(state),
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const root = requireDefined(state.tabs[0]).root;
		const previewRoot = requireSplit(
			requireDefined(
				projectPanelDrag(root, 'panel-1', {
					direction: 'left',
					targetPanelId: 'panel-3',
				}),
			),
		);

		expect(previewRoot.children.map((child) => child.node.id)).toEqual([
			'panel-4',
			'panel-2',
			'panel-1',
			'panel-3',
		]);
		expect(new Set(previewRoot.children.map((child) => child.id)).size).toBe(
			previewRoot.children.length,
		);
	});

	it('keeps slot ids unique when moving a panel out of a non-collapsing nested split', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		for (const [panelId, direction] of [
			['panel-1', 'right'],
			['panel-1', 'below'],
			['panel-3', 'below'],
		] as const) {
			state = apply(state, {
				direction,
				newTerminalTitle: terminalTitle(state),
				panelId,
				tabId: 'tab-a',
				type: 'panel-split',
			});
		}

		expect(() => assertValidPanelMove(state, 'panel-1', 'panel-2', 'left')).not.toThrow();
	});

	it('keeps the untouched sibling at 50 percent when the right panel splits again', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-2',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const root = requireSplit(requireDefined(state.tabs[0]).root);

		expect(root.children.map((child) => child.weight)).toEqual([50, 25, 25]);
	});

	it('keeps the parent split child identity stable when a panel becomes a nested split', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const originalRoot = requireSplit(requireDefined(state.tabs[0]).root);
		const originalChildIds = originalRoot.children.map((child) => child.id);

		state = apply(state, {
			direction: 'below',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const nestedRoot = requireSplit(requireDefined(state.tabs[0]).root);

		expect(nestedRoot.children.map((child) => child.id)).toEqual(originalChildIds);
		expect(nestedRoot.children[0]?.node.kind).toBe('split');
	});

	it('preserves parent split weights when closing a panel collapses a nested split', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const parentSplitId = requireSplit(requireDefined(state.tabs[0]).root).id;
		state = apply(state, {
			leadingWeight: 40,
			separatorIndex: 0,
			splitId: parentSplitId,
			tabId: 'tab-a',
			type: 'adjacent-panels-resized',
		});
		state = apply(state, {
			direction: 'below',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			panelId: 'panel-3',
			tabId: 'tab-a',
			type: 'panel-closed',
		});
		const root = requireSplit(requireDefined(state.tabs[0]).root);

		expect(root.id).toBe(parentSplitId);
		expect(root.children.map((child) => child.weight)).toEqual([40, 60]);
		expect(root.children.map((child) => child.node.kind)).toEqual(['panel', 'panel']);
	});

	it('keeps focus on the current panel when a different panel closes', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		for (const panelId of ['panel-1', 'panel-2']) {
			state = apply(state, {
				direction: 'right',
				newTerminalTitle: terminalTitle(state),
				panelId,
				tabId: 'tab-a',
				type: 'panel-split',
			});
		}
		state = apply(state, {
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-focused',
		});

		const result = transitionPanelWorkspace(state, {
			panelId: 'panel-3',
			tabId: 'tab-a',
			type: 'panel-closed',
		});

		expect(requireTab(result.state).focusedPanelId).toBe('panel-1');
	});

	it('resizes only the pair adjacent to a separator', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		for (const panelId of ['panel-1', 'panel-2']) {
			state = apply(state, {
				direction: 'right',
				newTerminalTitle: terminalTitle(state),
				panelId,
				tabId: 'tab-a',
				type: 'panel-split',
			});
		}
		const root = requireSplit(requireDefined(state.tabs[0]).root);
		state = apply(state, {
			leadingWeight: 30,
			separatorIndex: 0,
			splitId: root.id,
			tabId: 'tab-a',
			type: 'adjacent-panels-resized',
		});

		expect(
			requireSplit(requireDefined(state.tabs[0]).root).children.map((child) => child.weight),
		).toEqual([30, 45, 25]);
	});

	it('nests the other orientation and flattens repeated orientations', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			direction: 'below',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-2',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 4',
			panelId: 'panel-3',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			panelId: 'panel-2',
			tabId: 'tab-a',
			type: 'panel-closed',
		});

		expect(collectPanelLeaves(requireDefined(state.tabs[0]).root).map((panel) => panel.id)).toEqual(
			['panel-1', 'panel-3', 'panel-4'],
		);
	});

	it('creates one-level child terminals, switches presentation, and returns to primary on close', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'child-terminal-created',
		});
		let panel = requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-1'));

		expect(panel.childTerminals).toHaveLength(1);
		expect(getDisplayedTerminal(panel).title).toBe('Terminal 2');

		state = apply(state, {
			childTerminalId: requireDefined(panel.childTerminals[0]).id,
			panelId: panel.id,
			tabId: 'tab-a',
			type: 'child-terminal-closed',
		});
		panel = requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-1'));

		expect(panel.childTerminals).toHaveLength(0);
		expect(getDisplayedTerminal(panel)).toBe(panel.primaryTerminal);
	});

	it('inherits the displayed terminal cwd when creating a panel or child terminal', () => {
		const initial = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		const initialTab = requireDefined(initial.tabs[0]);
		const root = initialTab.root;
		if (root.kind !== 'panel') {
			throw new Error('Expected the initial root to be a panel.');
		}
		const withCwd: PanelWorkspaceState = {
			...initial,
			tabs: [
				{
					...initialTab,
					root: {
						...root,
						primaryTerminal: {
							...root.primaryTerminal,
							cwd: '/workspace',
						},
					},
				},
			],
		};
		let state = apply(withCwd, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-2',
			tabId: 'tab-a',
			type: 'child-terminal-created',
		});

		const panel = requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-2'));
		expect(panel.primaryTerminal.cwd).toBe('/workspace');
		expect(requireDefined(panel.childTerminals[0]).cwd).toBe('/workspace');
	});

	it('retains a panel when its terminal provisioning fails', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			phase: 'failed',
			tabId: 'tab-a',
			terminalId: 'terminal-2',
			type: 'terminal-phase-changed',
		});

		const panels = collectPanelLeaves(requireDefined(state.tabs[0]).root);
		expect(panels).toHaveLength(2);
		expect(
			requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-2')).primaryTerminal,
		).toMatchObject({ id: 'terminal-2', phase: 'failed' });
	});

	it('restores layout before splitting a maximized panel and refuses to close the last panel', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = apply(state, {
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-maximization-toggled',
		});
		state = apply(state, {
			direction: 'below',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});

		expect(requireDefined(state.tabs[0]).maximizedPanelId).toBeNull();

		const singleton = createPanelWorkspaceState([{ id: 'tab-b', title: 'Terminal 1' }]);
		const closeResult = transitionPanelWorkspace(singleton, {
			panelId: 'panel-1',
			tabId: 'tab-b',
			type: 'panel-closed',
		});
		expect(closeResult).toMatchObject({ outcome: 'rejected', rejection: 'last-panel' });
		expect(closeResult.state).toBe(singleton);
	});

	it('keeps tab workspaces isolated while reconciling additions, removals, and order', () => {
		let state = createPanelWorkspaceState([
			{ id: 'tab-a', title: 'Terminal A' },
			{ id: 'tab-b', title: 'Terminal B' },
		]);
		state = apply(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const tabARoot = requireDefined(state.tabs[0]).root;
		state = apply(state, {
			tabs: [
				{ id: 'tab-b', title: 'Terminal B' },
				{ id: 'tab-a', title: 'Terminal A' },
				{ id: 'tab-c', title: 'Terminal C' },
			],
			type: 'tabs-reconciled',
		});

		expect(state.tabs.map((tab) => tab.tabId)).toEqual(['tab-b', 'tab-a', 'tab-c']);
		expect(requireDefined(state.tabs[1]).root).toBe(tabARoot);
		expect(collectPanelLeaves(requireDefined(state.tabs[0]).root)).toHaveLength(1);
		expect(collectPanelLeaves(requireDefined(state.tabs[2]).root)).toHaveLength(1);
	});

	it('exhausts every ordered subset when reconciling a bounded tab collection', () => {
		const initial = createPanelWorkspaceState([
			{ id: 'tab-a', title: 'Terminal A' },
			{ id: 'tab-b', title: 'Terminal B' },
		]);
		const initialTabs = new Map(initial.tabs.map((tab) => [tab.tabId, tab]));
		const orders = enumerateOrderedSubsets(['tab-a', 'tab-b', 'tab-c']);

		for (const order of orders) {
			const result = transitionPanelWorkspace(initial, {
				tabs: order.map((id) => ({ id, title: id })),
				type: 'tabs-reconciled',
			});

			expect(result.state.tabs.map((tab) => tab.tabId)).toEqual(order);
			expect(
				result.state.tabs
					.filter((tab) => initialTabs.has(tab.tabId))
					.every((tab) => tab === initialTabs.get(tab.tabId)),
			).toBe(true);
		}

		expect(orders).toHaveLength(16);
	});

	it('preserves surviving tab identity through generated reconciliation sequences', () => {
		const tabOrderArbitrary = uniqueArray(constantFrom('tab-a', 'tab-b', 'tab-c', 'tab-d'), {
			maxLength: 4,
		});

		assert(
			property(array(tabOrderArbitrary, { maxLength: 40 }), (orders) => {
				let state = createPanelWorkspaceState([
					{ id: 'tab-a', title: 'Terminal A' },
					{ id: 'tab-b', title: 'Terminal B' },
				]);
				for (const order of orders) {
					const previousTabs = new Map(state.tabs.map((tab) => [tab.tabId, tab]));
					const result = transitionPanelWorkspace(state, {
						tabs: order.map((id) => ({ id, title: id })),
						type: 'tabs-reconciled',
					});

					expect(result.state.tabs.map((tab) => tab.tabId)).toEqual(order);
					expect(
						result.state.tabs
							.filter((tab) => previousTabs.has(tab.tabId))
							.every((tab) => tab === previousTabs.get(tab.tabId)),
					).toBe(true);
					state = result.state;
				}
			}),
			{ numRuns: 120, seed: 2_026_07_26 },
		);
	});

	it('preserves all invariants through generated stateful action sequences', () => {
		const actionArbitrary = record({
			direction: constantFrom<PanelSplitDirection>('above', 'below', 'left', 'right'),
			kind: constantFrom(
				'split',
				'move',
				'resize',
				'create-child',
				'activate-child',
				'close-child',
				'close-panel',
				'focus',
				'maximize',
				'phase',
				'show-primary',
			),
			phase: constantFrom('failed', 'idle', 'provisioning', 'running'),
			primaryIndex: nat(),
			secondaryIndex: nat(),
			weight: double({ min: -100, max: 200, noNaN: true }),
		});

		assert(
			property(array(actionArbitrary, { maxLength: 80 }), (actions) => {
				let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
				for (const action of actions) {
					const tab = requireDefined(state.tabs[0]);
					const panels = collectPanelLeaves(tab.root);
					const panel = requireDefined(panels[action.primaryIndex % panels.length]);
					if (action.kind === 'split' && panels.length < 20) {
						state = apply(state, {
							direction: action.direction,
							newTerminalTitle: terminalTitle(state),
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'panel-split',
						});
					} else if (action.kind === 'move' && panels.length > 1) {
						const targetPanel = requireDefined(
							panels[
								(action.primaryIndex + 1 + (action.secondaryIndex % (panels.length - 1))) %
									panels.length
							],
						);
						state = apply(state, {
							direction: action.direction,
							panelId: panel.id,
							tabId: tab.tabId,
							targetPanelId: targetPanel.id,
							type: 'panel-moved',
						});
					} else if (action.kind === 'create-child' && panel.childTerminals.length < 4) {
						state = apply(state, {
							newTerminalTitle: terminalTitle(state),
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'child-terminal-created',
						});
					} else if (action.kind === 'activate-child' && panel.childTerminals.length > 0) {
						const child = requireDefined(
							panel.childTerminals[action.secondaryIndex % panel.childTerminals.length],
						);
						state = apply(state, {
							childTerminalId: child.id,
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'child-terminal-activated',
						});
					} else if (action.kind === 'close-child' && panel.childTerminals.length > 0) {
						const child = requireDefined(
							panel.childTerminals[action.secondaryIndex % panel.childTerminals.length],
						);
						state = apply(state, {
							childTerminalId: child.id,
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'child-terminal-closed',
						});
					} else if (action.kind === 'close-panel' && panels.length > 1) {
						state = apply(state, {
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'panel-closed',
						});
					} else if (action.kind === 'focus') {
						state = apply(state, {
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'panel-focused',
						});
					} else if (action.kind === 'maximize') {
						state = apply(state, {
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'panel-maximization-toggled',
						});
					} else if (action.kind === 'phase') {
						state = apply(state, {
							phase: action.phase,
							tabId: tab.tabId,
							terminalId: getDisplayedTerminal(panel).id,
							type: 'terminal-phase-changed',
						});
					} else if (action.kind === 'show-primary') {
						state = apply(state, {
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'primary-terminal-shown',
						});
					} else if (action.kind === 'resize' && tab.root.kind === 'split') {
						const separatorIndex = action.secondaryIndex % (tab.root.children.length - 1);
						state = apply(state, {
							leadingWeight: action.weight,
							separatorIndex,
							splitId: tab.root.id,
							tabId: tab.tabId,
							type: 'adjacent-panels-resized',
						});
					}
					expect(state.tabs).toHaveLength(1);
				}
			}),
			{ numRuns: 120, seed: 2_026_07_26 },
		);
	});

	it('exhausts every panel move across all layouts with up to three splits', () => {
		const states = enumerateSplitLayouts(4);
		let exploredEquivalentMoveCount = 0;
		let exploredMoveCount = 0;

		for (const state of states) {
			const tab = requireTab(state);
			const panels = collectPanelLeaves(tab.root);
			for (const sourcePanel of panels) {
				for (const targetPanel of panels) {
					if (sourcePanel.id === targetPanel.id) {
						continue;
					}
					for (const direction of PANEL_SPLIT_DIRECTIONS) {
						assertValidPanelMove(state, sourcePanel.id, targetPanel.id, direction);
						exploredMoveCount += 1;
					}
				}
			}
			for (const move of collectEquivalentPanelMoves(tab.root)) {
				expect(projectPanelDrag(tab.root, move.panelId, move)).toBe(tab.root);
				expect(
					transitionPanelWorkspace(state, {
						...move,
						tabId: tab.tabId,
						type: 'panel-moved',
					}),
				).toMatchObject({ outcome: 'unchanged', state });
				exploredEquivalentMoveCount += 1;
			}
		}

		expect(states).toHaveLength(384);
		expect(exploredMoveCount).toBe(18_432);
		expect(exploredEquivalentMoveCount).toBeGreaterThan(0);
	});

	it('exhausts panel, terminal, focus, maximize, close, and resize actions across bounded layouts', () => {
		const states = enumerateSplitLayouts(4);
		const terminalPhases = ['exited', 'failed', 'idle', 'provisioning', 'running'] as const;
		let exploredActionCount = 0;

		for (const state of states) {
			const tab = requireTab(state);
			const panels = collectPanelLeaves(tab.root);
			const originalPanelIds = new Set(panels.map((panel) => panel.id));

			for (const panel of panels) {
				const focusResult = transitionPanelWorkspace(state, {
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'panel-focused',
				});
				expect(requireTab(focusResult.state)).toMatchObject({
					focusedPanelId: panel.id,
					root: tab.root,
				});
				exploredActionCount += 1;

				const maximizeResult = transitionPanelWorkspace(state, {
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'panel-maximization-toggled',
				});
				expect(requireTab(maximizeResult.state)).toMatchObject({
					focusedPanelId: panel.id,
					maximizedPanelId: panel.id,
					root: tab.root,
				});
				const minimizeResult = transitionPanelWorkspace(maximizeResult.state, {
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'panel-maximization-toggled',
				});
				expect(requireTab(minimizeResult.state)).toMatchObject({
					focusedPanelId: panel.id,
					maximizedPanelId: null,
					root: tab.root,
				});
				exploredActionCount += 2;

				const createChildResult = transitionPanelWorkspace(state, {
					newTerminalTitle: terminalTitle(state),
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'child-terminal-created',
				});
				const panelWithChild = requireDefined(
					findPanel(requireTab(createChildResult.state).root, panel.id),
				);
				const child = requireDefined(panelWithChild.childTerminals.at(-1));
				expect(panelWithChild.activeChildTerminalId).toBe(child.id);
				expect(child.cwd).toBe(getDisplayedTerminal(panel).cwd);
				expect(requireTab(createChildResult.state).focusedPanelId).toBe(panel.id);

				const primaryResult = transitionPanelWorkspace(createChildResult.state, {
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'primary-terminal-shown',
				});
				expect(
					requireDefined(findPanel(requireTab(primaryResult.state).root, panel.id))
						.activeChildTerminalId,
				).toBeNull();

				const activateResult = transitionPanelWorkspace(primaryResult.state, {
					childTerminalId: child.id,
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'child-terminal-activated',
				});
				expect(
					requireDefined(findPanel(requireTab(activateResult.state).root, panel.id))
						.activeChildTerminalId,
				).toBe(child.id);

				const closeChildResult = transitionPanelWorkspace(activateResult.state, {
					childTerminalId: child.id,
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'child-terminal-closed',
				});
				const panelAfterChildClose = requireDefined(
					findPanel(requireTab(closeChildResult.state).root, panel.id),
				);
				expect(panelAfterChildClose.childTerminals).toHaveLength(panel.childTerminals.length);
				expect(panelAfterChildClose.activeChildTerminalId).toBeNull();
				exploredActionCount += 4;

				for (const phase of terminalPhases) {
					const phaseResult = transitionPanelWorkspace(state, {
						phase,
						tabId: tab.tabId,
						terminalId: panel.primaryTerminal.id,
						type: 'terminal-phase-changed',
					});
					expect(
						requireDefined(findPanel(requireTab(phaseResult.state).root, panel.id)).primaryTerminal
							.phase,
					).toBe(phase);
					expect(hasPanelMembership(requireTab(phaseResult.state).root, originalPanelIds)).toBe(
						true,
					);
					exploredActionCount += 1;
				}

				const closeResult = transitionPanelWorkspace(state, {
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'panel-closed',
				});
				expect(closeResult.outcome).toBe('applied');
				const remainingPanelIds = new Set(
					[...originalPanelIds].filter((panelId) => panelId !== panel.id),
				);
				expect(hasPanelMembership(requireTab(closeResult.state).root, remainingPanelIds)).toBe(
					true,
				);
				expect(requireTab(closeResult.state).focusedPanelId).toBe(
					tab.focusedPanelId === panel.id
						? (panels[panels.indexOf(panel) + 1] ?? panels[panels.indexOf(panel) - 1])?.id
						: tab.focusedPanelId,
				);
				exploredActionCount += 1;
			}

			for (const split of collectPanelSplits(tab.root)) {
				for (
					let separatorIndex = 0;
					separatorIndex < split.children.length - 1;
					separatorIndex += 1
				) {
					const leadingChild = requireDefined(split.children[separatorIndex]);
					const trailingChild = requireDefined(split.children[separatorIndex + 1]);
					const pairWeight = leadingChild.weight + trailingChild.weight;
					for (const leadingWeight of [-10, 0, pairWeight / 2, pairWeight, pairWeight + 10]) {
						const resizeResult = transitionPanelWorkspace(state, {
							leadingWeight,
							separatorIndex,
							splitId: split.id,
							tabId: tab.tabId,
							type: 'adjacent-panels-resized',
						});
						const resizedSplit = requireDefined(
							collectPanelSplits(requireTab(resizeResult.state).root).find(
								(candidate) => candidate.id === split.id,
							),
						);
						const expectedLeadingWeight = Math.min(Math.max(leadingWeight, 0), pairWeight);
						expect(requireDefined(resizedSplit.children[separatorIndex]).weight).toBeCloseTo(
							expectedLeadingWeight,
						);
						expect(requireDefined(resizedSplit.children[separatorIndex + 1]).weight).toBeCloseTo(
							pairWeight - expectedLeadingWeight,
						);
						expect(
							resizedSplit.children
								.filter((_, index) => index !== separatorIndex && index !== separatorIndex + 1)
								.map((child) => child.weight),
						).toEqual(
							split.children
								.filter((_, index) => index !== separatorIndex && index !== separatorIndex + 1)
								.map((child) => child.weight),
						);
						exploredActionCount += 1;
					}
				}
			}
		}

		expect(states).toHaveLength(384);
		expect(exploredActionCount).toBe(25_728);
	});

	it('keeps generated split layouts valid across every possible panel move', () => {
		const splitActionArbitrary = record({
			direction: constantFrom<PanelSplitDirection>('above', 'below', 'left', 'right'),
			panelIndex: nat({ max: 7 }),
		});

		assert(
			property(array(splitActionArbitrary, { maxLength: 8, minLength: 3 }), (splitActions) => {
				let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
				for (const action of splitActions) {
					const tab = requireDefined(state.tabs[0]);
					const panels = collectPanelLeaves(tab.root);
					const panel = requireDefined(panels[action.panelIndex % panels.length]);
					state = apply(state, {
						direction: action.direction,
						newTerminalTitle: terminalTitle(state),
						panelId: panel.id,
						tabId: tab.tabId,
						type: 'panel-split',
					});
				}

				const tab = requireDefined(state.tabs[0]);
				const panels = collectPanelLeaves(tab.root);
				for (const sourcePanel of panels) {
					for (const targetPanel of panels) {
						if (sourcePanel.id === targetPanel.id) {
							continue;
						}
						for (const direction of PANEL_SPLIT_DIRECTIONS) {
							assertValidPanelMove(state, sourcePanel.id, targetPanel.id, direction);
						}
					}
				}
				for (const move of collectEquivalentPanelMoves(tab.root)) {
					expect(projectPanelDrag(tab.root, move.panelId, move)).toBe(tab.root);
					expect(
						transitionPanelWorkspace(state, {
							...move,
							tabId: tab.tabId,
							type: 'panel-moved',
						}),
					).toMatchObject({ outcome: 'unchanged', state });
				}
			}),
			{ numRuns: 120, seed: 2_026_07_26 },
		);
	});
});
