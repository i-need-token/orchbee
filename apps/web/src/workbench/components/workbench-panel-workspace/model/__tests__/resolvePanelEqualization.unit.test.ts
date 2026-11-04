import { array, assert, constantFrom, double, nat, property, record } from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelLeaf, PanelSplit } from '../panel-workspace-state';
import type { PanelSplitOrientation } from '../panel-layout-types';
import { resolvePanelEqualization } from '../resolvePanelEqualization';

describe('resolvePanelEqualization', () => {
	it('equalizes same-axis Panel extents proportionally', () => {
		const root = split(
			'split-root',
			'horizontal',
			panel('panel-1'),
			split('split-inner', 'horizontal', panel('panel-2'), panel('panel-3'), 80),
			80,
		);

		const result = resolvePanelEqualization(root);

		expect(result.kind).toBe('applied');
		const equalizedRoot = requireSplit(result.root);
		const equalizedInner = requireSplit(equalizedRoot.children[1]?.node);
		expect(equalizedRoot.children[0]?.weight).toBeCloseTo(100 / 3, 12);
		expect(equalizedRoot.children[1]?.weight).toBeCloseTo(200 / 3, 12);
		expect(equalizedInner.children.map((child) => child.weight)).toEqual([50, 50]);
	});

	it('counts a perpendicular subtree as one parent-axis unit while equalizing it recursively', () => {
		const root = split(
			'split-root',
			'horizontal',
			panel('panel-1'),
			split(
				'split-vertical-1',
				'vertical',
				panel('panel-2'),
				split(
					'split-vertical-2',
					'vertical',
					panel('panel-3'),
					split('split-vertical-3', 'vertical', panel('panel-4'), panel('panel-5'), 90),
					90,
				),
				90,
			),
			90,
		);

		const result = resolvePanelEqualization(root);

		const equalizedRoot = requireSplit(result.root);
		const vertical1 = requireSplit(equalizedRoot.children[1]?.node);
		const vertical2 = requireSplit(vertical1.children[1]?.node);
		const vertical3 = requireSplit(vertical2.children[1]?.node);
		expect(equalizedRoot.children.map((child) => child.weight)).toEqual([50, 50]);
		expect(vertical1.children[0]?.weight).toBeCloseTo(25, 12);
		expect(vertical2.children[0]?.weight).toBeCloseTo(100 / 3, 12);
		expect(vertical3.children.map((child) => child.weight)).toEqual([50, 50]);
	});

	it('preserves complete topology and entity identities', () => {
		const root = generatedLayout([
			{ leadingWeight: 20, orientation: 'horizontal', targetIndex: 0 },
			{ leadingWeight: 70, orientation: 'vertical', targetIndex: 1 },
			{ leadingWeight: 10, orientation: 'horizontal', targetIndex: 2 },
		]);
		const topologyBefore = projectTopology(root);
		const panelsBefore = collectPanels(root);

		const result = resolvePanelEqualization(root);

		expect(projectTopology(result.root)).toEqual(topologyBefore);
		const panelsAfter = collectPanels(result.root);
		expect(panelsAfter).toHaveLength(panelsBefore.length);
		for (const [index, before] of panelsBefore.entries()) {
			expect(panelsAfter[index]).toBe(before);
		}
	});

	it('keeps singleton and already-equal layouts as structural no-ops', () => {
		const singleton = panel('panel-1');
		const equal = split('split-root', 'horizontal', panel('panel-1'), panel('panel-2'), 50);

		expect(resolvePanelEqualization(singleton)).toEqual({
			kind: 'unchanged',
			reason: 'already-equal',
			root: singleton,
		});
		expect(resolvePanelEqualization(equal)).toEqual({
			kind: 'unchanged',
			reason: 'already-equal',
			root: equal,
		});
	});

	it('preserves proportional weights and idempotency across generated valid trees', () => {
		const instructionArbitrary = record({
			leadingWeight: double({ max: 99, min: 1, noNaN: true }),
			orientation: constantFrom<PanelSplitOrientation>('horizontal', 'vertical'),
			targetIndex: nat({ max: 64 }),
		});

		assert(
			property(array(instructionArbitrary, { maxLength: 8 }), (instructions) => {
				const root = generatedLayout(instructions);
				const topologyBefore = projectTopology(root);
				const panelsBefore = collectPanels(root);

				const first = resolvePanelEqualization(root);
				assertEqualized(first.root);
				expect(projectTopology(first.root)).toEqual(topologyBefore);
				for (const [index, before] of panelsBefore.entries()) {
					expect(collectPanels(first.root)[index]).toBe(before);
				}

				const second = resolvePanelEqualization(first.root);
				expect(second).toEqual({
					kind: 'unchanged',
					reason: 'already-equal',
					root: first.root,
				});
			}),
			{ numRuns: 240, seed: 2_026_08_27 },
		);
	});
});

