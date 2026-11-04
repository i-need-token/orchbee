import { array, assert, constantFrom, nat, property, record } from 'fast-check';
import { describe, expect, it, vi } from 'vitest';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputSink,
} from '../../../../types/workbench-terminal-surface';
import type { PanelLayoutNode, PanelLeaf, PanelSplit } from '../panel-workspace-state';
import type { PanelSplitOrientation } from '../panel-layout-types';
import {
	dispatchPanelBroadcastInput,
	preflightPanelBroadcast,
	resolvePanelBroadcastStart,
	transitionPanelBroadcastInput,
	type PanelBroadcastEnvironment,
	type PanelBroadcastInputEnvelope,
	type PanelBroadcastTargetSnapshot,
} from '../panel-broadcast-input';

describe('panel broadcast input', () => {
	it('captures every displayed running Panel terminal in canonical order', () => {
		const root = split(
			'split-root',
			'horizontal',
			panel('panel-1'),
			split('split-nested', 'vertical', panel('panel-2'), panel('panel-3')),
		);
		const environment = environmentFor(root);

		expect(resolvePanelBroadcastStart(environment, 'panel-2')).toEqual({
			kind: 'ready',
			snapshot: {
				sourcePanelId: 'panel-1',
				tabId: 'tab-1',
				targets: [
					{
						panelId: 'panel-1',
						presentationId: terminalId('panel-1'),
						sinkRevision: 'revision-panel-1',
					},
					{
						panelId: 'panel-2',
						presentationId: terminalId('panel-2'),
						sinkRevision: 'revision-panel-2',
					},
					{
						panelId: 'panel-3',
						presentationId: terminalId('panel-3'),
						sinkRevision: 'revision-panel-3',
					},
				],
				topology:
					'split:split-root:horizontal(split-root-leading:panel:panel-1|split-root-trailing:split:split-nested:vertical(split-nested-leading:panel:panel-2|split-nested-trailing:panel:panel-3))',
			},
		});
	});

	it('rejects singleton, maximized, non-running, conflicting, and unavailable targets', () => {
		const singleton = environmentFor(panel('panel-1'));
		expect(resolvePanelBroadcastStart(singleton, 'panel-1')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const root = split('split-root', 'horizontal', panel('panel-1'), panel('panel-2'));
		const maximized = environmentFor(root, { maximizedPanelId: 'panel-1' });
		expect(resolvePanelBroadcastStart(maximized, 'panel-1')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});

		const notRunning = environmentFor(
			split('split-root', 'horizontal', panel('panel-1'), panel('panel-2', 'exited')),
		);
		expect(resolvePanelBroadcastStart(notRunning, 'panel-1')).toEqual({
			kind: 'unavailable',
			reason: 'target-not-running',
		});

		const conflicting = environmentFor(root, { conflictingInteraction: true });
		expect(resolvePanelBroadcastStart(conflicting, 'panel-1')).toEqual({
			kind: 'unavailable',
			reason: 'conflicting-interaction',
		});

		const unavailable = environmentFor(root);
		unavailable.sinks.delete(terminalId('panel-2'));
		expect(resolvePanelBroadcastStart(unavailable, 'panel-1')).toEqual({
			kind: 'unavailable',
			reason: 'target-not-write-ready',
		});
	});

	it('revalidates topology, displayed identities, and sink revisions exactly', () => {
		const root = split('split-root', 'horizontal', panel('panel-1'), panel('panel-2'));
		const environment = environmentFor(root);
		const resolution = resolvePanelBroadcastStart(environment, 'panel-1');
		if (resolution.kind !== 'ready') throw new Error('Expected a broadcast snapshot.');

		expect(preflightPanelBroadcast(environment, resolution.snapshot).kind).toBe('ready');
		expect(
			preflightPanelBroadcast(
				environmentFor(root, { focusedPanelId: 'panel-2' }),
				resolution.snapshot,
			),
		).toEqual({ kind: 'invalidated' });
		environment.sinks.set(terminalId('panel-2'), sink('changed-revision'));
		expect(preflightPanelBroadcast(environment, resolution.snapshot)).toEqual({
			kind: 'invalidated',
		});
	});

	it('keeps FIFO state, rejects stale completions, and clears sensitive input on stop', () => {
		const snapshot = snapshotForTwoTargets();
		const first = envelope(1, 1, { kind: 'text', text: 'secret-one' });
		const second = envelope(1, 2, keyInput('Enter'));
		let state = transitionPanelBroadcastInput(
			{ kind: 'idle' },
			{ epoch: 1, snapshot, type: 'started' },
		).state;
		state = transitionPanelBroadcastInput(state, { envelope: first, type: 'enqueued' }).state;
		state = transitionPanelBroadcastInput(state, { envelope: second, type: 'enqueued' }).state;
		const dispatch = transitionPanelBroadcastInput(state, { type: 'dispatch-requested' });
		expect(dispatch.command).toEqual({ envelope: first, kind: 'dispatch', snapshot });
		state = dispatch.state;

		const stale = transitionPanelBroadcastInput(state, {
			deliveredTargetCount: 2,
			epoch: 0,
			sequence: 1,
			targetCount: 2,
			type: 'delivery-completed',
		});
		expect(stale.state).toBe(state);

		const stopped = transitionPanelBroadcastInput(state, {
			deliveredTargetCount: 1,
			epoch: 1,
			sequence: 1,
			targetCount: 2,
			type: 'delivery-completed',
		});
		expect(stopped.state).toEqual({
			deliveredTargetCount: 1,
			epoch: 1,
			kind: 'stopped',
			reason: 'partial-delivery',
			targetCount: 2,
		});
		expect(JSON.stringify(stopped.state)).not.toContain('secret-one');
	});

	it('fans one envelope out concurrently and reports non-atomic partial delivery', async () => {
		const first = deferred<{ readonly kind: 'sent' }>();
		const second = deferred<{ readonly kind: 'failed' }>();
		const firstSend = vi.fn<
			(
				input: WorkbenchTerminalBroadcastInput,
				expectedRevision: string,
			) => Promise<{ readonly kind: 'sent' }>
		>(() => first.promise);
		const secondSend = vi.fn<
			(
				input: WorkbenchTerminalBroadcastInput,
				expectedRevision: string,
			) => Promise<{ readonly kind: 'failed' }>
		>(() => second.promise);
		const root = split('split-root', 'horizontal', panel('panel-1'), panel('panel-2'));
		const environment = environmentFor(root, {
			sinks: new Map([
				[terminalId('panel-1'), sink('revision-panel-1', firstSend)],
				[terminalId('panel-2'), sink('revision-panel-2', secondSend)],
			]),
		});
		const resolution = resolvePanelBroadcastStart(environment, 'panel-1');
		if (resolution.kind !== 'ready') throw new Error('Expected a broadcast snapshot.');
		const input = keyInput('Enter');
		const result = dispatchPanelBroadcastInput(
			environment,
			resolution.snapshot,
			envelope(1, 1, input),
		);

		expect(firstSend).toHaveBeenCalledWith(input, 'revision-panel-1');
		expect(secondSend).toHaveBeenCalledWith(input, 'revision-panel-2');
		first.resolve({ kind: 'sent' });
		second.resolve({ kind: 'failed' });
		await expect(result).resolves.toEqual({
			deliveredTargetCount: 1,
			epoch: 1,
			sequence: 1,
			targetCount: 2,
			type: 'delivery-completed',
		});
	});

	it('preserves exact canonical target membership across generated nested layouts', () => {
		const instruction = record({
			orientation: constantFrom<PanelSplitOrientation>('horizontal', 'vertical'),
			targetIndex: nat({ max: 64 }),
		});
		assert(
			property(array(instruction, { minLength: 1, maxLength: 10 }), (instructions) => {
				const root = generatedLayout(instructions);
				const expected = collectPanels(root);
				const environment = environmentFor(root);
				const resolution = resolvePanelBroadcastStart(environment, expected[0]?.id ?? 'missing');
				expect(resolution.kind).toBe('ready');
				if (resolution.kind !== 'ready') return;
				expect(resolution.snapshot.targets.map((target) => target.panelId)).toEqual(
					expected.map((candidate) => candidate.id),
				);
				expect(new Set(resolution.snapshot.targets.map((target) => target.panelId)).size).toBe(
					expected.length,
				);
			}),
			{ numRuns: 220, seed: 2_026_08_28 },
		);
	});
});

