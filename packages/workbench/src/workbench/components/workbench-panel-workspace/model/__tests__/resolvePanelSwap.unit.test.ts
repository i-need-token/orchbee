import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelLeaf, PanelSplit } from '../panel-workspace-state';
import { resolvePanelSwap } from '../resolvePanelSwap';

describe('resolvePanelSwap', () => {
	it('exchanges complete nested leaves while preserving the layout topology', () => {
		const root = nestedLayout();
		const source = requirePanel(root.children[0]?.node);
		const nested = requireSplit(root.children[1]?.node);
		const target = requirePanel(nested.children[1]?.node);

		const result = resolvePanelSwap(root, source.id, target.id);

		expect(result).toMatchObject({ kind: 'applied' });
		const swappedRoot = requireSplit(result.root);
		const swappedNested = requireSplit(swappedRoot.children[1]?.node);
		expect(swappedRoot).not.toBe(root);
		expect(swappedRoot.id).toBe(root.id);
		expect(swappedRoot.orientation).toBe(root.orientation);
		expect(swappedRoot.children.map(({ id, weight }) => ({ id, weight }))).toEqual(
			root.children.map(({ id, weight }) => ({ id, weight })),
		);
		expect(swappedNested.id).toBe(nested.id);
		expect(swappedNested.orientation).toBe(nested.orientation);
		expect(swappedNested.children.map(({ id, weight }) => ({ id, weight }))).toEqual(
			nested.children.map(({ id, weight }) => ({ id, weight })),
		);
		expect(swappedRoot.children[0]?.node).toBe(target);
		expect(swappedNested.children[1]?.node).toBe(source);
		expect(swappedNested.children[0]).toBe(nested.children[0]);
	});

	it('is an involution over the admitted canonical layout', () => {
		const root = nestedLayout();
		const first = resolvePanelSwap(root, 'panel-1', 'panel-3');
		const second = resolvePanelSwap(first.root, 'panel-1', 'panel-3');

		expect(first.kind).toBe('applied');
		expect(second.kind).toBe('applied');
		expect(second.root).toEqual(root);
	});

	it('keeps the same Panel as an intentional identity-preserving no-op', () => {
		const root = nestedLayout();

		expect(resolvePanelSwap(root, 'panel-2', 'panel-2')).toEqual({
			kind: 'unchanged',
			reason: 'same-panel',
			root,
		});
	});

	it.each([
		['missing source', 'missing-panel', 'panel-2'],
		['missing target', 'panel-2', 'missing-panel'],
	] as const)(
		'rejects a %s without producing a partial layout',
		(_, sourcePanelId, targetPanelId) => {
			const root = nestedLayout();

			expect(resolvePanelSwap(root, sourcePanelId, targetPanelId)).toEqual({
				kind: 'rejected',
				reason: 'panel-not-found',
				root,
			});
		},
	);
});

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

function nestedLayout(): PanelSplit {
	return {
		children: [
			{ id: 'slot-1', node: panel('panel-1'), weight: 35 },
			{
				id: 'slot-2',
				node: {
					children: [
						{ id: 'slot-3', node: panel('panel-2'), weight: 40 },
						{ id: 'slot-4', node: panel('panel-3'), weight: 60 },
					],
					id: 'split-nested',
					kind: 'split',
					orientation: 'vertical',
				},
				weight: 65,
			},
		],
		id: 'split-root',
		kind: 'split',
		orientation: 'horizontal',
	};
}

function requirePanel(node: PanelLayoutNode | undefined): PanelLeaf {
	if (node?.kind !== 'panel') throw new Error('Expected a Panel leaf.');
	return node;
}

function requireSplit(node: PanelLayoutNode | undefined): PanelSplit {
	if (node?.kind !== 'split') throw new Error('Expected a Split node.');
	return node;
}
