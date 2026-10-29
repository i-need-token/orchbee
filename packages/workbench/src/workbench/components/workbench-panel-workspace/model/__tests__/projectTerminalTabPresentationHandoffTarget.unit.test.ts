import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { PanelLayoutNode, PanelLeaf, TabPanelWorkspace } from '../panel-workspace-state';
import { projectTerminalTabPresentationHandoffTarget } from '../projectTerminalTabPresentationHandoffTarget';

function panel(id: string): PanelLeaf {
	return {
		activeChildPresentationId: null,
		childPresentations: [],
		id,
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'idle',
			currentCwd: null,
			launchCwd: null,
			lifecycle: { kind: 'preview' },
			naturalTitle: null,
			presentationId: terminalPresentationId(`${id}-presentation`),
			title: id,
		},
	};
}

function split(
	id: string,
	left: PanelLayoutNode,
	right: PanelLayoutNode,
	leftWeight = 50,
	rightWeight = 50,
): PanelLayoutNode {
	return {
		children: [
			{ id: `${id}-left`, node: left, weight: leftWeight },
			{ id: `${id}-right`, node: right, weight: rightWeight },
		],
		id,
		kind: 'split',
		orientation: 'horizontal',
	};
}

function tab(root: PanelLayoutNode, maximizedPanelId: string | null = null): TabPanelWorkspace {
	return {
		focusedPanelId: 'panel-a',
		maximizedPanelId,
		panelFocusHistory: ['panel-a'],
		root,
		tabId: 'tab-a',
	};
}

describe('projectTerminalTabPresentationHandoffTarget', () => {
	it('requires every displayed terminal in positive-weight branches', () => {
		expect(
			projectTerminalTabPresentationHandoffTarget(
				tab(split('root', panel('panel-a'), panel('panel-b'))),
				true,
			),
		).toEqual(['panel-a-presentation', 'panel-b-presentation']);
	});

	it('excludes an entire zero-weight branch from the handoff barrier', () => {
		const hiddenBranch = split('hidden', panel('panel-b'), panel('panel-c'));

		expect(
			projectTerminalTabPresentationHandoffTarget(
				tab(split('root', panel('panel-a'), hiddenBranch, 100, 0)),
				true,
			),
		).toEqual(['panel-a-presentation']);
	});

	it('requires only the exact maximized Panel even beneath a zero-weight canonical ancestor', () => {
		const nested = split('nested', panel('panel-b'), panel('panel-c'));
		const root = split('root', panel('panel-a'), nested, 100, 0);

		expect(projectTerminalTabPresentationHandoffTarget(tab(root, 'panel-c'), true)).toEqual([
			'panel-c-presentation',
		]);
	});

	it('falls back to the canonical positive-weight projection for an invalid maximized identity', () => {
		expect(
			projectTerminalTabPresentationHandoffTarget(
				tab(split('root', panel('panel-a'), panel('panel-b')), 'missing-panel'),
				true,
			),
		).toEqual(['panel-a-presentation', 'panel-b-presentation']);
	});

	it('requires no terminal when the surface adapter is unavailable', () => {
		expect(projectTerminalTabPresentationHandoffTarget(tab(panel('panel-a')), false)).toEqual([]);
	});
});
