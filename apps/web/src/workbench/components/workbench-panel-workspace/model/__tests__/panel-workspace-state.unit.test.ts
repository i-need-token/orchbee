import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotSplit,
	snapshotTab,
	type FixtureRecord,
} from '../../../../../__tests__/server-workbench-snapshot';
import {
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
	projectPanelDrag,
	transitionPanelWorkspace,
	validatePanelWorkspaceState,
	type PanelLayoutNode,
	type PanelSplit,
	type PanelSplitDirection,
	type PanelWorkspaceState,
	type TabPanelWorkspace,
} from '../panel-workspace-state';

const PANEL_SPLIT_DIRECTIONS = ['above', 'below', 'left', 'right'] as const;
const TAB_ID = fixtureTabId(1);

/** The Panels a window holds when the Server publishes Workspace 1 with `tabs`. */
function publishedState(tabs: readonly FixtureRecord[]): PanelWorkspaceState {
	return activeTerminalContext(
		createSnapshotRuntime(oneWorkspaceSnapshot(1, tabs)),
	).panelWorkspaceStore.getSnapshot();
}

function oneTabState(root: FixtureRecord): PanelWorkspaceState {
	return publishedState([snapshotTab(1, root)]);
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

function requireTab(state: PanelWorkspaceState, index = 0): TabPanelWorkspace {
	return requireDefined(state.tabs[index]);
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
			const horizontal = node.orientation === 'horizontal';
			moves.push(
				{
					direction: horizontal ? 'left' : 'above',
					panelId: leadingPanel.id,
					targetPanelId: trailingPanel.id,
				},
				{
					direction: horizontal ? 'right' : 'below',
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

function assertValidDragPreview(
	root: PanelLayoutNode,
	sourcePanelId: string,
	targetPanelId: string,
	direction: PanelSplitDirection,
): void {
	const panelIds = new Set(collectPanelLeaves(root).map((panel) => panel.id));
	const previewRoot = projectPanelDrag(root, sourcePanelId, { direction, targetPanelId });
	if (previewRoot === null || !hasUniquePanelSlotIds(previewRoot)) {
		throw new Error(`Invalid move preview: ${sourcePanelId} ${direction} ${targetPanelId}`);
	}
	if (!hasPanelMembership(previewRoot, panelIds)) {
		throw new Error(`Move preview changed panel membership: ${sourcePanelId}`);
	}
}

type LayoutShape =
	| { readonly kind: 'panel'; readonly sequence: number }
	| {
			readonly first: LayoutShape;
			readonly kind: 'split';
			readonly orientation: 'horizontal' | 'vertical';
			readonly second: LayoutShape;
	  };

/** Every binary layout of `panelSequences` in visual order, with both orientations at each split. */
function enumerateLayoutShapes(panelSequences: readonly number[]): readonly LayoutShape[] {
	if (panelSequences.length === 1) {
		return [{ kind: 'panel', sequence: requireDefined(panelSequences[0]) }];
	}
	const shapes: LayoutShape[] = [];
	for (let cut = 1; cut < panelSequences.length; cut += 1) {
		for (const first of enumerateLayoutShapes(panelSequences.slice(0, cut))) {
			for (const second of enumerateLayoutShapes(panelSequences.slice(cut))) {
				for (const orientation of ['horizontal', 'vertical'] as const) {
					shapes.push({ first, kind: 'split', orientation, second });
				}
			}
		}
	}
	return shapes;
}

function layoutFixture(shape: LayoutShape, splitSequence = { next: 1 }): FixtureRecord {
	if (shape.kind === 'panel') return snapshotPanel(shape.sequence);
	const sequence = splitSequence.next;
	splitSequence.next += 1;
	return snapshotSplit(
		sequence,
		shape.orientation,
		layoutFixture(shape.first, splitSequence),
		layoutFixture(shape.second, splitSequence),
	);
}

function enumerateFourPanelLayouts(): readonly PanelWorkspaceState[] {
	return enumerateLayoutShapes([1, 2, 3, 4]).map((shape) => oneTabState(layoutFixture(shape)));
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
	it('projects the reversible drag grid before a panel move is committed', () => {
		const root = requireTab(
			oneTabState(
				snapshotSplit(
					1,
					'horizontal',
					snapshotPanel(1),
					snapshotSplit(2, 'vertical', snapshotPanel(2), snapshotPanel(3)),
				),
			),
		).root;
		const sourcePanel = requireDefined(findPanel(root, fixturePanelId(1)));

		const liftedRoot = requireDefined(projectPanelDrag(root, fixturePanelId(1), null));
		const targetRoot = requireDefined(
			projectPanelDrag(root, fixturePanelId(1), {
				direction: 'left',
				targetPanelId: fixturePanelId(3),
			}),
		);
		const targetSplit = requireSplit(targetRoot);
		const nestedTarget = requireSplit(requireDefined(targetSplit.children[1]).node);

		expect(collectPanelLeaves(root)).toHaveLength(3);
		expect(collectPanelLeaves(liftedRoot).map((panel) => panel.id)).toEqual([
			fixturePanelId(2),
			fixturePanelId(3),
		]);
		expect(nestedTarget.orientation).toBe('horizontal');
		expect(nestedTarget.children.map((child) => child.node.id)).toEqual([
			fixturePanelId(1),
			fixturePanelId(3),
		]);
		expect(findPanel(targetRoot, fixturePanelId(1))).toBe(sourcePanel);
	});

	it('keeps panel slot ids unique when dragging a panel out of a collapsing nested split', () => {
		const root = requireTab(
			oneTabState(
				snapshotSplit(
					1,
					'horizontal',
					snapshotSplit(2, 'vertical', snapshotPanel(1), snapshotPanel(4)),
					snapshotSplit(3, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
				),
			),
		).root;
		const rightSplitId = requireSplit(requireDefined(requireSplit(root).children[1]).node).id;
		const previewRoot = requireSplit(
			requireDefined(
				projectPanelDrag(root, fixturePanelId(1), {
					direction: 'left',
					targetPanelId: fixturePanelId(3),
				}),
			),
		);

		const rightBranch = requireSplit(requireDefined(previewRoot.children[1]).node);
		const targetBranch = requireSplit(requireDefined(rightBranch.children[1]).node);

		expect(previewRoot.children.map((child) => child.node.id)).toEqual([
			fixturePanelId(4),
			rightSplitId,
		]);
		expect(rightBranch.children.map((child) => child.node.id)).toEqual([
			fixturePanelId(2),
			'panel-drag-preview-split',
		]);
		expect(targetBranch.children.map((child) => child.node.id)).toEqual([
			fixturePanelId(1),
			fixturePanelId(3),
		]);
		expect(hasUniquePanelSlotIds(previewRoot)).toBe(true);
	});

	it('keeps slot ids unique when dragging a panel out of a non-collapsing nested split', () => {
		const root = requireTab(
			oneTabState(
				snapshotSplit(
					1,
					'horizontal',
					snapshotSplit(
						2,
						'vertical',
						snapshotPanel(1),
						snapshotSplit(3, 'vertical', snapshotPanel(3), snapshotPanel(4)),
					),
					snapshotPanel(2),
				),
			),
		).root;

		expect(() =>
			assertValidDragPreview(root, fixturePanelId(1), fixturePanelId(2), 'left'),
		).not.toThrow();
	});

	it('exhausts every drag preview across all four-Panel layouts', () => {
		const states = enumerateFourPanelLayouts();
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
						assertValidDragPreview(tab.root, sourcePanel.id, targetPanel.id, direction);
						exploredMoveCount += 1;
					}
				}
			}
			for (const move of collectEquivalentPanelMoves(tab.root)) {
				expect(projectPanelDrag(tab.root, move.panelId, move)).toBe(tab.root);
				exploredEquivalentMoveCount += 1;
			}
		}

		expect(states).toHaveLength(40);
		expect(exploredMoveCount).toBe(1_920);
		expect(exploredEquivalentMoveCount).toBeGreaterThan(0);
	});

	it('exhausts focus, maximize, and metadata changes across four-Panel layouts', () => {
		const terminalCwds = ['/a', '/b', '/c'] as const;
		let exploredActionCount = 0;

		for (const state of enumerateFourPanelLayouts()) {
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

				for (const currentCwd of terminalCwds) {
					const cwdResult = transitionPanelWorkspace(state, {
						currentCwd,
						naturalTitle: null,
						presentationId: panel.primaryPresentation.presentationId,
						tabId: tab.tabId,
						type: 'terminal-metadata-changed',
					});
					expect(
						requireDefined(findPanel(requireTab(cwdResult.state).root, panel.id))
							.primaryPresentation.currentCwd,
					).toBe(currentCwd);
					expect(hasPanelMembership(requireTab(cwdResult.state).root, originalPanelIds)).toBe(true);
					exploredActionCount += 1;
				}
			}
		}

		expect(exploredActionCount).toBe(40 * 4 * 6);
	});

	it('records focus history and switches between child terminals and the primary', () => {
		let state = oneTabState(
			snapshotSplit(
				1,
				'horizontal',
				snapshotPanel(1, snapshotPresentation(1), [snapshotPresentation(4)]),
				snapshotSplit(2, 'vertical', snapshotPanel(2), snapshotPanel(3)),
			),
		);
		for (const sequence of [3, 2]) {
			state = transitionPanelWorkspace(state, {
				panelId: fixturePanelId(sequence),
				tabId: TAB_ID,
				type: 'panel-focused',
			}).state;
		}

		expect(requireTab(state).panelFocusHistory).toEqual([
			fixturePanelId(1),
			fixturePanelId(3),
			fixturePanelId(2),
		]);

		const activated = transitionPanelWorkspace(state, {
			childPresentationId: terminalPresentationId(fixturePresentationId(4)),
			panelId: fixturePanelId(1),
			tabId: TAB_ID,
			type: 'child-terminal-activated',
		}).state;
		const activatedPanel = requireDefined(findPanel(requireTab(activated).root, fixturePanelId(1)));

		expect(getDisplayedTerminal(activatedPanel).presentationId).toBe(fixturePresentationId(4));
		expect(requireTab(activated).focusedPanelId).toBe(fixturePanelId(1));

		const primary = transitionPanelWorkspace(activated, {
			panelId: fixturePanelId(1),
			tabId: TAB_ID,
			type: 'primary-terminal-shown',
		}).state;
		const primaryPanel = requireDefined(findPanel(requireTab(primary).root, fixturePanelId(1)));

		expect(primaryPanel.activeChildPresentationId).toBeNull();
		expect(getDisplayedTerminal(primaryPanel)).toBe(primaryPanel.primaryPresentation);
	});

	it('reports malformed panel focus histories', () => {
		const state = oneTabState(snapshotPanel(1));
		const tab = requireTab(state);
		const panelId = fixturePanelId(1);

		expect(
			validatePanelWorkspaceState({ ...state, tabs: [{ ...tab, panelFocusHistory: [] }] }),
		).toContain(`Empty panel focus history: ${TAB_ID}`);
		expect(
			validatePanelWorkspaceState({
				...state,
				tabs: [{ ...tab, panelFocusHistory: [panelId, panelId] }],
			}),
		).toContain(`Duplicate panel focus history id: ${panelId}`);
		expect(
			validatePanelWorkspaceState({
				...state,
				tabs: [{ ...tab, panelFocusHistory: ['missing-panel', panelId] }],
			}),
		).toContain('Missing panel in focus history: missing-panel');
		expect(
			validatePanelWorkspaceState({
				...state,
				tabs: [{ ...tab, focusedPanelId: 'missing-panel', panelFocusHistory: [panelId] }],
			}),
		).toContain(`Panel focus history does not end with focused panel: ${TAB_ID}`);
	});

	it('keeps tab workspaces isolated while reordering and refuses a Tab it holds no Panels for', () => {
		const state = publishedState([
			snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
			snapshotTab(2, snapshotPanel(3)),
		]);
		const tabARoot = requireTab(state).root;

		const reordered = transitionPanelWorkspace(state, {
			tabIds: [fixtureTabId(2), fixtureTabId(1)],
			type: 'tabs-reconciled',
		}).state;
		const added = transitionPanelWorkspace(state, {
			tabIds: [fixtureTabId(1), fixtureTabId(2), fixtureTabId(3)],
			type: 'tabs-reconciled',
		});

		expect(reordered.tabs.map((tab) => tab.tabId)).toEqual([fixtureTabId(2), fixtureTabId(1)]);
		expect(requireTab(reordered, 1).root).toBe(tabARoot);
		expect(collectPanelLeaves(requireTab(reordered).root)).toHaveLength(1);
		expect(added).toMatchObject({
			outcome: 'rejected',
			rejection: 'terminal-presentation-required',
		});
		expect(added.state).toBe(state);
	});

	it('follows every ordered subset of a bounded tab collection', () => {
		const initial = publishedState([snapshotTab(1), snapshotTab(2), snapshotTab(3)]);
		const tabIds = initial.tabs.map((tab) => tab.tabId);
		const initialTabs = new Map(initial.tabs.map((tab) => [tab.tabId, tab]));
		const orders = enumerateOrderedSubsets(tabIds);

		for (const order of orders) {
			const result = transitionPanelWorkspace(initial, { tabIds: order, type: 'tabs-reconciled' });

			expect(result.state.tabs.map((tab) => tab.tabId)).toEqual(order);
			expect(result.state.tabs.every((tab) => tab === initialTabs.get(tab.tabId))).toBe(true);
		}

		expect(orders).toHaveLength(16);
	});
});
