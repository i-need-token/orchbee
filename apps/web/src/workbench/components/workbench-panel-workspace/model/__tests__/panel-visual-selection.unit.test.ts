import { assert, integer, property } from 'fast-check';
import { describe, expect, it } from 'vitest';

import type { PanelLayoutNode, PanelLeaf, PanelSplit } from '../panel-workspace-state';
import type { PanelSplitOrientation } from '../panel-layout-types';
import {
	startPanelVisualSelection,
	transitionPanelVisualSelection,
} from '../panel-visual-selection';

describe('panel visual selection', () => {
	it('rejects singleton, stale-source, and maximized layouts', () => {
		const singleton = panel('panel-1');
		const pair = split('split-1', 'horizontal', singleton, panel('panel-2'));

		expect(start(singleton, 'panel-1', 'focus')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});
		expect(start(pair, 'missing', 'focus')).toEqual({
			kind: 'unavailable',
			reason: 'source-panel-unavailable',
		});
		expect(
			startPanelVisualSelection({
				maximizedPanelId: 'panel-1',
				mode: 'focus',
				root: pair,
				sourcePanelId: 'panel-1',
				tabId: 'tab-1',
			}),
		).toEqual({ kind: 'unavailable', reason: 'panel-maximized' });
	});

	it('delegates a two-Panel focus or swap directly to the sole other Panel', () => {
		const pair = split('split-1', 'horizontal', panel('panel-1'), panel('panel-2'));

		expect(start(pair, 'panel-1', 'focus')).toEqual({
			command: {
				kind: 'focus',
				sourcePanelId: 'panel-1',
				tabId: 'tab-1',
				targetPanelId: 'panel-2',
			},
			kind: 'direct',
		});
		expect(start(pair, 'panel-2', 'swap')).toEqual({
			command: {
				kind: 'swap',
				sourcePanelId: 'panel-2',
				tabId: 'tab-1',
				targetPanelId: 'panel-1',
			},
			kind: 'direct',
		});
	});

	it('labels canonical targets while excluding the source Panel', () => {
		const root = split(
			'split-root',
			'horizontal',
			panel('panel-1'),
			split('split-inner', 'vertical', panel('panel-2'), panel('panel-3')),
		);
		const result = start(root, 'panel-2', 'focus');
		if (result.kind !== 'started') throw new Error('Expected a visual selection session.');

		expect(result.session).toMatchObject({
			mode: 'focus',
			panelIds: ['panel-1', 'panel-2', 'panel-3'],
			prefix: '',
			sourcePanelId: 'panel-2',
			tabId: 'tab-1',
			targets: [
				{ label: '1', panelId: 'panel-1' },
				{ label: '2', panelId: 'panel-3' },
			],
		});
	});

	it('uses equal-width prefix-free labels beyond the single-character alphabet', () => {
		const result = start(generatedLayout(38), 'panel-1', 'focus');
		if (result.kind !== 'started') throw new Error('Expected a visual selection session.');

		expect(result.session.targets).toHaveLength(37);
		expect(result.session.targets[0]?.label).toBe('11');
		expect(result.session.targets[35]?.label).toBe('1Z');
		expect(result.session.targets[36]?.label).toBe('21');
		expect(new Set(result.session.targets.map((target) => target.label)).size).toBe(37);
		expect(
			result.session.targets.every((target) =>
				result.session.targets.every(
					(candidate) => candidate === target || !candidate.label.startsWith(target.label),
				),
			),
		).toBe(true);
	});

	it('owns invalid, composing, and repeated keys while preserving the current prefix', () => {
		const result = start(generatedLayout(38), 'panel-1', 'focus');
		if (result.kind !== 'started') throw new Error('Expected a visual selection session.');

		const prefixed = transitionPanelVisualSelection(result.session, {
			code: 'Digit1',
			isComposing: false,
			repeat: false,
			type: 'key-pressed',
		});
		expect(prefixed).toMatchObject({ kind: 'active', outcome: 'updated' });
		if (prefixed.kind !== 'active') throw new Error('Expected the prefix to remain active.');
		expect(prefixed.session.prefix).toBe('1');

		for (const event of [
			{ code: 'Slash', isComposing: false, repeat: false, type: 'key-pressed' as const },
			{ code: 'Digit2', isComposing: true, repeat: false, type: 'key-pressed' as const },
			{ code: 'Digit2', isComposing: false, repeat: true, type: 'key-pressed' as const },
		]) {
			const unchanged = transitionPanelVisualSelection(prefixed.session, event);
			expect(unchanged).toEqual({
				kind: 'active',
				outcome: 'unchanged',
				session: prefixed.session,
			});
		}

		const backedUp = transitionPanelVisualSelection(prefixed.session, {
			code: 'Backspace',
			isComposing: false,
			repeat: false,
			type: 'key-pressed',
		});
		expect(backedUp).toMatchObject({ kind: 'active', outcome: 'updated' });
		if (backedUp.kind !== 'active') throw new Error('Expected Backspace to retain the session.');
		expect(backedUp.session.prefix).toBe('');
	});

	it('selects complete physical labels and restores focus according to the mode', () => {
		const root = generatedLayout(3);
		const focus = start(root, 'panel-1', 'focus');
		const swap = start(root, 'panel-1', 'swap');
		if (focus.kind !== 'started' || swap.kind !== 'started') {
			throw new Error('Expected visual selection sessions.');
		}

		expect(
			transitionPanelVisualSelection(focus.session, {
				code: 'Digit2',
				isComposing: false,
				repeat: false,
				type: 'key-pressed',
			}),
		).toEqual({
			command: {
				kind: 'focus',
				sourcePanelId: 'panel-1',
				tabId: 'tab-1',
				targetPanelId: 'panel-3',
			},
			kind: 'closed',
			outcome: 'selected',
			restorePanelId: 'panel-3',
		});
		expect(
			transitionPanelVisualSelection(swap.session, {
				panelId: 'panel-2',
				type: 'target-selected',
			}),
		).toEqual({
			command: {
				kind: 'swap',
				sourcePanelId: 'panel-1',
				tabId: 'tab-1',
				targetPanelId: 'panel-2',
			},
			kind: 'closed',
			outcome: 'selected',
			restorePanelId: 'panel-1',
		});
	});

	it('distinguishes explicit cancellation from stale cancellation', () => {
		const root = generatedLayout(3);
		const result = start(root, 'panel-1', 'focus');
		if (result.kind !== 'started') throw new Error('Expected a visual selection session.');

		expect(
			transitionPanelVisualSelection(result.session, {
				code: 'Escape',
				isComposing: false,
				repeat: false,
				type: 'key-pressed',
			}),
		).toEqual({
			command: null,
			kind: 'closed',
			outcome: 'cancelled',
			restorePanelId: 'panel-1',
		});
		expect(
			transitionPanelVisualSelection(result.session, {
				restoreSourceFocus: false,
				type: 'cancelled',
			}),
		).toEqual({
			command: null,
			kind: 'closed',
			outcome: 'cancelled',
			restorePanelId: null,
		});
		expect(
			transitionPanelVisualSelection(result.session, {
				activeTabId: 'tab-2',
				focusedPanelId: 'panel-1',
				maximizedPanelId: null,
				root,
				type: 'workspace-revalidated',
			}),
		).toEqual({ command: null, kind: 'closed', outcome: 'stale', restorePanelId: null });
	});

	it('retains only an exact live Tab, source focus, order, membership, and unmaximized layout', () => {
		const root = generatedLayout(3);
		const result = start(root, 'panel-1', 'focus');
		if (result.kind !== 'started') throw new Error('Expected a visual selection session.');
		const unchanged = transitionPanelVisualSelection(result.session, {
			activeTabId: 'tab-1',
			focusedPanelId: 'panel-1',
			maximizedPanelId: null,
			root,
			type: 'workspace-revalidated',
		});
		expect(unchanged).toEqual({
			kind: 'active',
			outcome: 'unchanged',
			session: result.session,
		});

		for (const event of [
			{
				activeTabId: 'tab-1',
				focusedPanelId: 'panel-2',
				maximizedPanelId: null,
				root,
				type: 'workspace-revalidated' as const,
			},
			{
				activeTabId: 'tab-1',
				focusedPanelId: 'panel-1',
				maximizedPanelId: 'panel-1',
				root,
				type: 'workspace-revalidated' as const,
			},
			{
				activeTabId: 'tab-1',
				focusedPanelId: 'panel-1',
				maximizedPanelId: null,
				root: split('reordered', 'horizontal', panel('panel-2'), panel('panel-1')),
				type: 'workspace-revalidated' as const,
			},
		]) {
			expect(transitionPanelVisualSelection(result.session, event)).toEqual({
				command: null,
				kind: 'closed',
				outcome: 'stale',
				restorePanelId: null,
			});
		}
	});

	it('keeps every generated canonical target unique, prefix-free, ordered, and reachable', () => {
		assert(
			property(integer({ min: 3, max: 80 }), integer({ min: 0, max: 255 }), (count, rawIndex) => {
				const root = generatedLayout(count);
				const panelIds = Array.from({ length: count }, (_value, index) => `panel-${index + 1}`);
				const sourcePanelId = panelIds[rawIndex % count];
				if (sourcePanelId === undefined) throw new Error('Expected a generated source Panel.');
				const result = start(root, sourcePanelId, 'focus');
				if (result.kind !== 'started') throw new Error('Expected a generated session.');
				const expectedTargets = panelIds.filter((panelId) => panelId !== sourcePanelId);

				expect(result.session.targets.map((target) => target.panelId)).toEqual(expectedTargets);
				expect(new Set(result.session.targets.map((target) => target.label)).size).toBe(
					expectedTargets.length,
				);
				for (const target of result.session.targets) {
					for (const candidate of result.session.targets) {
						if (candidate !== target && candidate.label.startsWith(target.label)) {
							throw new Error('Generated visual-selection labels must be prefix-free.');
						}
					}
					let session = result.session;
					let selectedPanelId: string | null = null;
					let selectedOutcome: string | null = null;
					for (const character of target.label) {
						const code =
							character >= '0' && character <= '9' ? `Digit${character}` : `Key${character}`;
						const transition = transitionPanelVisualSelection(session, {
							code,
							isComposing: false,
							repeat: false,
							type: 'key-pressed',
						});
						if (transition.kind === 'active') session = transition.session;
						else {
							selectedPanelId = transition.command?.targetPanelId ?? null;
							selectedOutcome = transition.outcome;
						}
					}
					expect(selectedPanelId).toBe(target.panelId);
					expect(selectedOutcome).toBe('selected');
				}
			}),
			{ numRuns: 180, seed: 2_026_08_28 },
		);
	});
});

function start(root: PanelLayoutNode, sourcePanelId: string, mode: 'focus' | 'swap') {
	return startPanelVisualSelection({
		maximizedPanelId: null,
		mode,
		root,
		sourcePanelId,
		tabId: 'tab-1',
	});
}

function panel(id: string): PanelLeaf {
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

function generatedLayout(count: number): PanelLayoutNode {
	let root: PanelLayoutNode = panel('panel-1');
	for (let index = 1; index < count; index += 1) {
		root = split(
			`split-${index}`,
			index % 2 === 0 ? 'horizontal' : 'vertical',
			root,
			panel(`panel-${index + 1}`),
		);
	}
	return root;
}
