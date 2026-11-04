import { array, assert, constantFrom, nat, property, record } from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelLeaf, PanelSplit } from '../panel-workspace-state';
import type { PanelSplitOrientation } from '../panel-layout-types';
import { resolveCyclicPanelFocus, type PanelFocusCycleDirection } from '../resolveCyclicPanelFocus';

describe('resolveCyclicPanelFocus', () => {
	it('follows canonical nested leaf order and wraps in both directions', () => {
		const root = split(
			'split-root',
			'horizontal',
			panel('panel-1'),
			split('split-inner', 'vertical', panel('panel-2'), panel('panel-3')),
		);

		expect(resolve(root, 'panel-1', 'next')).toBe('panel-2');
		expect(resolve(root, 'panel-2', 'next')).toBe('panel-3');
		expect(resolve(root, 'panel-3', 'next')).toBe('panel-1');
		expect(resolve(root, 'panel-1', 'previous')).toBe('panel-3');
		expect(resolve(root, 'panel-3', 'previous')).toBe('panel-2');
		expect(resolve(root, 'panel-2', 'previous')).toBe('panel-1');
	});

	it('returns null for a singleton, a stale source, or a maximized layout', () => {
		const singleton = panel('panel-1');
		const pair = split('split-root', 'horizontal', singleton, panel('panel-2'));

		expect(resolve(singleton, 'panel-1', 'next')).toBeNull();
		expect(resolve(pair, 'missing-panel', 'next')).toBeNull();
		expect(
			resolveCyclicPanelFocus({
				direction: 'next',
				maximizedPanelId: 'panel-1',
				root: pair,
				sourcePanelId: 'panel-1',
			}),
		).toBeNull();
	});

	it('keeps results live and makes directions inverse across generated valid trees', () => {
		const instructionArbitrary = record({
			orientation: constantFrom<PanelSplitOrientation>('horizontal', 'vertical'),
			targetIndex: nat({ max: 64 }),
		});

		assert(
			property(array(instructionArbitrary, { maxLength: 8 }), (instructions) => {
				const root = generatedLayout(instructions);
				const panels = collectPanels(root);
				for (const source of panels) {
					const next = resolve(root, source.id, 'next');
					const previous = resolve(root, source.id, 'previous');
					if (panels.length === 1) {
						if (next !== null || previous !== null) {
							throw new Error('Expected singleton cyclic focus to be unavailable.');
						}
						continue;
					}
					expect(next).not.toBe(source.id);
					expect(previous).not.toBe(source.id);
					expect(panels.some((panelLeaf) => panelLeaf.id === next)).toBe(true);
					expect(panels.some((panelLeaf) => panelLeaf.id === previous)).toBe(true);
					expect(resolve(root, requireId(next), 'previous')).toBe(source.id);
					expect(resolve(root, requireId(previous), 'next')).toBe(source.id);
				}
			}),
			{ numRuns: 240, seed: 2_026_08_28 },
		);
	});
});

interface SplitInstruction {
	readonly orientation: PanelSplitOrientation;
	readonly targetIndex: number;
}

function resolve(
	root: PanelLayoutNode,
	sourcePanelId: string,
	direction: PanelFocusCycleDirection,
): string | null {
	return resolveCyclicPanelFocus({
		direction,
		maximizedPanelId: null,
		root,
		sourcePanelId,
	});
}

function panel(id: string): PanelLeaf {
	return {
		activeChildPresentationId: null,
		childPresentations: [],
		id,
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'unknown',
			currentCwd: null,
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
): PanelSplit {
	return {
		children: [
			{ id: `${id}-leading-slot`, node: leading, weight: 50 },
			{ id: `${id}-trailing-slot`, node: trailing, weight: 50 },
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
			split(splitId, instruction.orientation, target, panel(`panel-${index + 2}`)),
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

function requireId(id: string | null): string {
	if (id === null) throw new Error('Expected a cyclic focus target.');
	return id;
}