interface SplitInstruction {
	readonly leadingWeight: number;
	readonly orientation: PanelSplitOrientation;
	readonly targetIndex: number;
}

type TopologyProjection =
	| { readonly id: string; readonly kind: 'panel' }
	| {
			readonly children: readonly {
				readonly id: string;
				readonly node: TopologyProjection;
			}[];
			readonly id: string;
			readonly kind: 'split';
			readonly orientation: PanelSplitOrientation;
	  };

function panel(id: string): PanelLeaf {
	return {
		activeChildPresentationId: null,
		childPresentations: [],
		id,
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'unknown',
			currentCwd: null,
			launchCwd: null,
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: `terminal-${id}` as never,
			title: id,
		},
	};
}

function split(
	id: string,
	orientation: PanelSplitOrientation,
	leading: PanelLayoutNode,
	trailing: PanelLayoutNode,
	leadingWeight: number,
): PanelSplit {
	return {
		children: [
			{ id: `${id}-leading-slot`, node: leading, weight: leadingWeight },
			{ id: `${id}-trailing-slot`, node: trailing, weight: 100 - leadingWeight },
		],
		id,
		kind: 'split',
		orientation,
	};
}

function generatedLayout(instructions: readonly SplitInstruction[]): PanelLayoutNode {
	let root: PanelLayoutNode = panel('panel-1');
	for (const [index, instruction] of instructions.entries()) {
		const panels = collectPanels(root);
		const target = panels[instruction.targetIndex % panels.length];
		if (target === undefined) throw new Error('Expected a generated target Panel.');
		const splitId = `split-${index + 1}`;
		root = replacePanel(
			root,
			target.id,
			split(
				splitId,
				instruction.orientation,
				target,
				panel(`panel-${index + 2}`),
				instruction.leadingWeight,
			),
		);
	}
	return root;
}

function replacePanel(
	node: PanelLayoutNode,
	panelId: string,
	replacement: PanelSplit,
): PanelLayoutNode {
	if (node.kind === 'panel') return node.id === panelId ? replacement : node;
	return {
		...node,
		children: node.children.map((child) => ({
			...child,
			node: replacePanel(child.node, panelId, replacement),
		})),
	};
}

function collectPanels(node: PanelLayoutNode): readonly PanelLeaf[] {
	return node.kind === 'panel'
		? [node]
		: node.children.flatMap((child) => collectPanels(child.node));
}

function projectTopology(node: PanelLayoutNode): TopologyProjection {
	return node.kind === 'panel'
		? { id: node.id, kind: 'panel' }
		: {
				children: node.children.map((child) => ({
					id: child.id,
					node: projectTopology(child.node),
				})),
				id: node.id,
				kind: 'split',
				orientation: node.orientation,
			};
}

function assertEqualized(node: PanelLayoutNode): void {
	if (node.kind === 'panel') return;
	const units = node.children.map((child) => axisUnits(child.node, node.orientation));
	const totalUnits = units.reduce((total, value) => total + value, 0);
	for (const [index, child] of node.children.entries()) {
		expect(child.weight).toBeCloseTo((100 * (units[index] ?? 0)) / totalUnits, 12);
		expect(Number.isFinite(child.weight)).toBe(true);
		expect(child.weight).toBeGreaterThanOrEqual(0);
		assertEqualized(child.node);
	}
	expect(node.children.reduce((total, child) => total + child.weight, 0)).toBeCloseTo(100, 12);
}

function axisUnits(node: PanelLayoutNode, orientation: PanelSplitOrientation): number {
	return node.kind === 'split' && node.orientation === orientation
		? node.children.reduce((total, child) => total + axisUnits(child.node, orientation), 0)
		: 1;
}

function requireSplit(node: PanelLayoutNode | undefined): PanelSplit {
	if (node?.kind !== 'split') throw new Error('Expected a Split node.');
	return node;
}
