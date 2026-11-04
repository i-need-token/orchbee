import { array, assert, constantFrom, nat, property, record } from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelLeaf, PanelSplit } from '../panel-workspace-state';
import type { PanelSplitOrientation } from '../panel-layout-types';
import {
	resolvePanelFocusByPosition,
	type PanelFocusPosition,
} from '../resolvePanelFocusByPosition';

describe('resolvePanelFocusByPosition', () => {
	it('resolves exact first, nested, current, and tenth canonical leaf positions', () => {
		const root = generatedLayout(
			Array.from({ length: 9 }, (_value, index) => ({
				orientation: index % 2 === 0 ? ('horizontal' as const) : ('vertical' as const),
				targetIndex: index,
			})),
		);

		expect(resolve(root, 'panel-10', 1)).toEqual({ kind: 'ready', panelId: 'panel-1' });
		expect(resolve(root, 'panel-10', 5)).toEqual({ kind: 'ready', panelId: 'panel-5' });
		expect(resolve(root, 'panel-10', 10)).toEqual({ kind: 'ready', panelId: 'panel-10' });
	});

	it('keeps singleton position one ready and rejects an exact missing position', () => {
		const root = panel('panel-1');

		expect(resolve(root, 'panel-1', 1)).toEqual({ kind: 'ready', panelId: 'panel-1' });
		expect(resolve(root, 'panel-1', 2)).toEqual({
			kind: 'unavailable',
			reason: 'panel-position-unavailable',
		});
	});

	it('rejects maximization, a stale source, and duplicate canonical Panel identities', () => {
		const pair = split('split-root', 'horizontal', panel('panel-1'), panel('panel-2'));

		expect(
			resolvePanelFocusByPosition({
				maximizedPanelId: 'panel-2',
				position: 1,
				root: pair,
				sourcePanelId: 'panel-1',
			}),
		).toEqual({ kind: 'unavailable', reason: 'panel-maximized' });
		expect(resolve(pair, 'missing-panel', 1)).toEqual({
			kind: 'unavailable',
			reason: 'source-panel-unavailable',
		});
		expect(
			resolve(
				split('split-duplicate', 'vertical', panel('panel-1'), panel('panel-1')),
				'panel-1',
				1,
			),
		).toEqual({ kind: 'unavailable', reason: 'invalid-panel-order' });
	});

	it('matches canonical membership exactly across generated nested layouts', () => {
		const instructionArbitrary = record({
			orientation: constantFrom<PanelSplitOrientation>('horizontal', 'vertical'),
			targetIndex: nat({ max: 64 }),
		});

		assert(
			property(array(instructionArbitrary, { maxLength: 11 }), (instructions) => {
				const root = generatedLayout(instructions);
				const panels = collectPanels(root);
				for (let position = 1; position <= 10; position += 1) {
					const resolution = resolve(
						root,
						panels.at(-1)?.id ?? 'panel-1',
						position as PanelFocusPosition,
					);
					const expected = panels[position - 1];
					expect(resolution).toEqual(
						expected === undefined
							? { kind: 'unavailable', reason: 'panel-position-unavailable' }
							: { kind: 'ready', panelId: expected.id },
					);
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
	position: PanelFocusPosition,
): ReturnType<typeof resolvePanelFocusByPosition> {
	return resolvePanelFocusByPosition({
		maximizedPanelId: null,
		position,
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
