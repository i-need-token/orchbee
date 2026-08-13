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
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	type TerminalPresentationCreation,
} from '../../../../../terminal-presentation';
import {
	clonePanelWorkspaceState as clonePanelWorkspaceStateCore,
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
	initializePanelWorkspace,
	projectPanelDrag,
	transitionPanelWorkspace as transitionPanelWorkspaceCore,
	type PanelLayoutNode,
	type PanelSplit,
	type PanelSplitDirection,
	type PanelWorkspaceEvent,
	type PanelWorkspaceState,
} from '../panel-workspace-state';

type PreviewPanelWorkspaceEvent = PanelWorkspaceEvent extends infer Event
	? Event extends { readonly type: 'child-terminal-created' | 'panel-split' }
		? Omit<Event, 'presentation'> & { readonly presentation?: TerminalPresentationCreation }
		: Event extends { readonly type: 'child-terminal-closed' | 'panel-closed' }
			? Omit<Event, 'close'> & { readonly close?: { readonly kind: 'preview' } }
			: Event
	: never;

function previewPresentation(sequence: number): TerminalPresentationCreation {
	return {
		kind: 'preview',
		presentationId: terminalPresentationId(`terminal-${sequence}`),
	};
}

function managedPresentation(
	presentationId: string,
	operationId: string,
): Extract<TerminalPresentationCreation, { readonly kind: 'managed' }> {
	return {
		kind: 'managed',
		operationId: terminalPresentationOperationId(operationId),
		presentationId: terminalPresentationId(presentationId),
	};
}

function createPanelWorkspaceState(
	tabs: readonly { readonly cwd?: string | null; readonly id: string; readonly title: string }[],
): PanelWorkspaceState {
	return initializePanelWorkspace(
		tabs.map((tab, index) => ({ ...tab, presentation: previewPresentation(index + 1) })),
	).state;
}

function clonePanelWorkspaceState(
	state: PanelWorkspaceState,
	tabIdMap: ReadonlyMap<string, string>,
	identityPrefix: string,
): PanelWorkspaceState {
	let sequence = 1;
	return clonePanelWorkspaceStateCore(state, tabIdMap, identityPrefix, () =>
		terminalPresentationId(`${identityPrefix}-terminal-${sequence++}`),
	);
}

function transitionPanelWorkspace(state: PanelWorkspaceState, event: PreviewPanelWorkspaceEvent) {
	if (event.type === 'child-terminal-created' || event.type === 'panel-split') {
		return transitionPanelWorkspaceCore(state, {
			...event,
			presentation: event.presentation ?? previewPresentation(state.nextTerminalSequence),
		});
	}
	if (event.type === 'child-terminal-closed' || event.type === 'panel-closed') {
		return transitionPanelWorkspaceCore(state, {
			...event,
			close: event.close ?? { kind: 'preview' },
		});
	}
	if (event.type === 'tabs-reconciled') {
		const existingTabIds = new Set(state.tabs.map((tab) => tab.tabId));
		let sequence = state.nextTerminalSequence;
		return transitionPanelWorkspaceCore(state, {
			tabs: event.tabs.map((tab) =>
				existingTabIds.has(tab.id) || tab.presentation !== undefined
					? tab
					: { ...tab, presentation: previewPresentation(sequence++) },
			),
			type: event.type,
		});
	}
	return transitionPanelWorkspaceCore(state, event);
}

const PANEL_SPLIT_DIRECTIONS = ['above', 'below', 'left', 'right'] as const;

