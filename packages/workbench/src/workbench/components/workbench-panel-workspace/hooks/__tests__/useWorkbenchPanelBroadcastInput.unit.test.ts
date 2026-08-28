// @vitest-environment jsdom

import { TabModel } from '@orchbee/tab-strip';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputResult,
	WorkbenchTerminalBroadcastInputSink,
} from '../../../../types/workbench-terminal-surface';
import { PanelWorkspaceStore } from '../../model/PanelWorkspaceStore';
import {
	collectPanelLeaves,
	getDisplayedTerminal,
	type PanelLayoutNode,
	type PanelWorkspaceState,
} from '../../model/panel-workspace-state';
import { useWorkbenchPanelBroadcastInput } from '../useWorkbenchPanelBroadcastInput';

afterEach(() => cleanup());

describe('useWorkbenchPanelBroadcastInput', () => {
	it('serializes queued input through the captured sink revisions', async () => {
		const store = runningTwoPanelStore();
		const tab = store.getSnapshot().tabs[0];
		if (tab === undefined) throw new Error('Expected a two-Panel Tab.');
		const panels = collectPanelLeaves(tab.root);
		const presentations = panels.map((panel) => getDisplayedTerminal(panel).presentationId);
		const deliveries = presentations.map((presentationId) => {
			const first = deferred<WorkbenchTerminalBroadcastInputResult>();
			const revision = `${presentationId}:ready`;
			const send = vi
				.fn<
					(
						input: WorkbenchTerminalBroadcastInput,
						expectedRevision: string,
					) => Promise<WorkbenchTerminalBroadcastInputResult>
				>()
				.mockImplementationOnce(() => first.promise)
				.mockResolvedValue({ kind: 'sent' });
			const sink: WorkbenchTerminalBroadcastInputSink = {
				getReadiness: () => ({ kind: 'ready', revision }),
				send,
				subscribe: () => ({ dispose: () => undefined }),
			};
			return { first, revision, send, sink };
		});
		const workspaceElement = document.createElement('div');
		const rendered = renderHook(() =>
			useWorkbenchPanelBroadcastInput({
				activeTabId: store.getActiveTabId(),
				conflictingInteraction: false,
				store,
				workspaceReference: { current: workspaceElement },
				workspaceState: store.getSnapshot(),
			}),
		);
		const registrations: Array<{ dispose(): void }> = [];
		act(() => {
			for (const [index, presentationId] of presentations.entries()) {
				const delivery = deliveries[index];
				if (delivery === undefined) throw new Error('Expected a delivery sink.');
				registrations.push(rendered.result.current.bindSink(presentationId, delivery.sink));
			}
		});
		const sourcePanel = panels[0];
		if (sourcePanel === undefined) throw new Error('Expected a source Panel.');
		let started = false;
		act(() => {
			started = rendered.result.current.start(sourcePanel.id);
			rendered.result.current.enqueue({ kind: 'text', text: 'first' });
			rendered.result.current.enqueue({ kind: 'text', text: 'second' });
		});
		expect(started).toBe(true);
		await waitFor(() => {
			for (const delivery of deliveries) expect(delivery.send).toHaveBeenCalledOnce();
		});
		for (const delivery of deliveries) {
			expect(delivery.send).toHaveBeenCalledWith(
				{ kind: 'text', text: 'first' },
				delivery.revision,
			);
		}

		act(() => {
			for (const delivery of deliveries) delivery.first.resolve({ kind: 'sent' });
		});
		await waitFor(() => {
			for (const delivery of deliveries) expect(delivery.send).toHaveBeenCalledTimes(2);
		});
		for (const delivery of deliveries) {
			expect(delivery.send).toHaveBeenNthCalledWith(
				2,
				{ kind: 'text', text: 'second' },
				delivery.revision,
			);
		}
		await waitFor(() =>
			expect(rendered.result.current.state).toMatchObject({ inFlight: null, kind: 'active' }),
		);

		act(() => rendered.result.current.end());
		expect(rendered.result.current.state).toEqual({ kind: 'idle' });
		for (const registration of registrations) registration.dispose();
		rendered.unmount();
	});
});

function runningTwoPanelStore(): PanelWorkspaceStore {
	const tabModel = new TabModel({
		activeTabId: 'tab-broadcast',
		tabs: [{ group: null, id: 'tab-broadcast', pinned: false, title: 'Broadcast' }],
	});
	let presentationSequence = 1;
	const composition = {
		createPresentationId: () =>
			terminalPresentationId(`terminal-broadcast-${presentationSequence++}`),
		initialPresentationId: terminalPresentationId('terminal-broadcast-initial'),
		kind: 'preview' as const,
	};
	const builder = new PanelWorkspaceStore(tabModel, { composition });
	const initialTab = builder.getSnapshot().tabs[0];
	if (initialTab === undefined) throw new Error('Expected an initial Tab.');
	builder.transition({
		direction: 'right',
		newTerminalTitle: 'Terminal 2',
		panelId: initialTab.focusedPanelId,
		presentation: builder.createTerminalPresentation(),
		tabId: initialTab.tabId,
		type: 'panel-split',
	});
	return new PanelWorkspaceStore(tabModel, {
		composition,
		initialState: runningWorkspaceState(builder.getSnapshot()),
	});
}

function runningWorkspaceState(state: PanelWorkspaceState): PanelWorkspaceState {
	return {
		...state,
		tabs: state.tabs.map((tab) => ({ ...tab, root: runningLayout(tab.root) })),
	};
}

function runningLayout(node: PanelLayoutNode): PanelLayoutNode {
	if (node.kind === 'split') {
		return {
			...node,
			children: node.children.map((child) => ({ ...child, node: runningLayout(child.node) })),
		};
	}
	return {
		...node,
		childPresentations: node.childPresentations.map((presentation) => ({
			...presentation,
			lifecycle: { kind: 'running' },
		})),
		primaryPresentation: {
			...node.primaryPresentation,
			lifecycle: { kind: 'running' },
		},
	};
}

function deferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise = (_value: Value): void => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