interface EnvironmentOverrides {
	readonly conflictingInteraction?: boolean;
	readonly focusedPanelId?: string;
	readonly maximizedPanelId?: string | null;
	readonly sinks?: Map<TerminalPresentationId, WorkbenchTerminalBroadcastInputSink>;
}

function environmentFor(
	root: PanelLayoutNode,
	overrides: EnvironmentOverrides = {},
): PanelBroadcastEnvironment & {
	readonly sinks: Map<TerminalPresentationId, WorkbenchTerminalBroadcastInputSink>;
} {
	const panels = collectPanels(root);
	const sinks =
		overrides.sinks ??
		new Map(
			panels.map((candidate) => [
				candidate.primaryPresentation.presentationId,
				sink(`revision-${candidate.id}`),
			]),
		);
	return {
		activeTabId: 'tab-1',
		conflictingInteraction: overrides.conflictingInteraction ?? false,
		sinks,
		state: {
			tabs: [
				{
					focusedPanelId: overrides.focusedPanelId ?? panels[0]?.id ?? 'missing',
					maximizedPanelId: overrides.maximizedPanelId ?? null,
					panelFocusHistory: panels[0] === undefined ? [] : [panels[0].id],
					root,
					tabId: 'tab-1',
				},
			],
		},
	};
}