function apply(state: PanelWorkspaceState, event: PreviewPanelWorkspaceEvent): PanelWorkspaceState {
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
	it('clones the complete layout into fresh scoped identities without sharing layout objects', () => {
		let state = createPanelWorkspaceState([
			{ cwd: '/workspace', id: 'tab-a', title: 'Terminal 1' },
		]);
		state = transitionPanelWorkspace(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		}).state;
		state = apply(state, {
			currentCwd: '/changed',
			presentationId: terminalPresentationId('terminal-1'),
			tabId: 'tab-a',
			type: 'terminal-current-cwd-changed',
		});

		const clone = clonePanelWorkspaceState(state, new Map([['tab-a', 'tab-copy']]), 'copy-context');

		expect(clone.tabs[0]?.tabId).toBe('tab-copy');
		expect(clone.tabs[0]?.root).not.toBe(state.tabs[0]?.root);
		expect(clone.tabs[0]).toMatchObject({
			focusedPanelId: 'copy-context-panel-1',
			root: {
				children: [
					{
						id: 'copy-context-panel-slot-1',
						node: {
							id: 'copy-context-panel-2',
							primaryPresentation: {
								currentCwd: '/workspace',
								launchCwd: '/workspace',
								lifecycle: { kind: 'preview' },
								presentationId: 'copy-context-terminal-1',
							},
						},
					},
					{
						id: 'copy-context-panel-slot-2',
						node: {
							id: 'copy-context-panel-1',
							primaryPresentation: {
								presentationId: 'copy-context-terminal-2',
							},
						},
					},
				],
				id: 'copy-context-split-1',
				kind: 'split',
			},
		});
	});

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

		expect(panel.childPresentations).toHaveLength(1);
		expect(getDisplayedTerminal(panel).title).toBe('Terminal 2');

		state = apply(state, {
			childPresentationId: requireDefined(panel.childPresentations[0]).presentationId,
			panelId: panel.id,
			tabId: 'tab-a',
			type: 'child-terminal-closed',
		});
		panel = requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-1'));

		expect(panel.childPresentations).toHaveLength(0);
		expect(getDisplayedTerminal(panel)).toBe(panel.primaryPresentation);
	});

	it('inherits the displayed terminal cwd when creating a panel or child terminal', () => {
		let state = createPanelWorkspaceState([{ cwd: '/launch', id: 'tab-a', title: 'Terminal 1' }]);
		state = apply(state, {
			currentCwd: '/workspace',
			presentationId: terminalPresentationId('terminal-1'),
			tabId: 'tab-a',
			type: 'terminal-current-cwd-changed',
		});
		state = apply(state, {
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
		expect(panel.primaryPresentation.currentCwd).toBe('/workspace');
		expect(panel.primaryPresentation.launchCwd).toBe('/workspace');
		expect(requireDefined(panel.childPresentations[0])).toMatchObject({
			currentCwd: '/workspace',
			launchCwd: '/workspace',
		});
		const originalPanel = requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-1'));
		expect(originalPanel.primaryPresentation).toMatchObject({
			currentCwd: '/workspace',
			launchCwd: '/launch',
		});
	});

	it('retains a failed provision and requires a fresh operation ID to retry it', () => {
		let state = createPanelWorkspaceState([{ id: 'tab-a', title: 'Terminal 1' }]);
		const split = transitionPanelWorkspace(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			presentation: {
				kind: 'managed',
				operationId: terminalPresentationOperationId('provision-2'),
				presentationId: terminalPresentationId('terminal-2'),
			},
			tabId: 'tab-a',
			type: 'panel-split',
		});
		state = transitionPanelWorkspace(split.state, {
			failure: { code: 'spawn-failed', retryable: true },
			operationId: terminalPresentationOperationId('provision-2'),
			presentationId: terminalPresentationId('terminal-2'),
			tabId: 'tab-a',
			type: 'terminal-provision-failed',
		}).state;

		const panels = collectPanelLeaves(requireDefined(state.tabs[0]).root);
		expect(panels).toHaveLength(2);
		expect(
			requireDefined(findPanel(requireDefined(state.tabs[0]).root, 'panel-2')).primaryPresentation,
		).toMatchObject({
			lifecycle: {
				failure: { code: 'spawn-failed', retryable: true },
				kind: 'provision-failed',
			},
			presentationId: 'terminal-2',
		});
		const reusedOperation = transitionPanelWorkspaceCore(state, {
			operationId: terminalPresentationOperationId('provision-2'),
			presentationId: terminalPresentationId('terminal-2'),
			tabId: 'tab-a',
			type: 'terminal-provision-retried',
		});
		expect(reusedOperation).toMatchObject({
			outcome: 'rejected',
			rejection: 'stale-operation',
		});
		expect(reusedOperation.state).toBe(state);
		const retry = transitionPanelWorkspaceCore(state, {
			operationId: terminalPresentationOperationId('provision-2-retry'),
			presentationId: terminalPresentationId('terminal-2'),
			tabId: 'tab-a',
			type: 'terminal-provision-retried',
		});
		expect(retry.commands).toEqual([
			{
				launchCwd: null,
				operationId: 'provision-2-retry',
				presentationId: 'terminal-2',
				type: 'terminal-provision-requested',
			},
		]);
	});

	it('accepts an authoritative runtime failure before provisioning completes', () => {
		const initialization = initializePanelWorkspace([
			{
				id: 'tab-a',
				presentation: managedPresentation('presentation-1', 'provision-1'),
				title: 'Terminal 1',
			},
		]);
		const failure = { code: 'terminal-runtime-pty-read-read-output', retryable: false };
		const failed = transitionPanelWorkspaceCore(initialization.state, {
			failure,
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-failed',
		});

		expect(failed.outcome).toBe('applied');
		expect(
			findPanel(requireDefined(failed.state.tabs[0]).root, 'panel-1')?.primaryPresentation
				.lifecycle,
		).toEqual({ failure, kind: 'failed' });
		expect(
			transitionPanelWorkspaceCore(failed.state, {
				operationId: terminalPresentationOperationId('provision-1'),
				presentationId: terminalPresentationId('presentation-1'),
				tabId: 'tab-a',
				type: 'terminal-provision-completed',
			}),
		).toMatchObject({ outcome: 'rejected', rejection: 'stale-operation' });
	});

	it('clears transient terminal metadata when a stopped session starts fresh', () => {
		const initialization = initializePanelWorkspace([
			{
				cwd: '/launch',
				id: 'tab-a',
				presentation: managedPresentation('presentation-1', 'provision-1'),
				title: 'Terminal 1',
			},
		]);
		let state = transitionPanelWorkspaceCore(initialization.state, {
			operationId: terminalPresentationOperationId('provision-1'),
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-provision-completed',
		}).state;
		state = transitionPanelWorkspaceCore(state, {
			currentCwd: '/observed',
			naturalTitle: 'old process',
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-metadata-changed',
		}).state;
		state = transitionPanelWorkspaceCore(state, {
			operations: [
				{
					operationId: terminalPresentationOperationId('stop-1'),
					presentationId: terminalPresentationId('presentation-1'),
				},
			],
			type: 'terminal-sessions-stop-requested',
		}).state;
		state = transitionPanelWorkspaceCore(state, {
			operationId: terminalPresentationOperationId('stop-1'),
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-stop-completed',
		}).state;

		const restarted = transitionPanelWorkspaceCore(state, {
			presentation: managedPresentation('presentation-2', 'provision-2'),
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-stopped-session-started',
		});

		expect(
			getDisplayedTerminal(
				requireDefined(findPanel(requireDefined(restarted.state.tabs[0]).root, 'panel-1')),
			),
		).toMatchObject({
			currentCwd: '/launch',
			naturalTitle: null,
			presentationId: 'presentation-2',
			title: 'Terminal 1',
		});
	});

	it('keeps a partially closed Panel until failed terminals retry and complete', () => {
		const initialization = initializePanelWorkspace([
			{
				id: 'tab-a',
				presentation: managedPresentation('presentation-1', 'provision-1'),
				title: 'Terminal 1',
			},
		]);
		expect(initialization.commands).toEqual([
			{
				launchCwd: null,
				operationId: 'provision-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-requested',
			},
		]);
		let state = transitionPanelWorkspaceCore(initialization.state, {
			operationId: terminalPresentationOperationId('provision-1'),
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-provision-completed',
		}).state;
		state = transitionPanelWorkspaceCore(state, {
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			presentation: managedPresentation('presentation-2', 'provision-2'),
			tabId: 'tab-a',
			type: 'panel-split',
		}).state;
		state = transitionPanelWorkspaceCore(state, {
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-2',
			presentation: managedPresentation('presentation-3', 'provision-3'),
			tabId: 'tab-a',
			type: 'child-terminal-created',
		}).state;
		for (const [presentationId, operationId] of [
			['presentation-2', 'provision-2'],
			['presentation-3', 'provision-3'],
		] as const) {
			state = transitionPanelWorkspaceCore(state, {
				operationId: terminalPresentationOperationId(operationId),
				presentationId: terminalPresentationId(presentationId),
				tabId: 'tab-a',
				type: 'terminal-provision-completed',
			}).state;
		}

		const close = transitionPanelWorkspaceCore(state, {
			close: {
				kind: 'managed',
				operations: [
					{
						operationId: terminalPresentationOperationId('close-2'),
						presentationId: terminalPresentationId('presentation-2'),
					},
					{
						operationId: terminalPresentationOperationId('close-3'),
						presentationId: terminalPresentationId('presentation-3'),
					},
				],
				removalId: terminalPresentationRemovalId('remove-panel-2'),
			},
			panelId: 'panel-2',
			tabId: 'tab-a',
			type: 'panel-closed',
		});
		expect(close.commands).toEqual([
			{
				operationId: 'close-2',
				presentationId: 'presentation-2',
				type: 'terminal-close-requested',
			},
			{
				operationId: 'close-3',
				presentationId: 'presentation-3',
				type: 'terminal-close-requested',
			},
		]);
		expect(collectPanelLeaves(requireTab(close.state).root)).toHaveLength(2);

		const firstCompleted = transitionPanelWorkspaceCore(close.state, {
			operationId: terminalPresentationOperationId('close-2'),
			presentationId: terminalPresentationId('presentation-2'),
			removalId: terminalPresentationRemovalId('remove-panel-2'),
			tabId: 'tab-a',
			type: 'terminal-close-completed',
		});
		const partialFailure = transitionPanelWorkspaceCore(firstCompleted.state, {
			failure: { code: 'host-busy', retryable: true },
			operationId: terminalPresentationOperationId('close-3'),
			presentationId: terminalPresentationId('presentation-3'),
			removalId: terminalPresentationRemovalId('remove-panel-2'),
			tabId: 'tab-a',
			type: 'terminal-close-failed',
		});
		expect(collectPanelLeaves(requireTab(partialFailure.state).root)).toHaveLength(2);
		expect(
			findPanel(requireTab(partialFailure.state).root, 'panel-2')?.primaryPresentation.lifecycle,
		).toMatchObject({ kind: 'closed' });
		expect(
			findPanel(requireTab(partialFailure.state).root, 'panel-2')?.childPresentations[0]?.lifecycle,
		).toMatchObject({ failure: { code: 'host-busy' }, kind: 'close-failed' });

		const stale = transitionPanelWorkspaceCore(partialFailure.state, {
			operationId: terminalPresentationOperationId('close-3'),
			presentationId: terminalPresentationId('presentation-3'),
			removalId: terminalPresentationRemovalId('remove-panel-2'),
			tabId: 'tab-a',
			type: 'terminal-close-completed',
		});
		expect(stale).toMatchObject({ outcome: 'rejected', rejection: 'stale-operation' });
		expect(stale.state).toBe(partialFailure.state);
		const reusedOperation = transitionPanelWorkspaceCore(partialFailure.state, {
			operationId: terminalPresentationOperationId('close-3'),
			presentationId: terminalPresentationId('presentation-3'),
			removalId: terminalPresentationRemovalId('remove-panel-2'),
			tabId: 'tab-a',
			type: 'terminal-close-retried',
		});
		expect(reusedOperation).toMatchObject({
			outcome: 'rejected',
			rejection: 'stale-operation',
		});
		expect(reusedOperation.state).toBe(partialFailure.state);

		const retry = transitionPanelWorkspaceCore(partialFailure.state, {
			operationId: terminalPresentationOperationId('close-3-retry'),
			presentationId: terminalPresentationId('presentation-3'),
			removalId: terminalPresentationRemovalId('remove-panel-2'),
			tabId: 'tab-a',
			type: 'terminal-close-retried',
		});
		expect(retry.commands).toEqual([
			{
				operationId: 'close-3-retry',
				presentationId: 'presentation-3',
				type: 'terminal-close-requested',
			},
		]);
		const completed = transitionPanelWorkspaceCore(retry.state, {
			operationId: terminalPresentationOperationId('close-3-retry'),
			presentationId: terminalPresentationId('presentation-3'),
			removalId: terminalPresentationRemovalId('remove-panel-2'),
			tabId: 'tab-a',
			type: 'terminal-close-completed',
		});
		expect(collectPanelLeaves(requireTab(completed.state).root).map((panel) => panel.id)).toEqual([
			'panel-1',
		]);
		expect(completed.state.pendingRemovals).toEqual([]);
	});

	it('ignores a late provision result after close has committed and emits Tab readiness last', () => {
		const initialization = initializePanelWorkspace([
			{
				id: 'tab-a',
				presentation: managedPresentation('presentation-1', 'provision-1'),
				title: 'Terminal 1',
			},
		]);
		const close = transitionPanelWorkspaceCore(initialization.state, {
			close: {
				kind: 'managed',
				operations: [
					{
						operationId: terminalPresentationOperationId('close-1'),
						presentationId: terminalPresentationId('presentation-1'),
					},
				],
				removalId: terminalPresentationRemovalId('remove-tab-a'),
			},
			tabId: 'tab-a',
			type: 'tab-close-requested',
		});
		const lateProvision = transitionPanelWorkspaceCore(close.state, {
			operationId: terminalPresentationOperationId('provision-1'),
			presentationId: terminalPresentationId('presentation-1'),
			tabId: 'tab-a',
			type: 'terminal-provision-completed',
		});
		expect(lateProvision).toMatchObject({ outcome: 'rejected', rejection: 'stale-operation' });
		expect(lateProvision.state).toBe(close.state);
		const prematureReconciliation = transitionPanelWorkspaceCore(close.state, {
			tabs: [],
			type: 'tabs-reconciled',
		});
		expect(prematureReconciliation).toMatchObject({
			outcome: 'rejected',
			rejection: 'terminal-removal-required',
		});
		expect(prematureReconciliation.state).toBe(close.state);

		const completed = transitionPanelWorkspaceCore(close.state, {
			operationId: terminalPresentationOperationId('close-1'),
			presentationId: terminalPresentationId('presentation-1'),
			removalId: terminalPresentationRemovalId('remove-tab-a'),
			tabId: 'tab-a',
			type: 'terminal-close-completed',
		});
		expect(completed.commands).toEqual([
			{ removalId: 'remove-tab-a', tabId: 'tab-a', type: 'tab-close-ready' },
		]);
		expect(completed.state.tabs).toHaveLength(1);
		expect(completed.state.pendingRemovals).toHaveLength(1);

		const reconciled = transitionPanelWorkspaceCore(completed.state, {
			tabs: [],
			type: 'tabs-reconciled',
		});
		expect(reconciled.state).toMatchObject({ pendingRemovals: [], tabs: [] });
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
				'cwd',
				'show-primary',
			),
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
					} else if (action.kind === 'create-child' && panel.childPresentations.length < 4) {
						state = apply(state, {
							newTerminalTitle: terminalTitle(state),
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'child-terminal-created',
						});
					} else if (action.kind === 'activate-child' && panel.childPresentations.length > 0) {
						const child = requireDefined(
							panel.childPresentations[action.secondaryIndex % panel.childPresentations.length],
						);
						state = apply(state, {
							childPresentationId: child.presentationId,
							panelId: panel.id,
							tabId: tab.tabId,
							type: 'child-terminal-activated',
						});
					} else if (action.kind === 'close-child' && panel.childPresentations.length > 0) {
						const child = requireDefined(
							panel.childPresentations[action.secondaryIndex % panel.childPresentations.length],
						);
						state = apply(state, {
							childPresentationId: child.presentationId,
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
					} else if (action.kind === 'cwd') {
						state = apply(state, {
							currentCwd: `/generated/${action.secondaryIndex % 7}`,
							presentationId: getDisplayedTerminal(panel).presentationId,
							tabId: tab.tabId,
							type: 'terminal-current-cwd-changed',
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

	it('exhausts panel, presentation, focus, maximize, close, and resize actions across bounded layouts', () => {
		const states = enumerateSplitLayouts(4);
		const terminalCwds = ['/a', '/b', '/c', '/d', '/e'] as const;
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
				const child = requireDefined(panelWithChild.childPresentations.at(-1));
				expect(panelWithChild.activeChildPresentationId).toBe(child.presentationId);
				expect(child.currentCwd).toBe(getDisplayedTerminal(panel).currentCwd);
				expect(requireTab(createChildResult.state).focusedPanelId).toBe(panel.id);

				const primaryResult = transitionPanelWorkspace(createChildResult.state, {
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'primary-terminal-shown',
				});
				expect(
					requireDefined(findPanel(requireTab(primaryResult.state).root, panel.id))
						.activeChildPresentationId,
				).toBeNull();

				const activateResult = transitionPanelWorkspace(primaryResult.state, {
					childPresentationId: child.presentationId,
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'child-terminal-activated',
				});
				expect(
					requireDefined(findPanel(requireTab(activateResult.state).root, panel.id))
						.activeChildPresentationId,
				).toBe(child.presentationId);

				const closeChildResult = transitionPanelWorkspace(activateResult.state, {
					childPresentationId: child.presentationId,
					panelId: panel.id,
					tabId: tab.tabId,
					type: 'child-terminal-closed',
				});
				const panelAfterChildClose = requireDefined(
					findPanel(requireTab(closeChildResult.state).root, panel.id),
				);
				expect(panelAfterChildClose.childPresentations).toHaveLength(
					panel.childPresentations.length,
				);
				expect(panelAfterChildClose.activeChildPresentationId).toBeNull();
				exploredActionCount += 4;

				for (const currentCwd of terminalCwds) {
					const cwdResult = transitionPanelWorkspace(state, {
						currentCwd,
						presentationId: panel.primaryPresentation.presentationId,
						tabId: tab.tabId,
						type: 'terminal-current-cwd-changed',
					});
					expect(
						requireDefined(findPanel(requireTab(cwdResult.state).root, panel.id))
							.primaryPresentation.currentCwd,
					).toBe(currentCwd);
					expect(hasPanelMembership(requireTab(cwdResult.state).root, originalPanelIds)).toBe(true);
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
