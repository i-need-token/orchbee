import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelSplit } from '../panel-workspace-state';
import { resolvePanelResize } from '../resolvePanelResize';

describe('resolvePanelResize', () => {
	it('adjusts the nearest split ancestor on the requested axis', () => {
		const root = nestedLayout();

		const wider = resolvePanelResize(root, 'panel-3', 'wider', 1);
		expect(wider).toMatchObject({ kind: 'applied' });
		expect(requireSplit(wider.root).children.map((child) => child.weight)).toEqual([45, 55]);
		expect(
			requireSplit(requireSplit(wider.root).children[1]?.node).children.map(
				(child) => child.weight,
			),
		).toEqual([50, 50]);

		const taller = resolvePanelResize(root, 'panel-3', 'taller', 1);
		expect(taller).toMatchObject({ kind: 'applied' });
		expect(requireSplit(taller.root).children.map((child) => child.weight)).toEqual([50, 50]);
		expect(
			requireSplit(requireSplit(taller.root).children[1]?.node).children.map(
				(child) => child.weight,
			),
		).toEqual([45, 55]);
	});

	it('applies normal and doubled steps symmetrically for either child', () => {
		const root = horizontalLayout(40, 60);

		expect(weights(resolvePanelResize(root, 'panel-1', 'wider', 1).root)).toEqual([45, 55]);
		expect(weights(resolvePanelResize(root, 'panel-1', 'narrower', 2).root)).toEqual([30, 70]);
		expect(weights(resolvePanelResize(root, 'panel-2', 'wider', 2).root)).toEqual([30, 70]);
		expect(weights(resolvePanelResize(root, 'panel-2', 'narrower', 1).root)).toEqual([45, 55]);
	});

	it('retains a visible branch and reports an intentional boundary no-op', () => {
		const root = horizontalLayout(5, 95);

		const blocked = resolvePanelResize(root, 'panel-1', 'narrower', 2);
		expect(blocked).toMatchObject({ kind: 'unchanged', reason: 'boundary' });
		expect(blocked.root).toBe(root);

		const admitted = resolvePanelResize(root, 'panel-1', 'wider', 2);
		expect(admitted).toMatchObject({ kind: 'applied' });
		expect(weights(admitted.root)).toEqual([15, 85]);
	});

	it('never reverses direction when an imported layout is already beyond the boundary', () => {
		const narrowLeading = horizontalLayout(2, 98);
		const narrowTrailing = horizontalLayout(98, 2);

		expect(resolvePanelResize(narrowLeading, 'panel-1', 'narrower', 1)).toMatchObject({
			kind: 'unchanged',
			reason: 'boundary',
			root: narrowLeading,
		});
		expect(weights(resolvePanelResize(narrowLeading, 'panel-1', 'wider', 1).root)).toEqual([7, 93]);
		expect(resolvePanelResize(narrowTrailing, 'panel-1', 'wider', 1)).toMatchObject({
			kind: 'unchanged',
			reason: 'boundary',
			root: narrowTrailing,
		});
		expect(weights(resolvePanelResize(narrowTrailing, 'panel-1', 'narrower', 1).root)).toEqual([
			93, 7,
		]);
	});

	it('leaves an unavailable axis unchanged without falling back to another split', () => {
		const root = verticalLayout(50, 50);

		expect(resolvePanelResize(root, 'panel-1', 'wider', 1)).toMatchObject({
			kind: 'unchanged',
			reason: 'axis-unavailable',
			root,
		});
	});

	it('resets every split in the target Tab while preserving topology and identities', () => {
		const root = nestedLayout(25, 75, 20, 80);

		const reset = resolvePanelResize(root, 'panel-3', 'reset', 1);
		expect(reset).toMatchObject({ kind: 'applied' });
		const resetRoot = requireSplit(reset.root);
		expect(resetRoot.id).toBe('split-root');
		expect(resetRoot.children.map((child) => child.weight)).toEqual([50, 50]);
		const resetNested = requireSplit(resetRoot.children[1]?.node);
		expect(resetNested.id).toBe('split-nested');
		expect(resetNested.children.map((child) => child.weight)).toEqual([50, 50]);

		const alreadyReset = resolvePanelResize(reset.root, 'panel-3', 'reset', 2);
		expect(alreadyReset).toMatchObject({ kind: 'unchanged', reason: 'already-reset' });
		expect(alreadyReset.root).toBe(reset.root);
	});

	it('rejects stale targets and invalid multipliers without producing a layout', () => {
		const root = horizontalLayout(50, 50);

		expect(resolvePanelResize(root, 'missing-panel', 'wider', 1)).toEqual({
			kind: 'rejected',
			reason: 'panel-not-found',
			root,
		});
		expect(resolvePanelResize(root, 'panel-1', 'wider', 3 as 1)).toEqual({
			kind: 'rejected',
			reason: 'invalid-multiplier',
			root,
		});
	});
});

function panel(id: string): Extract<PanelLayoutNode, { readonly kind: 'panel' }> {
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