function sink(
	revision: string,
	send: (
		input: WorkbenchTerminalBroadcastInput,
		expectedRevision: string,
	) => Promise<{ readonly kind: 'failed' | 'sent' }> = async () => ({ kind: 'sent' }),
): WorkbenchTerminalBroadcastInputSink {
	return {
		getReadiness: () => ({ kind: 'ready', revision }),
		send,
		subscribe: () => ({ dispose: () => undefined }),
	};
}

function terminalId(panelId: string): TerminalPresentationId {
	return `terminal-${panelId}` as TerminalPresentationId;
}

function panel(id: string, ended: 'exited' | 'running' = 'running'): PanelLeaf {
	return {
		activeChildPresentationId: null,
		childPresentations: [],
		id,
		kind: 'panel',
		primaryPresentation: {
			closeActivity: 'unknown',
			currentCwd: null,
			lifecycle:
				ended === 'exited' ? { exitCode: 0, kind: 'exited', signal: null } : { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalId(id),
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
			{ id: `${id}-leading`, node: leading, weight: 50 },
			{ id: `${id}-trailing`, node: trailing, weight: 50 },
		],
		id,
		kind: 'split',
		orientation,
	};
}

function snapshotForTwoTargets(): PanelBroadcastTargetSnapshot {
	const resolution = resolvePanelBroadcastStart(
		environmentFor(split('split-root', 'horizontal', panel('panel-1'), panel('panel-2'))),
		'panel-1',
	);
	if (resolution.kind !== 'ready') throw new Error('Expected a two-target snapshot.');
	return resolution.snapshot;
}

function envelope(
	epoch: number,
	sequence: number,
	input: WorkbenchTerminalBroadcastInput,
): PanelBroadcastInputEnvelope {
	return { epoch, input, sequence };
}

function keyInput(key: string): WorkbenchTerminalBroadcastInput {
	return {
		code: key,
		key,
		kind: 'key',
		modifiers: {
			alt: false,
			altGraph: false,
			altLeft: false,
			altRight: false,
			capsLock: false,
			control: false,
			controlRight: false,
			meta: false,
			metaRight: false,
			numLock: false,
			shift: false,
			shiftRight: false,
		},
		repeat: false,
	};
}

function deferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolve = (_value: Value): void => undefined;
	const promise = new Promise<Value>((settle) => {
		resolve = settle;
	});
	return { promise, resolve };
}

function generatedLayout(
	instructions: readonly {
		readonly orientation: PanelSplitOrientation;
		readonly targetIndex: number;
	}[],
): PanelLayoutNode {
	let root: PanelLayoutNode = panel('panel-1');
	for (const [index, instruction] of instructions.entries()) {
		const panels = collectPanels(root);
		const target = panels[instruction.targetIndex % panels.length];
		if (target === undefined) throw new Error('Expected a generated target Panel.');
		root = replacePanel(
			root,
			target.id,
			split(`split-${index + 1}`, instruction.orientation, target, panel(`panel-${index + 2}`)),
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
