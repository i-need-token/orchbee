import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { PanelLayoutNode, PanelLeaf, TabPanelWorkspace } from '../panel-workspace-state';
import { projectTerminalTabPresentationRevision } from '../projectTerminalTabPresentationRevision';

function panel(id: string, childIds: readonly string[] = []): PanelLeaf {
	const children = childIds.map((childId) => ({
		closeActivity: 'idle' as const,
		currentCwd: null,
		launchCwd: null,
		lifecycle: { kind: 'preview' as const },
		naturalTitle: null,
		presentationId: terminalPresentationId(childId),
		title: childId,
	}));
	return {
		activeChildPresentationId: children[0]?.presentationId ?? null,
		childPresentations: children,
		id,
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'idle',
			currentCwd: null,
			launchCwd: null,
			lifecycle: { kind: 'preview' },
			naturalTitle: null,
			presentationId: terminalPresentationId(`${id}-primary`),
			title: id,
		},
	};
}

function split(left: PanelLeaf, right: PanelLeaf, weights: readonly [number, number] = [50, 50]) {
	return {
		children: [
			{ id: 'slot-left', node: left, weight: weights[0] },
			{ id: 'slot-right', node: right, weight: weights[1] },
		],
		id: 'split-root',
		kind: 'split' as const,
		orientation: 'horizontal' as const,
	};
}

function tab(root: PanelLayoutNode): TabPanelWorkspace {
	return {
		focusedPanelId: 'panel-a',
		maximizedPanelId: null,
		panelFocusHistory: ['panel-a'],
		root,
		tabId: 'tab-a',
	};
}

describe('projectTerminalTabPresentationRevision', () => {
	it('returns no revision without a target Tab', () => {
		expect(projectTerminalTabPresentationRevision('scope-a', null)).toBeNull();
	});

	it('changes for presentation scope, topology, order, maximization, and displayed terminal', () => {
		const panelA = panel('panel-a', ['child-a', 'child-b']);
		const panelB = panel('panel-b');
		const baseline = tab(split(panelA, panelB));
		const revision = projectTerminalTabPresentationRevision('scope-a', baseline);

		expect(projectTerminalTabPresentationRevision('scope-b', baseline)).not.toBe(revision);
		expect(
			projectTerminalTabPresentationRevision('scope-a', {
				...baseline,
				maximizedPanelId: 'panel-a',
			}),
		).not.toBe(revision);
		expect(
			projectTerminalTabPresentationRevision('scope-a', {
				...baseline,
				root: split(panelB, panelA),
			}),
		).not.toBe(revision);
		expect(
			projectTerminalTabPresentationRevision('scope-a', {
				...baseline,
				root: panelA,
			}),
		).not.toBe(revision);
		expect(
			projectTerminalTabPresentationRevision('scope-a', {
				...baseline,
				root: split(
					{
						...panelA,
						activeChildPresentationId: terminalPresentationId('child-b'),
					},
					panelB,
				),
			}),
		).not.toBe(revision);
	});

	it('ignores focus, terminal metadata, lifecycle, and ordinary positive weights', () => {
		const panelA = panel('panel-a');
		const panelB = panel('panel-b');
		const baseline = tab(split(panelA, panelB));
		const changedPanelA: PanelLeaf = {
			...panelA,
			primaryPresentation: {
				...panelA.primaryPresentation,
				currentCwd: '/changed',
				lifecycle: { kind: 'running' },
				naturalTitle: 'changed',
				title: 'changed',
			},
		};
		const changed: TabPanelWorkspace = {
			...baseline,
			focusedPanelId: 'panel-b',
			panelFocusHistory: ['panel-a', 'panel-b'],
			root: split(changedPanelA, panelB, [70, 30]),
		};

		expect(projectTerminalTabPresentationRevision('scope-a', changed)).toBe(
			projectTerminalTabPresentationRevision('scope-a', baseline),
		);
	});
});
