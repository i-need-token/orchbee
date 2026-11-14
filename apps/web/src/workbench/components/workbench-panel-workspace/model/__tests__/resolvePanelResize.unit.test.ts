import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelSplit } from '../panel-workspace-state';
import { resolvePanelResize } from '../resolvePanelResize';

describe('resolvePanelResize', () => {
	it('adjusts the nearest split ancestor on the requested axis', () => {
		const root = nestedLayout();

		const wider = resolvePanelResize(root, 'panel-3', 'wider');
		expect(wider).toMatchObject({ kind: 'applied' });
		expect(requireSplit(wider.root).children.map((child) => child.weight)).toEqual([45, 55]);
		expect(
			requireSplit(requireSplit(wider.root).children[1]?.node).children.map(
				(child) => child.weight,
			),
		).toEqual([50, 50]);

		const taller = resolvePanelResize(root, 'panel-3', 'taller');
		expect(taller).toMatchObject({ kind: 'applied' });
		expect(requireSplit(taller.root).children.map((child) => child.weight)).toEqual([50, 50]);
		expect(
			requireSplit(requireSplit(taller.root).children[1]?.node).children.map(
				(child) => child.weight,
			),
		).toEqual([45, 55]);
	});

	it('applies steps symmetrically for either child', () => {
		const root = horizontalLayout(40, 60);

		expect(weights(resolvePanelResize(root, 'panel-1', 'wider').root)).toEqual([45, 55]);
		expect(weights(twoSteps(root, 'panel-1', 'narrower'))).toEqual([30, 70]);
		expect(weights(twoSteps(root, 'panel-2', 'wider'))).toEqual([30, 70]);
		expect(weights(resolvePanelResize(root, 'panel-2', 'narrower').root)).toEqual([45, 55]);
	});

	it('retains a visible branch and reports an intentional boundary no-op', () => {
		const root = horizontalLayout(5, 95);

		const blocked = resolvePanelResize(root, 'panel-1', 'narrower');
		expect(blocked).toMatchObject({ kind: 'unchanged', reason: 'boundary' });
		expect(blocked.root).toBe(root);

		const admitted = resolvePanelResize(root, 'panel-1', 'wider');
		expect(admitted).toMatchObject({ kind: 'applied' });
		expect(weights(twoSteps(root, 'panel-1', 'wider'))).toEqual([15, 85]);
	});

	it('never reverses direction when an imported layout is already beyond the boundary', () => {
		const narrowLeading = horizontalLayout(2, 98);
		const narrowTrailing = horizontalLayout(98, 2);

		expect(resolvePanelResize(narrowLeading, 'panel-1', 'narrower')).toMatchObject({
			kind: 'unchanged',
			reason: 'boundary',
			root: narrowLeading,
		});
		expect(weights(resolvePanelResize(narrowLeading, 'panel-1', 'wider').root)).toEqual([7, 93]);
		expect(resolvePanelResize(narrowTrailing, 'panel-1', 'wider')).toMatchObject({
			kind: 'unchanged',
			reason: 'boundary',
			root: narrowTrailing,
		});
		expect(weights(resolvePanelResize(narrowTrailing, 'panel-1', 'narrower').root)).toEqual([
			93, 7,
		]);
	});

	it('leaves an unavailable axis unchanged without falling back to another split', () => {
		const root = verticalLayout(50, 50);

		expect(resolvePanelResize(root, 'panel-1', 'wider')).toMatchObject({
			kind: 'unchanged',
			reason: 'axis-unavailable',
			root,
		});
	});

	it('rejects stale targets without producing a layout', () => {
		const root = horizontalLayout(50, 50);

		expect(resolvePanelResize(root, 'missing-panel', 'wider')).toEqual({
			kind: 'rejected',
			reason: 'panel-not-found',
			root,
		});
	});
});

function twoSteps(
	root: PanelLayoutNode,
	panelId: string,
	adjustment: Parameters<typeof resolvePanelResize>[2],
): PanelLayoutNode {
	return resolvePanelResize(resolvePanelResize(root, panelId, adjustment).root, panelId, adjustment)
		.root;
}

function panel(id: string): Extract<PanelLayoutNode, { readonly kind: 'panel' }> {
	return {
		activeChildPresentationId: null,
		childPresentations: [],
		id,
		kind: 'panel',
		primaryPresentation: {
			currentCwd: null,
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: `terminal-${id}` as never,
			title: id,
		},
	};
}

function horizontalLayout(leadingWeight: number, trailingWeight: number): PanelSplit {
	return {
		children: [
			{ id: 'slot-1', node: panel('panel-1'), weight: leadingWeight },
			{ id: 'slot-2', node: panel('panel-2'), weight: trailingWeight },
		],
		id: 'split-root',
		kind: 'split',
		orientation: 'horizontal',
	};
}

function verticalLayout(leadingWeight: number, trailingWeight: number): PanelSplit {
	return {
		...horizontalLayout(leadingWeight, trailingWeight),
		orientation: 'vertical',
	};
}

function nestedLayout(
	rootLeadingWeight = 50,
	rootTrailingWeight = 50,
	nestedLeadingWeight = 50,
	nestedTrailingWeight = 50,
): PanelSplit {
	return {
		children: [
			{ id: 'slot-1', node: panel('panel-1'), weight: rootLeadingWeight },
			{
				id: 'slot-2',
				node: {
					children: [
						{ id: 'slot-3', node: panel('panel-2'), weight: nestedLeadingWeight },
						{ id: 'slot-4', node: panel('panel-3'), weight: nestedTrailingWeight },
					],
					id: 'split-nested',
					kind: 'split',
					orientation: 'vertical',
				},
				weight: rootTrailingWeight,
			},
		],
		id: 'split-root',
		kind: 'split',
		orientation: 'horizontal',
	};
}

function requireSplit(node: PanelLayoutNode | undefined): PanelSplit {
	if (node?.kind !== 'split') throw new Error('Expected a split node.');
	return node;
}

function weights(node: PanelLayoutNode): readonly number[] {
	return requireSplit(node).children.map((child) => child.weight);
}
